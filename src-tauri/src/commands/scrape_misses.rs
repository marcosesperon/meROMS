//! ROMs que el scraper no encontró: una marca con la fecha del intento, para saber al volver a
//! un sistema cuáles ya se intentaron (y que Scrapear todo se las salte si no se pide lo contrario).
//!
//! Se pone sola cuando un scrapeo responde «no encontrado» (o la búsqueda manual no da ningún
//! resultado) y se quita sola cuando el juego se scrapea bien. También se pone y quita a mano.
//!
//! Igual que los estados de juego (`game_status.rs`), vive en meROMS (`scrape_misses.json` en la
//! carpeta de datos de la app) y no en la gamelist, con la ruta absoluta de la ROM como clave;
//! renombrar, mover, CHD y deshacer la llevan a la ruta nueva (`remap`), quitar el juego la borra
//! (`forget`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::layout::strip_dot_slash;

/// La marca de un juego
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrapeMiss {
    /// Cuándo (segundos Unix): el último intento sin resultado, o cuándo se marcó a mano
    pub at: i64,
    /// Puesta a mano (juego dado por imposible), no por un scrapeo
    #[serde(default, skip_serializing_if = "is_false")]
    pub manual: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

// ---------- Almacén ----------

type Store = HashMap<String, ScrapeMiss>;

static STORE_FILE: OnceLock<PathBuf> = OnceLock::new();
static STORE: Mutex<Option<Store>> = Mutex::new(None);

/// Al arrancar: dónde vive el fichero (carpeta de datos de la app)
pub fn init(app: &tauri::AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = STORE_FILE.set(dir.join("scrape_misses.json"));
    }
}

/// Ejecuta `f` con las marcas en memoria (cargándolas la primera vez); si `f` devuelve `true`
/// (ha cambiado algo), se guarda el fichero
fn with_store<T>(f: impl FnOnce(&mut Store) -> (T, bool)) -> Result<T, String> {
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = guard.get_or_insert_with(|| {
        STORE_FILE
            .get()
            .and_then(|f| fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    });
    let (out, changed) = f(store);
    if changed {
        save(store)?;
    }
    Ok(out)
}

fn save(store: &Store) -> Result<(), String> {
    let Some(file) = STORE_FILE.get() else { return Ok(()) };
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    // Por un temporal: una lectura a la vez nunca ve el fichero a medias
    let tmp = file.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("Failed to save {}: {}", file.display(), e))?;
    fs::rename(&tmp, file).map_err(|e| format!("Failed to save {}: {}", file.display(), e))
}

