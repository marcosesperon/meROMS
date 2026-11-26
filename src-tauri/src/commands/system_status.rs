//! Estado del sistema: el resultado completo de cada comprobación, guardado.
//!
//! Las comprobaciones son las de cada herramienta: versiones y duplicados (`find_duplicates`),
//! medios dañados, placeholders, sobrantes, repetidos, rutas rotas, carátulas y capturas
//! cambiadas, nombres antiguos y la vista previa de Renombrar. Cada comando, cuando mira el
//! sistema entero, pasa por `remember`: si la «firma» del sistema (tamaño y fecha de la gamelist;
//! entradas y fecha de la carpeta de ROMs, de la de medios y de sus subcarpetas; fecha del DAT)
//! y la huella de los ajustes y de las cachés de CRC y de nombres no han cambiado, devuelve lo
//! guardado (en memoria y en `status_cache/` en disco) sin leer ningún medio. Así el estado del
//! panel (que la interfaz monta pidiendo las comprobaciones a la vez) sale al momento al entrar
//! en un sistema ya revisado, y «Ver» abre cada herramienta sin volver a buscar.
//! `forget_system_status` («Volver a revisar», o tras scrapear: un fichero sobrescrito con el
//! mismo nombre no cambia la firma) lo descarta.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::layout::{Layout, SystemRef};


/// Lo que cambia cuando se añade, quita o renombra algo en una carpeta: entradas y fecha
type FolderSig = (usize, u64);

/// Sube cuando cambia lo que se comprueba (p. ej. Normalizar también busca por el título):
/// lo guardado con otra versión no vale y se vuelve a mirar
const STATUS_VERSION: u32 = 6;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Signature {
    #[serde(default)]
    version: u32,
    gamelist: (u64, u64),
    folders: Vec<(String, FolderSig)>,
    dat: u64,
}

/// Fecha de modificación en nanosegundos: con segundos, un cambio hecho en el mismo segundo que
/// la comprobación no cambiaba la firma y se daba por bueno lo guardado
fn modified_secs(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64)
}

fn folder_sig(dir: &Path) -> FolderSig {
    (fs::read_dir(dir).map(|it| it.count()).unwrap_or(0), modified_secs(dir))
}

/// Firma: la gamelist, la carpeta de ROMs y sus subcarpetas, la carpeta de medios y las suyas
/// (una lectura por carpeta, ningún fichero abierto) y el DAT guardado
fn signature(layout: &Layout, system: &SystemRef, dat_file: Option<&Path>) -> Signature {
    let gamelist = layout.gamelist_path(system);
    let size = fs::metadata(&gamelist).map(|m| m.len()).unwrap_or(0);
    let mut folders = Vec::new();
    let media_base = layout.media_type_dir(system, "");
    let mut roots = vec![system.rom_dir.clone()];
    if media_base != system.rom_dir {
        roots.push(media_base);
    }
    for root in roots {
        folders.push((root.to_string_lossy().to_string(), folder_sig(&root)));
        let Ok(entries) = fs::read_dir(&root) else { continue };
        let mut subdirs: Vec<_> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
        subdirs.sort();
        for dir in subdirs {
            folders.push((dir.to_string_lossy().to_string(), folder_sig(&dir)));
        }
    }
    Signature { version: STATUS_VERSION, gamelist: (size, modified_secs(&gamelist)), folders, dat: dat_file.map_or(0, modified_secs) }
}

// ---------- Resultados completos de cada comprobación ----------
//
// El estado solo enseña números, pero para sacarlos cada comprobación calcula la lista entera
// (qué grupos de versiones, qué ficheros dañados…). Esa lista se guarda aquí, en memoria y en
// disco (`status_cache/`), con la firma del sistema y una huella de los ajustes: cuando una
// ventana («Ver», «Ponerlo todo en orden») pide lo mismo y nada ha cambiado, sale al momento.

/// Lo guardado de una comprobación: firma del sistema, huella de los ajustes y el resultado
#[derive(Clone, Serialize, Deserialize)]
struct Remembered {
    signature: Signature,
    settings: u64,
    value: serde_json::Value,
}

static RESULTS: Mutex<Option<HashMap<String, Remembered>>> = Mutex::new(None);

fn hash_of(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Carpeta de disco de un sistema (así «Volver a revisar» la borra entera)
fn system_cache_dir(app: &tauri::AppHandle, system_path: &str) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("status_cache").join(format!("{:016x}", hash_of(system_path))))
}

/// Fichero de disco de una comprobación (nombre = huella de la clave)
fn remembered_file(app: &tauri::AppHandle, system_path: &str, key: &str) -> Option<std::path::PathBuf> {
    system_cache_dir(app, system_path).map(|d| d.join(format!("{:016x}.json", hash_of(key))))
}

