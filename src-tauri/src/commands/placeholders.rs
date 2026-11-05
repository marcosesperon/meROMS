//! Media «de relleno»: ficheros que las fuentes sirven cuando no tienen el de verdad (la
//! contraportada verde vacía de ScreenScraper, el vídeo «This game is not playable»…).
//!
//! No hay una lista pública de ellos, así que se aprenden de la propia biblioteca: un mismo
//! fichero (byte a byte) en PLACEHOLDER_MIN_GAMES juegos distintos de un sistema no puede ser
//! una carátula real. «Limpiar media» los enseña; al borrarlos, su huella (tamaño + MD5) va a
//! una lista negra y el scraper ya no los vuelve a guardar.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::layout::{Layout, SystemRef};
use crate::media::{self, MediaIndex};
use crate::models::Game;

/// Con cuántos juegos distintos un fichero idéntico cuenta como relleno
pub const PLACEHOLDER_MIN_GAMES: usize = 5;

/// Huella de un relleno de la lista negra
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceholderEntry {
    pub size: u64,
    pub md5: String,
    /// Tipo de medio donde se vio (solo informativo)
    #[serde(default)]
    pub field: String,
}

/// Un relleno encontrado en el sistema
#[derive(Debug, Clone, Serialize)]
pub struct PlaceholderGroup {
    pub md5: String,
    pub size: u64,
    /// Tipos de medio en los que aparece (image, boxback, video…)
    pub fields: Vec<String>,
    /// Cuántos juegos distintos lo tienen
    pub games: usize,
    pub files: Vec<PlaceholderFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaceholderFile {
    pub path: String,
    pub field: String,
    pub game_path: String,
    pub game_name: String,
}

// ---------- Lista negra ----------

/// Fichero de la lista negra (se fija al arrancar la app) y su contenido, ya leído
static LIST_FILE: OnceLock<PathBuf> = OnceLock::new();
static LIST: Mutex<Option<Vec<PlaceholderEntry>>> = Mutex::new(None);

/// Al arrancar: dónde vive la lista (carpeta de datos de la app)
pub fn init(app: &tauri::AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = LIST_FILE.set(dir.join("media_placeholders.json"));
    }
}

fn read_list(file: &Path) -> Vec<PlaceholderEntry> {
    fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Ejecuta `f` con la lista en memoria (cargándola la primera vez)
fn with_list<T>(f: impl FnOnce(&mut Vec<PlaceholderEntry>) -> T) -> T {
    let mut guard = LIST.lock().unwrap_or_else(|e| e.into_inner());
    let list = guard.get_or_insert_with(|| LIST_FILE.get().map(|f| read_list(f)).unwrap_or_default());
    f(list)
}

fn save_list(list: &[PlaceholderEntry]) -> Result<(), String> {
    let Some(file) = LIST_FILE.get() else { return Ok(()) };
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    fs::write(file, json).map_err(|e| format!("Failed to save {}: {}", file.display(), e))
}

fn md5_hex(bytes: &[u8]) -> String {
    format!("{:x}", Md5::digest(bytes))
}

/// ¿Es un relleno conocido? Primero el tamaño (barato) y, solo si coincide, el MD5.
/// Lo usan las descargas del scraper antes de escribir el fichero.
pub fn is_placeholder(bytes: &[u8]) -> bool {
    let size = bytes.len() as u64;
    with_list(|list| {
        if !list.iter().any(|e| e.size == size) {
            return false;
        }
        let hash = md5_hex(bytes);
        list.iter().any(|e| e.size == size && e.md5 == hash)
    })
}

fn add_entries(list: &mut Vec<PlaceholderEntry>, entries: Vec<PlaceholderEntry>) -> usize {
    let before = list.len();
    for entry in entries {
        if !list.iter().any(|e| e.size == entry.size && e.md5 == entry.md5) {
            list.push(entry);
        }
    }
    list.len() - before
}

/// Apunta rellenos en la lista negra (tras borrarlos en «Limpiar media»). Devuelve cuántos nuevos.
#[tauri::command]
pub fn learn_placeholders(entries: Vec<PlaceholderEntry>) -> Result<usize, String> {
    with_list(|list| {
        let added = add_entries(list, entries);
        save_list(list)?;
        Ok(added)
    })
}

/// Cuántos rellenos hay aprendidos (Ajustes)
#[tauri::command]
pub fn placeholder_count() -> usize {
    with_list(|list| list.len())
}

/// Olvida todos los rellenos aprendidos (Ajustes)
#[tauri::command]
pub fn forget_placeholders() -> Result<(), String> {
    with_list(|list| {
        list.clear();
        save_list(list)
    })
}

// ---------- Búsqueda en un sistema ----------

/// Rellenos del sistema: ficheros idénticos en PLACEHOLDER_MIN_GAMES juegos distintos o más
#[tauri::command]
pub fn find_placeholder_media(app: tauri::AppHandle, system_path: String) -> Result<Vec<PlaceholderGroup>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let games = super::gamelist::read_gamelist(&layout, &system)?.games;
    Ok(find_placeholders(&layout, &system, &games))
}