/// Clave de un juego: ruta absoluta de su ROM, con `/` (igual que en `game_status`)
fn key(rom_dir: &Path, game_path: &str) -> String {
    rom_dir.join(strip_dot_slash(game_path)).to_string_lossy().replace('\\', "/")
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

// ---------- Lógica (sin Tauri, para poder probarla) ----------

fn misses_in(store: &Store, rom_dir: &Path, game_paths: &[String]) -> HashMap<String, ScrapeMiss> {
    game_paths
        .iter()
        .filter_map(|p| store.get(&key(rom_dir, p)).map(|m| (p.clone(), m.clone())))
        .collect()
}

/// Marca (`Some`) o desmarca (`None`) unos juegos. true = ha cambiado algo
fn set_in(store: &mut Store, rom_dir: &Path, game_paths: &[String], miss: Option<&ScrapeMiss>) -> bool {
    let mut changed = false;
    for p in game_paths {
        let k = key(rom_dir, p);
        match miss {
            Some(m) => {
                if store.get(&k) != Some(m) {
                    store.insert(k, m.clone());
                    changed = true;
                }
            }
            None => changed |= store.remove(&k).is_some(),
        }
    }
    changed
}

fn remap_in(store: &mut Store, moves: &[(String, String)]) -> bool {
    let mut changed = false;
    for (old, new) in moves {
        if old == new {
            continue;
        }
        if let Some(miss) = store.remove(old) {
            store.insert(new.clone(), miss);
            changed = true;
        }
    }
    changed
}

// ---------- Para el scraper y las operaciones que cambian rutas ----------

/// Las marcas de unos juegos de un sistema, por ruta de juego
pub fn misses_of(rom_dir: &Path, game_paths: &[String]) -> HashMap<String, ScrapeMiss> {
    with_store(|store| (misses_in(store, rom_dir, game_paths), false)).unwrap_or_default()
}

/// El scraper no encontró este juego: marca con la fecha de hoy (una marca a mano sigue a mano)
pub fn mark_not_found(rom_dir: &Path, game_path: &str) {
    let _ = with_store(|store| {
        let manual = store.get(&key(rom_dir, game_path)).is_some_and(|m| m.manual);
        ((), set_in(store, rom_dir, &[game_path.to_string()], Some(&ScrapeMiss { at: now(), manual })))
    });
}

/// El juego se ha scrapeado bien: fuera la marca
pub fn clear(rom_dir: &Path, game_path: &str) {
    let _ = with_store(|store| ((), set_in(store, rom_dir, &[game_path.to_string()], None)));
}

/// Juegos renombrados o movidos (rutas de juego `./x` de cada sistema): la marca los sigue
pub fn remap(from: &Path, to: &Path, moves: &[(String, String)]) {
    let moves: Vec<(String, String)> = moves.iter().map(|(o, n)| (key(from, o), key(to, n))).collect();
    let _ = with_store(|store| ((), remap_in(store, &moves)));
}

/// Juegos quitados de la gamelist: su marca se olvida
pub fn forget(rom_dir: &Path, game_paths: &[String]) {
    let _ = with_store(|store| ((), set_in(store, rom_dir, game_paths, None)));
}

// ---------- Comandos ----------

/// Las marcas de los juegos de un sistema (solo los marcados), por ruta de juego
#[tauri::command(async)]
pub fn get_scrape_misses(app: tauri::AppHandle, system_path: String) -> Result<HashMap<String, ScrapeMiss>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let gamelist = super::gamelist::read_gamelist(&layout, &system)?;
    let paths: Vec<String> = gamelist.games.into_iter().map(|g| g.path).collect();
    with_store(|store| (misses_in(store, &system.rom_dir, &paths), false))
}

/// Marca (`marked` = true, a mano y con la fecha de hoy) o desmarca unos juegos.
/// Devuelve las marcas de esos juegos tal como quedan.
#[tauri::command(async)]
pub fn set_scrape_misses(
    app: tauri::AppHandle,
    system_path: String,
    game_paths: Vec<String>,
    marked: bool,
) -> Result<HashMap<String, ScrapeMiss>, String> {
    let (_, system) = super::system_context(&app, &system_path)?;
    let miss = ScrapeMiss { at: now(), manual: true };
    with_store(|store| {
        let changed = set_in(store, &system.rom_dir, &game_paths, marked.then_some(&miss));
        (misses_in(store, &system.rom_dir, &game_paths), changed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn marcar_y_desmarcar() {
        let mut store = Store::new();
        let dir = Path::new("/roms/nes");
        let miss = ScrapeMiss { at: 10, manual: false };
        assert!(set_in(&mut store, dir, &paths(&["./a.nes", "./b.nes"]), Some(&miss)));
        // Lo mismo otra vez no cambia nada (no hace falta guardar)
        assert!(!set_in(&mut store, dir, &paths(&["./a.nes"]), Some(&miss)));
        let found = misses_in(&store, dir, &paths(&["./a.nes", "./b.nes", "./c.nes"]));
        assert_eq!(found.len(), 2);
        assert_eq!(found["./a.nes"], miss);
        assert!(set_in(&mut store, dir, &paths(&["./a.nes"]), None));
        assert_eq!(misses_in(&store, dir, &paths(&["./a.nes", "./b.nes"])).len(), 1);
        // Con o sin «./», la misma ROM
        assert!(store.contains_key(&key(dir, "b.nes")));
    }

    #[test]
    fn la_marca_sigue_al_juego() {
        let mut store = Store::new();
        let dir = Path::new("/roms/nes");
        set_in(&mut store, dir, &paths(&["./Billiard (En).nes"]), Some(&ScrapeMiss { at: 1, manual: true }));
        let moves = vec![(key(dir, "./Billiard (En).nes"), key(Path::new("/roms/fds"), "./Billiard.nes"))];
        assert!(remap_in(&mut store, &moves));
        assert!(misses_in(&store, dir, &paths(&["./Billiard (En).nes"])).is_empty());
        let moved = misses_in(&store, Path::new("/roms/fds"), &paths(&["./Billiard.nes"]));
        assert!(moved["./Billiard.nes"].manual);
    }
}
