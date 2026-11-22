//! Fuente «ScreenScraper (por contenido)» de Renombrar: el nombre de fichero que ScreenScraper
//! tiene registrado para cada ROM. Se pregunta por la huella (como el scraper) y solo vale si
//! la huella que devuelve coincide con la nuestra; si solo la encontró por nombre, nos
//! devolvería el mismo nombre malo. Lo encontrado se guarda en `ss_rom_names.json` (carpeta de
//! datos de la app) por ruta + tamaño + fecha, para no gastar cupo dos veces con la misma ROM.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::{sleep, Duration};

use crate::layout::SystemRef;
use crate::models::game::Game;
use crate::models::system::screenscraper_id_for_system;
use crate::models::AppSettings;

use super::progress::Progress;
use super::scraper::{build_http_client, ss_lookup_rom, RomKey, REQUEST_DELAY_MS};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    size: u64,
    modified: u64,
    /// Nombre de fichero de ScreenScraper; `None` = se preguntó y no lo tiene
    name: Option<String>,
}

/// Caché de nombres por ROM (ruta absoluta)
struct NameCache {
    file: Option<PathBuf>,
    entries: HashMap<String, CacheEntry>,
    dirty: bool,
}

fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some((meta.len(), modified))
}

impl NameCache {
    fn load(file: &Path) -> Self {
        let entries = fs::read_to_string(file).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
        NameCache { file: Some(file.to_path_buf()), entries, dirty: false }
    }

    /// Lo guardado para una ROM, si el fichero no ha cambiado desde entonces.
    /// `Some(None)` = ya se preguntó y ScreenScraper no tiene nombre para ella.
    fn cached(&self, rom: &Path) -> Option<Option<String>> {
        let entry = self.entries.get(&rom.to_string_lossy().to_string())?;
        let (size, modified) = stamp(rom)?;
        (entry.size == size && entry.modified == modified).then(|| entry.name.clone())
    }

    fn put(&mut self, rom: &Path, name: Option<String>) {
        let Some((size, modified)) = stamp(rom) else { return };
        self.entries.insert(rom.to_string_lossy().to_string(), CacheEntry { size, modified, name });
        self.dirty = true;
    }

    fn save(&mut self) -> Result<(), String> {
        let Some(file) = &self.file else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        let json = serde_json::to_string(&self.entries).map_err(|e| e.to_string())?;
        fs::write(file, json).map_err(|e| format!("Failed to save {}: {}", file.display(), e))?;
        self.dirty = false;
        Ok(())
    }
}

fn cache_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    Ok(dir.join("ss_rom_names.json"))
}

/// De la respuesta de `jeuInfos.php`, el nombre de fichero de la ROM cuya huella es la nuestra.
/// Se mira la ROM reconocida (`rom`) y la lista de ROMs del juego (`roms`); sin coincidencia, nada.
fn matching_filename(jeu: &Value, key: &RomKey) -> Option<String> {
    let listed = jeu["roms"].as_array().into_iter().flatten();
    std::iter::once(&jeu["rom"]).chain(listed).find_map(|rom| {
        let (field, ours) = match key {
            RomKey::Crc(crc) => ("romcrc", crc),
            RomKey::Md5(md5) => ("rommd5", md5),
        };
        let theirs = rom[field].as_str()?;
        if !theirs.eq_ignore_ascii_case(ours) {
            return None;
        }
        // Solo el nombre, por si viniera con carpeta
        let name = rom["romfilename"].as_str()?.rsplit(['/', '\\']).next()?.trim();
        (!name.is_empty()).then(|| name.to_string())
    })
}

