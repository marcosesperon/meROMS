//! Estados de juego (idea de RomM): un estado de partida (incompleto, terminado, completado al
//! 100 %, abandonado, nunca jugar) y dos marcas sueltas, «jugando» y «pendiente».
//!
//! La gamelist no tiene sitio para esto, y ES-DE rehace la entrada de un juego entera (solo con
//! sus campos) cada vez que cambia, así que una etiqueta propia se perdería al jugar. Por eso se
//! guardan en meROMS (`game_status.json` en la carpeta de datos de la app) y a la gamelist solo
//! va lo que ES-DE entiende: `<completed>` = terminado o completado.
//!
//! La clave es la ruta absoluta de la ROM: no cambia al migrar entre estructuras (las ROMs no se
//! tocan) y vale para todos los perfiles con la misma carpeta de ROMs. Cuando meROMS cambia la
//! ruta de un juego (renombrar, mover, CHD, deshacer) se llama a `remap`; al quitarlo, `forget`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::layout::{strip_dot_slash, Layout, SystemRef};
use crate::models::Game;

/// Estados de partida posibles (uno a la vez)
pub const STATUSES: &[&str] = &["incomplete", "finished", "completed", "abandoned", "never"];
/// Los que cuentan como «terminado» para el `<completed>` de la gamelist
const DONE: &[&str] = &["finished", "completed"];

/// Estado de un juego. Vacío = no se guarda.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GameStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub playing: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub backlog: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl GameStatus {
    fn is_empty(&self) -> bool {
        self.status.is_none() && !self.playing && !self.backlog
    }
    fn is_done(&self) -> bool {
        self.status.as_deref().is_some_and(|s| DONE.contains(&s))
    }
}

/// Qué cambiar en varios juegos a la vez. `None` = dejarlo como está.
/// En `status`, `Some(None)` quita el estado; `Some(Some(x))` lo pone.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatusChange {
    #[serde(default, deserialize_with = "double_option")]
    pub status: Option<Option<String>>,
    #[serde(default)]
    pub playing: Option<bool>,
    #[serde(default)]
    pub backlog: Option<bool>,
}

/// Distingue «campo ausente» (None) de «campo a null» (Some(None))
fn double_option<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Ok(Some(Option::<String>::deserialize(d)?))
}

// ---------- Almacén ----------

type Store = HashMap<String, GameStatus>;

static STORE_FILE: OnceLock<PathBuf> = OnceLock::new();
static STORE: Mutex<Option<Store>> = Mutex::new(None);

/// Al arrancar: dónde vive el fichero (carpeta de datos de la app)
pub fn init(app: &tauri::AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = STORE_FILE.set(dir.join("game_status.json"));
    }
}

/// Ejecuta `f` con los estados en memoria (cargándolos la primera vez); si `f` devuelve
/// `true` (ha cambiado algo), se guarda el fichero
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
    fs::write(file, json).map_err(|e| format!("Failed to save {}: {}", file.display(), e))
}

/// Clave de un juego: ruta absoluta de su ROM, con `/` (igual en Windows al leer y al escribir)
fn key(rom_dir: &Path, game_path: &str) -> String {
    rom_dir.join(strip_dot_slash(game_path)).to_string_lossy().replace('\\', "/")
}

// ---------- Lógica (sin Tauri, para poder probarla) ----------

/// Estados de los juegos de una gamelist, por ruta de juego. Lo marcado como `completed` en la
/// gamelist (en ES-DE, por ejemplo) y sin estado de terminado en meROMS pasa a «finished».
fn statuses_for(store: &mut Store, rom_dir: &Path, games: &[Game]) -> (HashMap<String, GameStatus>, bool) {
    let mut out = HashMap::new();
    let mut changed = false;
    for game in games {
        let k = key(rom_dir, &game.path);
        let mut status = store.get(&k).cloned().unwrap_or_default();
        if game.completed.as_deref() == Some("true") && !status.is_done() {
            status.status = Some("finished".into());
            store.insert(k, status.clone());
            changed = true;
        }
        if !status.is_empty() {
            out.insert(game.path.clone(), status);
        }
    }
    (out, changed)
}

