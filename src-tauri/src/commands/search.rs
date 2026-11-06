//! Buscador global (⌘K): un juego entre todos los sistemas del perfil, por su nombre o el de
//! su ROM, sin importar tildes ni mayúsculas.
//!
//! Los gamelists se guardan leídos en memoria (por ruta y fecha de modificación): la primera
//! búsqueda los lee todos y las siguientes ya no tocan el disco, salvo los que hayan cambiado.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Serialize;

use crate::layout::{strip_dot_slash, Layout, SystemRef};
use crate::media;
use crate::models::system::system_full_name;
use crate::parser;

/// Como mucho, cuántos resultados se devuelven
const MAX_RESULTS: usize = 60;

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub system_path: String,
    pub system_name: String,
    pub system_full_name: String,
    pub game_path: String,
    pub name: String,
    /// Carátula (ruta absoluta) si la hay
    pub cover: Option<String>,
}

/// Lo que se guarda de cada juego para buscar: ruta, nombre a enseñar, textos ya normalizados
/// y la carátula de su campo (estructura clásica)
#[derive(Clone)]
struct Entry {
    path: String,
    name: String,
    name_key: String,
    file_key: String,
    image: Option<String>,
}

struct Cached {
    modified: Option<SystemTime>,
    entries: Vec<Entry>,
}

static CACHE: Mutex<Option<HashMap<PathBuf, Cached>>> = Mutex::new(None);

/// Minúsculas y sin tildes («Pokémon» → «pokemon»); lo que no es letra ni cifra, un espacio
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(|c| c.to_lowercase()) {
        let base = match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'ø' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        out.push(base);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Puntuación de un texto para lo buscado (menos = mejor): empieza por ello (0), alguna palabra
/// empieza por ello (1), lo contiene (2). None si no aparece.
fn rank(text: &str, query: &str) -> Option<u8> {
    if text.starts_with(query) {
        Some(0)
    } else if text.split(' ').any(|w| w.starts_with(query)) || text.contains(&format!(" {}", query)) {
        Some(1)
    } else if text.contains(query) {
        Some(2)
    } else {
        None
    }
}

/// Juegos de un gamelist, leyéndolo solo si ha cambiado desde la última vez
fn entries_of(gamelist: &Path) -> Vec<Entry> {
    let modified = fs::metadata(gamelist).and_then(|m| m.modified()).ok();
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(c) = cache.get(gamelist) {
        if c.modified == modified {
            return c.entries.clone();
        }
    }
    let entries: Vec<Entry> = fs::read_to_string(gamelist)
        .ok()
        .and_then(|xml| parser::parse_gamelist(&xml).ok())
        .map(|gl| {
            gl.games
                .into_iter()
                .map(|g| {
                    let file = Path::new(strip_dot_slash(&g.path))
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let name = g.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| file.clone());
                    Entry { name_key: normalize(&name), file_key: normalize(&file), name, path: g.path, image: g.image }
                })
                .collect()
        })
        .unwrap_or_default();
    cache.insert(gamelist.to_path_buf(), Cached { modified, entries: entries.clone() });
    entries
}

/// Carátula de un resultado: en clásica, la de su campo; en ES-DE (sin rutas en el XML), la de
/// `covers/` con el nombre de la ROM
fn cover_of(layout: &Layout, system: &SystemRef, entry: &Entry) -> Option<String> {
    if let Some(image) = entry.image.as_deref().filter(|i| !i.trim().is_empty()) {
        let path = media::resolve_media_path(system, image);
        if path.is_file() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    let kind = media::kind_for_folder("covers")?;
    kind.extensions()
        .iter()
        .map(|ext| layout.media_file_path(system, kind.folder, &entry.path, ext))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().to_string())
}