/// Nombre sin extensión de ScreenScraper para cada juego (clave = ruta del juego en la gamelist).
/// Pregunta solo por las ROMs que no están en la caché; con pausa entre preguntas, progreso y
/// Cancelar (lo ya preguntado se guarda igualmente). Los que no tienen nombre no salen.
pub fn names_for(app: &tauri::AppHandle, settings: &AppSettings, system: &SystemRef, games: &[&Game], progress: &Progress) -> Result<HashMap<String, String>, String> {
    if !settings.has_screenscraper() {
        return Err("ScreenScraper is not configured (Settings > General)".to_string());
    }
    if screenscraper_id_for_system(&system.name).is_none() {
        return Err(format!("Unknown system for ScreenScraper: {}", system.name));
    }
    let mut cache = NameCache::load(&cache_file(app)?);
    // Este comando ya corre dentro del runtime de Tauri, que no deja esperar a otra tarea
    // asíncrona desde dentro: las preguntas van en un hilo aparte, y aquí se espera a que acabe
    let result = std::thread::scope(|scope| {
        scope
            .spawn(|| tauri::async_runtime::block_on(ask_missing(settings, system, games, progress, &mut cache)))
            .join()
            .unwrap_or_else(|_| Err("ScreenScraper lookup failed".to_string()))
    });
    cache.save()?;
    result
}

async fn ask_missing(settings: &AppSettings, system: &SystemRef, games: &[&Game], progress: &Progress, cache: &mut NameCache) -> Result<HashMap<String, String>, String> {
    let client = build_http_client()?;
    let mut names = HashMap::new();
    let mut asked = 0;
    for (i, game) in games.iter().enumerate() {
        progress.check()?;
        progress.step("asking", i + 1, games.len(), &game.path);
        let rom = system.rom_dir.join(game.path.strip_prefix("./").unwrap_or(&game.path));
        // Las carpetas-ROM no se renombran con extensión: no se pregunta por ellas
        if !rom.is_file() {
            continue;
        }
        let name = match cache.cached(&rom) {
            Some(name) => name,
            None => {
                if asked > 0 {
                    sleep(Duration::from_millis(REQUEST_DELAY_MS)).await;
                }
                asked += 1;
                let (jeu, key) = ss_lookup_rom(&client, settings, game, system).await?;
                let name = jeu.and_then(|jeu| matching_filename(&jeu, &key));
                cache.put(&rom, name.clone());
                name
            }
        };
        if let Some(stem) = name.as_deref().and_then(|n| Path::new(n).file_stem()).map(|s| s.to_string_lossy().to_string()) {
            names.insert(game.path.clone(), stem);
        }
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_name_only_counts_when_the_hash_matches() {
        let jeu = json!({
            "rom": { "romfilename": "Wrong Name.sfc", "rommd5": "AAAA", "romcrc": "11111111" },
            "roms": [
                { "romfilename": "Super Mario World (USA).sfc", "rommd5": "abcd", "romcrc": "2222abcd" },
                { "romfilename": "Super Mario World (Europe).sfc", "rommd5": "ffff", "romcrc": "33333333" }
            ]
        });
        // MD5 sin distinguir mayúsculas
        assert_eq!(matching_filename(&jeu, &RomKey::Md5("ABCD".into())).as_deref(), Some("Super Mario World (USA).sfc"));
        assert_eq!(matching_filename(&jeu, &RomKey::Crc("33333333".into())).as_deref(), Some("Super Mario World (Europe).sfc"));
        assert_eq!(matching_filename(&jeu, &RomKey::Crc("11111111".into())).as_deref(), Some("Wrong Name.sfc"));
        // Encontrado solo por nombre: ninguna huella coincide
        assert_eq!(matching_filename(&jeu, &RomKey::Md5("9999".into())), None);
        assert_eq!(matching_filename(&json!({ "id": "1" }), &RomKey::Md5("abcd".into())), None);
    }

    #[test]
    fn the_cache_is_dropped_when_the_rom_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let rom = tmp.path().join("Mario.sfc");
        let file = tmp.path().join("ss_rom_names.json");
        fs::write(&rom, b"version 1").unwrap();

        let mut cache = NameCache::load(&file);
        assert_eq!(cache.cached(&rom), None);
        cache.put(&rom, Some("Super Mario World (USA).sfc".into()));
        cache.save().unwrap();
        // Otra sesión: sale de la caché
        assert_eq!(NameCache::load(&file).cached(&rom), Some(Some("Super Mario World (USA).sfc".into())));
        // Otro tamaño: ya no vale
        fs::write(&rom, b"version dos, mas larga").unwrap();
        assert_eq!(NameCache::load(&file).cached(&rom), None);
    }
}