/// Huella de lo que no está en la carpeta del sistema pero cambia los resultados: los ajustes
/// (regiones, rutas de medios, reglas…), la caché de CRC («Comprobar por contenido»: versiones
/// por DAT, nombres del DAT) y la de nombres de ScreenScraper (Renombrar)
fn settings_hash(app: &tauri::AppHandle) -> u64 {
    let settings = super::settings::get_settings(app.clone()).ok().and_then(|s| serde_json::to_string(&s).ok()).unwrap_or_default();
    let caches = app
        .path()
        .app_data_dir()
        .map(|d| format!("{}|{}", modified_secs(&d.join("crc_cache.json")), modified_secs(&d.join("ss_rom_names.json"))))
        .unwrap_or_default();
    hash_of(&format!("{}\n{}", settings, caches))
}

/// Devuelve lo guardado de esta comprobación (`kind` + `extra`, p. ej. las opciones de Renombrar)
/// si el sistema y los ajustes no han cambiado; si no, la hace con `compute` y la guarda. Un
/// error (o «cancelled») no se guarda.
pub(super) fn remember<T>(app: &tauri::AppHandle, system_path: &str, kind: &str, extra: &str, compute: impl FnOnce() -> Result<T, String>) -> Result<T, String>
where
    T: Serialize + serde::de::DeserializeOwned,
{
    let Ok((layout, system)) = super::system_context(app, system_path) else { return compute() };
    let key = format!("{}\n{}\n{}\n{}", layout.gamelist_path(&system).to_string_lossy(), system_path, kind, extra);
    let dat = super::dat::dat_file(app, &system.name).ok();
    let sig = signature(&layout, &system, dat.as_deref());
    let settings = settings_hash(app);
    let valid = |r: &Remembered| r.signature == sig && r.settings == settings;

    // En memoria y, si no, en disco (de otra sesión)
    let in_memory = RESULTS.lock().ok().and_then(|m| m.as_ref().and_then(|m| m.get(&key).cloned()));
    let saved = in_memory.or_else(|| {
        let text = fs::read_to_string(remembered_file(app, system_path, &key)?).ok()?;
        serde_json::from_str::<Remembered>(&text).ok()
    });
    if let Some(saved) = saved.filter(valid) {
        if let Ok(value) = serde_json::from_value::<T>(saved.value.clone()) {
            if let Ok(mut m) = RESULTS.lock() {
                m.get_or_insert_with(HashMap::new).insert(key, saved);
            }
            return Ok(value);
        }
    }

    let result = compute()?;
    if let Ok(value) = serde_json::to_value(&result) {
        // La firma de después de mirar: si algo cambió mientras, la próxima vez se rehace
        let entry = Remembered { signature: signature(&layout, &system, dat.as_deref()), settings, value };
        if let Some(file) = remembered_file(app, system_path, &key) {
            if let Some(dir) = file.parent() {
                let _ = fs::create_dir_all(dir);
            }
            if let Ok(text) = serde_json::to_string(&entry) {
                let _ = fs::write(file, text);
            }
        }
        if let Ok(mut m) = RESULTS.lock() {
            m.get_or_insert_with(HashMap::new).insert(key, entry);
        }
    }
    Ok(result)
}

/// Olvida todo lo guardado de un sistema («Volver a revisar», o tras scrapear: un fichero
/// sobrescrito con el mismo nombre no cambia la firma)
fn forget(app: &tauri::AppHandle, system_path: &str) {
    let marker = format!("\n{}\n", system_path);
    if let Ok(mut m) = RESULTS.lock() {
        if let Some(map) = m.as_mut() {
            map.retain(|k, _| !k.contains(&marker));
        }
    }
    if let Some(dir) = system_cache_dir(app, system_path) {
        let _ = fs::remove_dir_all(dir);
    }
}

/// «Volver a revisar» (o tras scrapear): olvida lo guardado del sistema; lo siguiente que se
/// pida se vuelve a mirar
#[tauri::command(async)]
pub fn forget_system_status(app: tauri::AppHandle, system_path: String) {
    forget(&app, &system_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn the_signature_changes_when_a_media_file_is_added_or_the_gamelist_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("nes"));
        touch(&system.rom_dir.join("Juego.nes"));
        touch(&system.rom_dir.join("covers/Juego.png"));
        touch(&system.rom_dir.join("gamelist.xml"));
        let first = signature(&Layout::Classic, &system, None);
        assert_eq!(first, signature(&Layout::Classic, &system, None), "nothing changed, same signature");
        touch(&system.rom_dir.join("covers/Otro.png"));
        assert_ne!(first, signature(&Layout::Classic, &system, None), "a new cover changes it");
        let second = signature(&Layout::Classic, &system, None);
        fs::write(system.rom_dir.join("gamelist.xml"), b"<gameList></gameList>").unwrap();
        assert_ne!(second, signature(&Layout::Classic, &system, None), "a different gamelist changes it");
    }

}