/// Lo que se busca en un juego: la mejor puntuación entre su nombre y el de su ROM
fn score(entry: &Entry, query: &str) -> Option<u8> {
    let by_name = rank(&entry.name_key, query);
    // Por el fichero cuenta un poco menos que por el nombre
    let by_file = rank(&entry.file_key, query).map(|r| r + 1);
    match (by_name, by_file) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn search(layout: &Layout, system_paths: &[String], query: &str) -> Vec<SearchHit> {
    let query = normalize(query);
    if query.is_empty() {
        return Vec::new();
    }
    // (puntuación, orden del sistema, nombre) para ordenar; los sistemas, en el orden recibido
    let mut found: Vec<(u8, usize, String, SystemRef, Entry)> = Vec::new();
    for (i, system_path) in system_paths.iter().enumerate() {
        let system = SystemRef::from_rom_dir(system_path);
        for entry in entries_of(&layout.gamelist_path(&system)) {
            if let Some(s) = score(&entry, &query) {
                found.push((s, i, entry.name_key.clone(), system.clone(), entry));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    found.truncate(MAX_RESULTS);
    // Agrupados por sistema (en el orden de su mejor resultado), cada grupo por puntuación
    let mut order: Vec<usize> = Vec::new();
    for f in &found {
        if !order.contains(&f.1) {
            order.push(f.1);
        }
    }
    found.sort_by_key(|f| order.iter().position(|&o| o == f.1));
    found
        .into_iter()
        .map(|(_, _, _, system, entry)| SearchHit {
            system_path: system.rom_dir.to_string_lossy().to_string(),
            system_full_name: system_full_name(&system.name).unwrap_or(&system.name).to_string(),
            system_name: system.name.clone(),
            cover: cover_of(layout, &system, &entry),
            game_path: entry.path,
            name: entry.name,
        })
        .collect()
}

/// Busca `query` en los gamelists de estos sistemas (los del perfil activo, en el orden de la
/// barra lateral)
#[tauri::command]
pub async fn search_games(app: tauri::AppHandle, system_paths: Vec<String>, query: String) -> Result<Vec<SearchHit>, String> {
    let layout = super::active_layout(&app)?;
    tauri::async_runtime::spawn_blocking(move || Ok(search(&layout, &system_paths, &query)))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_tildes_y_signos() {
        assert_eq!(normalize("Pokémon: Edición Roja"), "pokemon edicion roja");
        assert_eq!(normalize("  Street Fighter II' - Champion  "), "street fighter ii champion");
    }

    #[test]
    fn orden_de_las_coincidencias() {
        assert_eq!(rank("alex kidd in miracle world", "alex"), Some(0));
        assert_eq!(rank("the alex kidd", "alex"), Some(1));
        assert_eq!(rank("superalex", "alex"), Some(2));
        assert_eq!(rank("sonic", "alex"), None);
    }

    fn gamelist(dir: &Path, system: &str, games: &[(&str, &str)]) {
        let d = dir.join(system);
        fs::create_dir_all(&d).unwrap();
        let body: String = games
            .iter()
            .map(|(p, n)| format!("<game><path>{}</path><name>{}</name></game>", p, n))
            .collect();
        fs::write(d.join("gamelist.xml"), format!("<?xml version=\"1.0\"?><gameList>{}</gameList>", body)).unwrap();
    }

    #[test]
    fn busca_en_todos_los_sistemas_por_nombre_y_fichero() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        gamelist(root, "cps1", &[("./sf2.zip", "Street Fighter II"), ("./ffight.zip", "Final Fight")]);
        gamelist(root, "snes", &[("./Street Fighter II Turbo.zip", "Street Fighter II Turbo"), ("./Zelda.zip", "Zelda")]);
        let systems: Vec<String> = ["cps1", "snes"].iter().map(|s| root.join(s).to_string_lossy().to_string()).collect();

        let hits = search(&Layout::Classic, &systems, "street");
        assert_eq!(hits.iter().map(|h| h.name.as_str()).collect::<Vec<_>>(), vec!["Street Fighter II", "Street Fighter II Turbo"]);
        assert_eq!(hits[0].system_name, "cps1");

        // Por el nombre de la ROM
        let hits = search(&Layout::Classic, &systems, "SF2");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].game_path, "./sf2.zip");

        // La caché se renueva si el gamelist cambia
        std::thread::sleep(std::time::Duration::from_millis(1100));
        gamelist(root, "snes", &[("./Zelda.zip", "The Legend of Zelda")]);
        let hits = search(&Layout::Classic, &systems, "legend");
        assert_eq!(hits.len(), 1);
        assert!(search(&Layout::Classic, &systems, "").is_empty());
    }
}
