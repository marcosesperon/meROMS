//! Vigilar las carpetas de ROMs: meROMS se entera solo de las ROMs nuevas (no están en el
//! gamelist) y de las desaparecidas (el gamelist las tiene, el disco no), sin entrar en el sistema.
//! Solo avisa; añadir o quitar sigue siendo cosa del usuario («ROMs nuevas» / «Sin ROM»).
//!
//! - `rom_changes`: cuenta, por sistema, las nuevas y las desaparecidas (con las mismas funciones
//!   que esos dos botones, así que los números son los mismos).
//! - `watch_roms`: vigila en tiempo real la carpeta de ROMs del perfil activo (`notify`). Cuando
//!   algo cambia, espera a que se calme (`QUIET`) y emite `roms-changed` con los sistemas tocados.
//!   Lo que no son ROMs (gamelist, copias, medios, ocultos, temporales) no cuenta: si no, cada
//!   guardado de meROMS se avisaría a sí mismo. La app hace además un repaso al volver a la
//!   ventana, por si la vigilancia falla (discos de red o USB).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::Mutex;
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::Emitter;

use super::new_roms;
use super::progress::Progress;
use crate::layout::{Layout, SystemRef};
use crate::rename_rules::CompiledRules;

/// Tiempo sin cambios antes de avisar (copiar 50 ROMs no da 50 avisos)
const QUIET: Duration = Duration::from_secs(2);

/// La vigilancia en marcha (al soltarla, se para y su hilo termina)
static WATCHER: Mutex<Option<RecommendedWatcher>> = Mutex::new(None);

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct RomChanges {
    /// ROMs de la carpeta que no están en el gamelist
    pub new_roms: usize,
    /// Juegos del gamelist cuya ROM ya no está
    pub missing: usize,
}

/// ROMs nuevas y desaparecidas de cada sistema dado (los del perfil activo con gamelist)
#[tauri::command(async)]
pub fn rom_changes(app: tauri::AppHandle, system_paths: Vec<String>) -> Result<HashMap<String, RomChanges>, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let layout = super::active_layout(&app)?;
    let rules = CompiledRules::new(&settings.rename_rules_or_default());
    let mut out = HashMap::new();
    for path in system_paths {
        let system = SystemRef::from_rom_dir(&path);
        // Un sistema que no se puede leer no rompe el resto
        if let Ok(changes) = changes_of(&layout, &system, &rules) {
            out.insert(path, changes);
        }
    }
    Ok(out)
}

fn changes_of(layout: &Layout, system: &SystemRef, rules: &CompiledRules) -> Result<RomChanges, String> {
    let games = super::gamelist::read_gamelist(layout, system)?.games;
    Ok(RomChanges {
        new_roms: new_roms::find_new_roms(system, &games, rules).len(),
        missing: new_roms::find_missing(system, &games, &Progress::none())?.games.len(),
    })
}

/// Arranca (o para, con `enabled` false) la vigilancia de la carpeta de ROMs del perfil activo.
/// Llamarla otra vez la rehace (al cambiar de perfil o el ajuste).
#[tauri::command(async)]
pub fn watch_roms(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut slot = WATCHER.lock().unwrap();
    *slot = None;
    if !enabled {
        return Ok(());
    }
    let settings = super::settings::get_settings(app.clone())?;
    let Some(profile) = settings.active_profile() else { return Ok(()) };
    let root = PathBuf::from(&profile.rom_root_path);
    if !root.is_dir() {
        return Ok(());
    }
    let layout = super::active_layout(&app)?;

    let (tx, rx) = channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        // Leer un fichero no cambia nada; el resto (crear, borrar, renombrar, escribir) sí
        let Ok(event) = res else { return };
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        for path in event.paths {
            let _ = tx.send(path);
        }
    })
    .map_err(|e| format!("Can't watch the ROM folder: {}", e))?;
    watcher.watch(&root, RecursiveMode::Recursive).map_err(|e| format!("Can't watch the ROM folder: {}", e))?;

    let skipped = new_roms::skipped_dirs();
    // Las rutas de los avisos vienen sin enlaces simbólicos (macOS): se comparan con la canónica
    let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
    std::thread::spawn(move || {
        let system_of = |path: &Path| {
            system_of(&canonical, path, &skipped)
                .filter(|name| !layout.is_own_folder(&root.join(name)))
                .map(|name| root.join(name).to_string_lossy().to_string())
        };
        notify_loop(&rx, system_of, |systems| {
            let _ = app.emit("roms-changed", systems);
        });
    });
    *slot = Some(watcher);
    Ok(())
}

