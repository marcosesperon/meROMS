//! Fuente «ScreenScraper (por contenido)» de Renombrar: el nombre de fichero que ScreenScraper
//! tiene registrado para cada ROM. Se pregunta por la huella (como el scraper) y solo vale si
//! la huella que devuelve coincide con la nuestra; si solo la encontró por nombre, nos
//! devolvería el mismo nombre malo. Lo encontrado se guarda en `ss_rom_names.json` (carpeta de
//! datos de la app) por ruta + tamaño + fecha, para no gastar cupo dos veces con la misma ROM.
//! El scraper también la rellena (`remember`) con las respuestas que ya recibe al scrapear.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::{sleep, Duration};

use crate::layout::SystemRef;
use crate::models::game::Game;
use crate::models::system::screenscraper_id_for_system;
use crate::models::AppSettings;

use super::progress::Progress;
use super::scraper::{build_http_client, is_fatal_ss_error, ss_lookup_rom, RomKey, REQUEST_DELAY_MS};

/// Tantas ROMs seguidas con fallo de conexión (no por tardar) y se para: ScreenScraper
/// probablemente está caído
const MAX_FAILURES_IN_A_ROW: usize = 5;

/// Renombrar y el scraper pueden escribir la caché a la vez: se lee y se escribe de uno en uno
static CACHE_LOCK: Mutex<()> = Mutex::new(());

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
    /// Lo añadido en esta sesión: al guardar se mezcla con lo que haya en disco entonces
    changed: HashMap<String, CacheEntry>,
}

fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some((meta.len(), modified))
}

impl NameCache {
    fn load(file: &Path) -> Self {
        let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        NameCache { file: Some(file.to_path_buf()), entries: read_entries(file), changed: HashMap::new() }
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
        let key = rom.to_string_lossy().to_string();
        let entry = CacheEntry { size, modified, name };
        self.entries.insert(key.clone(), entry.clone());
        self.changed.insert(key, entry);
    }

    /// Escribe lo nuevo encima de lo que haya ahora en disco (otro pudo guardar mientras tanto)
    fn save(&mut self) -> Result<(), String> {
        let Some(file) = &self.file else { return Ok(()) };
        if self.changed.is_empty() {
            return Ok(());
        }
        let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut on_disk = read_entries(file);
        on_disk.extend(self.changed.drain());
        let json = serde_json::to_string(&on_disk).map_err(|e| e.to_string())?;
        fs::write(file, json).map_err(|e| format!("Failed to save {}: {}", file.display(), e))
    }
}