fn find_placeholders(layout: &Layout, system: &SystemRef, games: &[Game]) -> Vec<PlaceholderGroup> {
    let index = MediaIndex::build(layout, system);

    // Todos los ficheros de media de cada juego, agrupados por tamaño (no se lee nada aún)
    let mut by_size: HashMap<u64, Vec<(PathBuf, &'static str, &Game)>> = HashMap::new();
    let mut seen: HashSet<(PathBuf, &str, &str)> = HashSet::new();
    for game in games {
        for kind in media::MEDIA_KINDS {
            let Some(path) = super::media_kinds::current_file(layout, system, &index, game, kind) else { continue };
            // Un mismo fichero apuntado desde varios juegos (clásica) cuenta para cada uno,
            // pero se lista una sola vez por juego y tipo
            if !seen.insert((path.clone(), game.path.as_str(), kind.field)) {
                continue;
            }
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            if size > 0 {
                by_size.entry(size).or_default().push((path, kind.field, game));
            }
        }
    }

    let distinct_games = |files: &[(PathBuf, &str, &Game)]| files.iter().map(|(_, _, g)| g.path.as_str()).collect::<HashSet<_>>().len();
    let mut groups = Vec::new();
    for (size, files) in by_size {
        // Solo se calcula el MD5 si hay bastantes juegos con ese tamaño exacto
        if distinct_games(&files) < PLACEHOLDER_MIN_GAMES {
            continue;
        }
        let mut by_hash: HashMap<String, Vec<(PathBuf, &str, &Game)>> = HashMap::new();
        let mut hashes: HashMap<PathBuf, Option<String>> = HashMap::new();
        for (path, field, game) in files {
            let hash = hashes
                .entry(path.clone())
                .or_insert_with(|| fs::read(&path).ok().map(|b| md5_hex(&b)))
                .clone();
            if let Some(hash) = hash {
                by_hash.entry(hash).or_default().push((path, field, game));
            }
        }
        for (md5, files) in by_hash {
            let games = distinct_games(&files);
            if games < PLACEHOLDER_MIN_GAMES {
                continue;
            }
            let mut fields: Vec<String> = files.iter().map(|(_, f, _)| f.to_string()).collect();
            fields.sort();
            fields.dedup();
            let mut list: Vec<PlaceholderFile> = files
                .iter()
                .map(|(path, field, game)| PlaceholderFile {
                    path: path.to_string_lossy().to_string(),
                    field: field.to_string(),
                    game_path: game.path.clone(),
                    game_name: game.name.clone().unwrap_or_else(|| game.path.clone()),
                })
                .collect();
            list.sort_by(|a, b| a.game_name.to_lowercase().cmp(&b.game_name.to_lowercase()));
            // Una ruta compartida (clásica) aparece una vez en la lista de ficheros a borrar
            list.dedup_by(|a, b| a.path == b.path && a.field == b.field);
            groups.push(PlaceholderGroup { md5, size, fields, games, files: list });
        }
    }
    groups.sort_by(|a, b| b.games.cmp(&a.games));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn esde(root: &Path) -> Layout {
        Layout::Esde(crate::layout::EsdePaths { home: root.join("ES-DE"), media_root: root.join("media") })
    }

    fn games(names: &[&str]) -> Vec<Game> {
        names.iter().map(|n| Game { path: format!("./{}.sfc", n), name: Some(n.to_string()), ..Default::default() }).collect()
    }

    #[test]
    fn the_same_file_in_five_games_is_a_placeholder_in_four_it_is_not() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        let list = games(&["a", "b", "c", "d", "e", "f"]);
        for g in ["a", "b", "c", "d", "e"] {
            write(&tmp.path().join(format!("media/snes/backcovers/{}.png", g)), b"contraportada verde vacia");
        }
        // Cuatro juegos con el mismo vídeo: no llega
        for g in ["a", "b", "c", "d"] {
            write(&tmp.path().join(format!("media/snes/videos/{}.mp4", g)), b"video repetido");
        }
        // Dos regiones con la misma carátula (y otra distinta): nada
        write(&tmp.path().join("media/snes/covers/e.png"), b"caratula");
        write(&tmp.path().join("media/snes/covers/f.png"), b"caratula");

        let found = find_placeholders(&layout, &system, &list);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].games, 5);
        assert_eq!(found[0].fields, vec!["boxback".to_string()]);
        assert_eq!(found[0].files.len(), 5);
        assert_eq!(found[0].md5, md5_hex(b"contraportada verde vacia"));
    }

    #[test]
    fn learned_entries_are_not_repeated() {
        let mut list = Vec::new();
        let entry = PlaceholderEntry { size: 3, md5: md5_hex(b"abc"), field: "boxback".into() };
        assert_eq!(add_entries(&mut list, vec![entry.clone(), entry.clone()]), 1);
        assert_eq!(add_entries(&mut list, vec![entry]), 0);
        assert_eq!(list.len(), 1);
    }
}