/// Junta los cambios hasta que hay `QUIET` de calma y entonces avisa con los sistemas tocados.
/// Termina cuando se suelta la vigilancia (el canal se cierra).
fn notify_loop(rx: &Receiver<PathBuf>, system_of: impl Fn(&Path) -> Option<String>, emit: impl Fn(Vec<String>)) {
    while let Ok(first) = rx.recv() {
        let mut systems: BTreeSet<String> = system_of(&first).into_iter().collect();
        loop {
            match rx.recv_timeout(QUIET) {
                Ok(path) => systems.extend(system_of(&path)),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        if !systems.is_empty() {
            emit(systems.into_iter().collect());
        }
    }
}

/// El sistema (carpeta de primer nivel) de un cambio en `root`, si puede ser una ROM: dentro de
/// una carpeta de sistema, fuera de carpetas ocultas o de medios, y sin ser gamelist, copia,
/// colección ni temporal. Un cambio en la propia carpeta de ROMs (un sistema nuevo) no cuenta.
fn system_of(root: &Path, path: &Path, skipped_dirs: &HashSet<String>) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    if parts.len() < 2 {
        return None;
    }
    let (file, folders) = parts.split_last()?;
    if folders.iter().any(|d| d.starts_with('.')) || folders[1..].iter().any(|d| skipped_dirs.contains(&d.to_lowercase())) {
        return None;
    }
    let lower = file.to_lowercase();
    let not_rom = lower.starts_with('.')
        || lower == "gamelist.xml"
        || lower.ends_with(".tmp")
        || lower.contains(".meroms-tmp")
        || (lower.starts_with("custom-") && lower.ends_with(".cfg"));
    if not_rom {
        return None;
    }
    Some(folders[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn skipped() -> HashSet<String> {
        ["images", "videos", "media"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn sistema_de_cada_cambio() {
        let root = Path::new("/roms");
        let of = |p: &str| system_of(root, Path::new(p), &skipped());
        assert_eq!(of("/roms/snes/Mario.sfc"), Some("snes".into()));
        assert_eq!(of("/roms/psx/Final Fantasy/disc1.chd"), Some("psx".into()));
        // Lo que guarda meROMS o no son ROMs
        assert_eq!(of("/roms/snes/gamelist.xml"), None);
        assert_eq!(of("/roms/snes/gamelist.xml.tmp"), None);
        assert_eq!(of("/roms/snes/.backups/gamelist_1.xml"), None);
        assert_eq!(of("/roms/snes/images/Mario.png"), None);
        assert_eq!(of("/roms/snes/custom-rpg.cfg"), None);
        assert_eq!(of("/roms/psx/Game.meroms-tmp.chd"), None);
        assert_eq!(of("/roms/snes/.DS_Store"), None);
        // Fuera de un sistema, o una carpeta de sistema nueva
        assert_eq!(of("/roms/snes"), None);
        assert_eq!(of("/elsewhere/snes/Mario.sfc"), None);
        assert_eq!(of("/roms/.hidden/Mario.sfc"), None);
    }

    #[test]
    fn junta_los_cambios_y_avisa_una_vez() {
        let (tx, rx) = channel::<PathBuf>();
        let emitted = std::sync::Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let sink = emitted.clone();
        let handle = std::thread::spawn(move || {
            let root = Path::new("/roms");
            let skip = skipped();
            notify_loop(&rx, |p| system_of(root, p, &skip), |s| sink.lock().unwrap().push(s));
        });
        for p in ["/roms/snes/A.sfc", "/roms/snes/B.sfc", "/roms/nes/C.nes", "/roms/snes/gamelist.xml"] {
            tx.send(PathBuf::from(p)).unwrap();
        }
        std::thread::sleep(QUIET + Duration::from_millis(500));
        drop(tx);
        handle.join().unwrap();
        assert_eq!(*emitted.lock().unwrap(), vec![vec!["nes".to_string(), "snes".to_string()]]);
    }
}