fn read_entries(file: &Path) -> HashMap<String, CacheEntry> {
    fs::read_to_string(file).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

/// Para el scraper: guarda lo que dice una respuesta de ScreenScraper sobre una ROM (su nombre
/// si la huella coincide; si no, «no lo tiene»). `jeu` = None: no reconoció la ROM. Es un extra:
/// si no se puede guardar, el scrapeo sigue igual.
pub(super) fn remember(app: &tauri::AppHandle, rom: &Path, jeu: Option<&Value>, key: &RomKey) {
    let Ok(file) = cache_file(app) else { return };
    remember_in(&file, rom, jeu, key);
}

fn remember_in(file: &Path, rom: &Path, jeu: Option<&Value>, key: &RomKey) {
    let mut cache = NameCache::load(file);
    cache.put(rom, jeu.and_then(|jeu| matching_filename(jeu, key)));
    let _ = cache.save();
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

/// Lo que sale de una búsqueda de nombres
#[derive(Debug, Default)]
pub struct ScraperNames {
    /// Nombre sin extensión por ruta del juego (los que no tienen nombre no salen)
    pub names: HashMap<String, String>,
    /// Juegos por los que no se llegó a preguntar porque se paró la búsqueda
    pub pending: HashSet<String>,
    /// Juegos por los que ScreenScraper no contestó (tiempo de espera, conexión…): no se
    /// guardan en caché, así que el próximo «Buscar» los reintenta
    pub failed: HashSet<String>,
    /// Se paró con Cancelar
    pub stopped: bool,
}

/// Nombre sin extensión de ScreenScraper para cada juego (clave = ruta del juego en la gamelist).
/// Pregunta solo por las ROMs que no están en la caché; con pausa entre preguntas y progreso.
/// Cancelar no da error: deja de preguntar, termina con lo que haya en caché y marca el resto
/// como pendiente (lo ya preguntado queda guardado y no se vuelve a pedir).
pub fn names_for(app: &tauri::AppHandle, settings: &AppSettings, system: &SystemRef, games: &[&Game], progress: &Progress) -> Result<ScraperNames, String> {
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

/// Lo que ya se sabe sin preguntar (caché): para la vista previa de Renombrar, que se recalcula
/// con cada opción. Las ROMs que nunca se preguntaron salen como pendientes («Buscar»).
pub fn cached_names(app: &tauri::AppHandle, system: &SystemRef, games: &[&Game]) -> Result<ScraperNames, String> {
    let cache = NameCache::load(&cache_file(app)?);
    let mut found = ScraperNames::default();
    for game in games {
        let rom = system.rom_dir.join(game.path.strip_prefix("./").unwrap_or(&game.path));
        if !rom.is_file() {
            continue;
        }
        match cache.cached(&rom) {
            Some(name) => {
                if let Some(stem) = name.as_deref().and_then(|n| Path::new(n).file_stem()) {
                    found.names.insert(game.path.clone(), stem.to_string_lossy().to_string());
                }
            }
            None => {
                found.pending.insert(game.path.clone());
            }
        }
    }
    Ok(found)
}

async fn ask_missing(settings: &AppSettings, system: &SystemRef, games: &[&Game], progress: &Progress, cache: &mut NameCache) -> Result<ScraperNames, String> {
    let client = build_http_client()?;
    let mut found = ScraperNames::default();
    let mut asked = 0;
    let mut failures_in_a_row = 0;
    for (i, game) in games.iter().enumerate() {
        // Parado: ya no se pregunta, pero lo que está en caché sigue valiendo
        found.stopped = found.stopped || progress.cancelled();
        if !found.stopped {
            progress.step("asking", i + 1, games.len(), &game.path);
        }
        let rom = system.rom_dir.join(game.path.strip_prefix("./").unwrap_or(&game.path));
        // Las carpetas-ROM no se renombran con extensión: no se pregunta por ellas
        if !rom.is_file() {
            continue;
        }
        let name = match cache.cached(&rom) {
            Some(name) => name,
            None if found.stopped => {
                found.pending.insert(game.path.clone());
                continue;
            }
            None => {
                if asked > 0 {
                    sleep(Duration::from_millis(REQUEST_DELAY_MS)).await;
                }
                asked += 1;
                match ss_lookup_rom(&client, settings, game, system).await {
                    Ok((jeu, key)) => {
                        failures_in_a_row = 0;
                        let name = jeu.and_then(|jeu| matching_filename(&jeu, &key));
                        cache.put(&rom, name.clone());
                        name
                    }
                    // Credenciales, cupo agotado…: no tiene sentido seguir
                    Err(e) if is_fatal_ss_error(&e) => return Err(e),
                    // Solo esta ROM (no contestó a tiempo, conexión…): se salta y se sigue
                    Err(e) => {
                        // Que tarde con una ROM concreta es normal (en NES, con los recopilatorios
                        // chinos): solo los fallos de conexión hacen pensar que está caído
                        if !e.contains("timed out") {
                            failures_in_a_row += 1;
                        }
                        if failures_in_a_row >= MAX_FAILURES_IN_A_ROW {
                            return Err(format!("ScreenScraper did not answer for {} ROMs in a row: {}", failures_in_a_row, e));
                        }
                        found.failed.insert(game.path.clone());
                        continue;
                    }
                }
            }
        };
        if let Some(stem) = name.as_deref().and_then(|n| Path::new(n).file_stem()).map(|s| s.to_string_lossy().to_string()) {
            found.names.insert(game.path.clone(), stem);
        }
    }
    Ok(found)
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

    #[test]
    fn a_stopped_search_keeps_what_is_cached_and_marks_the_rest_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        fs::create_dir_all(&system.rom_dir).unwrap();
        fs::write(system.rom_dir.join("cached.sfc"), b"a").unwrap();
        fs::write(system.rom_dir.join("new.sfc"), b"b").unwrap();
        let mut cache = NameCache::load(&tmp.path().join("ss_rom_names.json"));
        cache.put(&system.rom_dir.join("cached.sfc"), Some("Super Mario World (USA).sfc".into()));
        let games = [Game { path: "./new.sfc".into(), ..Default::default() }, Game { path: "./cached.sfc".into(), ..Default::default() }];
        let refs: Vec<&Game> = games.iter().collect();

        // Cancelada antes de empezar: no se pregunta nada (sin red), pero la caché sale
        let progress = Progress::for_task("ss-names-stop");
        super::super::progress::cancel_task("ss-names-stop".into());
        let found = tauri::async_runtime::block_on(ask_missing(&AppSettings::default(), &system, &refs, &progress, &mut cache)).unwrap();
        assert!(found.stopped);
        assert_eq!(found.names.get("./cached.sfc").map(String::as_str), Some("Super Mario World (USA)"));
        assert!(found.pending.contains("./new.sfc"));
        assert!(!found.pending.contains("./cached.sfc"));
    }

    #[test]
    fn scraping_leaves_the_name_ready_for_rename() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("ss_rom_names.json");
        let good = tmp.path().join("mario mal.sfc");
        let other = tmp.path().join("otro.sfc");
        fs::write(&good, b"a").unwrap();
        fs::write(&other, b"b").unwrap();
        let jeu = json!({ "rom": { "romfilename": "Super Mario World (USA).sfc", "rommd5": "abcd" } });

        remember_in(&file, &good, Some(&jeu), &RomKey::Md5("ABCD".into()));
        // Juego reconocido pero ninguna ROM con nuestra huella, y ROM no reconocida: «no lo tiene»
        remember_in(&file, &other, Some(&jeu), &RomKey::Md5("ffff".into()));
        let cache = NameCache::load(&file);
        assert_eq!(cache.cached(&good), Some(Some("Super Mario World (USA).sfc".into())));
        assert_eq!(cache.cached(&other), Some(None));

        // Una sesión de Renombrar abierta antes no borra lo que guardó el scraper mientras tanto
        let mut rename = NameCache::load(&file);
        remember_in(&file, &other, None, &RomKey::Md5("ffff".into()));
        let third = tmp.path().join("tercero.sfc");
        fs::write(&third, b"c").unwrap();
        rename.put(&third, None);
        rename.save().unwrap();
        let cache = NameCache::load(&file);
        assert_eq!(cache.cached(&good), Some(Some("Super Mario World (USA).sfc".into())));
        assert_eq!(cache.cached(&third), Some(None));
    }
}