/// Aplica un cambio a unos juegos. Devuelve los estados nuevos de esos juegos y si algún
/// `<completed>` de la gamelist tiene que cambiar (ya cambiado en `games`).
fn apply_change(store: &mut Store, rom_dir: &Path, games: &mut [Game], game_paths: &[String], change: &StatusChange) -> (HashMap<String, GameStatus>, bool) {
    let mut out = HashMap::new();
    let mut gamelist_changed = false;
    for game in games.iter_mut().filter(|g| game_paths.contains(&g.path)) {
        let k = key(rom_dir, &game.path);
        let mut status = store.get(&k).cloned().unwrap_or_default();
        if let Some(s) = &change.status {
            status.status = s.clone().filter(|s| STATUSES.contains(&s.as_str()));
        }
        if let Some(p) = change.playing {
            status.playing = p;
        }
        if let Some(b) = change.backlog {
            status.backlog = b;
        }
        // La gamelist: «completed» sí o no según el estado
        let completed = if status.is_done() { "true" } else { "false" };
        if game.completed.as_deref() != Some(completed) {
            // Sin estado de terminado y sin «completed» en la gamelist: no hace falta escribir «false»
            if !(completed == "false" && game.completed.is_none()) {
                game.completed = Some(completed.into());
                gamelist_changed = true;
            }
        }
        if status.is_empty() {
            store.remove(&k);
        } else {
            store.insert(k, status.clone());
        }
        out.insert(game.path.clone(), status);
    }
    (out, gamelist_changed)
}

/// El juego ha cambiado de sitio: su estado lo sigue
fn remap_in(store: &mut Store, moves: &[(String, String)]) -> bool {
    let mut changed = false;
    for (old, new) in moves {
        if old == new {
            continue;
        }
        if let Some(status) = store.remove(old) {
            store.insert(new.clone(), status);
            changed = true;
        }
    }
    changed
}

// ---------- Para las operaciones que cambian rutas ----------

/// Juegos renombrados o movidos (rutas de juego `./x` de cada sistema)
pub fn remap(from: &Path, to: &Path, moves: &[(String, String)]) {
    let moves: Vec<(String, String)> = moves.iter().map(|(o, n)| (key(from, o), key(to, n))).collect();
    let _ = with_store(|store| ((), remap_in(store, &moves)));
}

/// Juegos quitados de la gamelist: su estado se olvida
pub fn forget(rom_dir: &Path, game_paths: &[String]) {
    let _ = with_store(|store| {
        let before = store.len();
        for p in game_paths {
            store.remove(&key(rom_dir, p));
        }
        ((), store.len() != before)
    });
}

// ---------- Comandos ----------

/// Estados de los juegos de un sistema (solo los que tienen alguno), por ruta de juego
#[tauri::command(async)]
pub fn get_game_statuses(app: tauri::AppHandle, system_path: String) -> Result<HashMap<String, GameStatus>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let gamelist = super::gamelist::read_gamelist(&layout, &system)?;
    with_store(|store| statuses_for(store, &system.rom_dir, &gamelist.games))
}

/// Cambia el estado de unos juegos y lleva `<completed>` a la gamelist (con copia) si cambia.
/// Devuelve el estado nuevo de cada uno.
#[tauri::command(async)]
pub fn set_game_statuses(
    app: tauri::AppHandle,
    system_path: String,
    game_paths: Vec<String>,
    change: StatusChange,
) -> Result<HashMap<String, GameStatus>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    set_statuses(&layout, &system, &game_paths, &change)
}

fn set_statuses(layout: &Layout, system: &SystemRef, game_paths: &[String], change: &StatusChange) -> Result<HashMap<String, GameStatus>, String> {
    let mut gamelist = super::gamelist::read_gamelist(layout, system)?;
    let (out, gamelist_changed) = with_store(|store| {
        let (out, gamelist_changed) = apply_change(store, &system.rom_dir, &mut gamelist.games, game_paths, change);
        ((out, gamelist_changed), true)
    })?;
    if gamelist_changed {
        super::gamelist::write_gamelist(layout, system, &gamelist)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(path: &str, completed: Option<&str>) -> Game {
        Game { path: path.into(), completed: completed.map(String::from), ..Default::default() }
    }

    #[test]
    fn completed_en_la_gamelist_pasa_a_terminado() {
        let mut store = Store::new();
        let dir = Path::new("/roms/snes");
        let games = vec![game("./a.sfc", Some("true")), game("./b.sfc", Some("false")), game("./c.sfc", None)];
        let (out, changed) = statuses_for(&mut store, dir, &games);
        assert!(changed);
        assert_eq!(out.len(), 1);
        assert_eq!(out["./a.sfc"].status.as_deref(), Some("finished"));
        assert_eq!(store["/roms/snes/a.sfc"].status.as_deref(), Some("finished"));
        // Ya terminado al 100 %: se respeta
        store.insert("/roms/snes/a.sfc".into(), GameStatus { status: Some("completed".into()), ..Default::default() });
        let (out, changed) = statuses_for(&mut store, dir, &games);
        assert!(!changed);
        assert_eq!(out["./a.sfc"].status.as_deref(), Some("completed"));
    }

    #[test]
    fn el_cambio_escribe_completed_y_las_marcas() {
        let mut store = Store::new();
        let dir = Path::new("/roms/snes");
        let mut games = vec![game("./a.sfc", None), game("./b.sfc", None)];
        let paths = vec!["./a.sfc".to_string()];

        let change = StatusChange { status: Some(Some("completed".into())), playing: Some(true), backlog: None };
        let (out, gl) = apply_change(&mut store, dir, &mut games, &paths, &change);
        assert!(gl);
        assert_eq!(games[0].completed.as_deref(), Some("true"));
        assert_eq!(games[1].completed, None);
        assert_eq!(out["./a.sfc"], GameStatus { status: Some("completed".into()), playing: true, backlog: false });

        // Abandonado: completed pasa a false; «jugando» se queda como estaba
        let change = StatusChange { status: Some(Some("abandoned".into())), ..Default::default() };
        let (out, gl) = apply_change(&mut store, dir, &mut games, &paths, &change);
        assert!(gl);
        assert_eq!(games[0].completed.as_deref(), Some("false"));
        assert!(out["./a.sfc"].playing);

        // Quitarlo todo: fuera del almacén
        let change = StatusChange { status: Some(None), playing: Some(false), backlog: Some(false) };
        apply_change(&mut store, dir, &mut games, &paths, &change);
        assert!(store.is_empty());
    }

    #[test]
    fn solo_marcas_no_tocan_la_gamelist() {
        let mut store = Store::new();
        let mut games = vec![game("./a.sfc", None)];
        let change = StatusChange { backlog: Some(true), ..Default::default() };
        let (_, gl) = apply_change(&mut store, Path::new("/r"), &mut games, &["./a.sfc".into()], &change);
        assert!(!gl);
        assert_eq!(games[0].completed, None);
    }

    #[test]
    fn el_estado_sigue_al_juego() {
        let mut store = Store::new();
        store.insert("/roms/snes/a.sfc".into(), GameStatus { backlog: true, ..Default::default() });
        let moves = vec![(key(Path::new("/roms/snes"), "./a.sfc"), key(Path::new("/roms/sfc"), "./Sub/A.sfc"))];
        assert!(remap_in(&mut store, &moves));
        assert!(store.contains_key("/roms/sfc/Sub/A.sfc"));
        assert!(!store.contains_key("/roms/snes/a.sfc"));
    }

    #[test]
    fn deserializa_quitar_estado() {
        let c: StatusChange = serde_json::from_str(r#"{"status": null}"#).unwrap();
        assert_eq!(c.status, Some(None));
        let c: StatusChange = serde_json::from_str(r#"{"playing": true}"#).unwrap();
        assert_eq!(c.status, None);
        assert_eq!(c.playing, Some(true));
    }
}
