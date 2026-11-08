use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

use crate::models::AppSettings;

/// Lecturas y escrituras de `settings.json` van en fila: los comandos corren en hilos y, sin
/// esto, una lectura podía coger el fichero a medio escribir (p. ej. al arrancar, dos lecturas
/// a la vez y una de ellas guardando una migración) y la app creía que no había perfil.
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

/// Lee los ajustes del fichero (sin fichero, los de por defecto). Hay que tener el candado.
fn read_file(path: &Path) -> Result<AppSettings, String> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read settings: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse settings: {}", e))
}

/// Escribe los ajustes de golpe: primero en `settings.json.tmp` y luego se cambia por el bueno,
/// así nunca queda (ni se lee) un fichero a medias. Hay que tener el candado.
fn write_file(path: &Path, settings: &AppSettings) -> Result<(), String> {
    let content = serde_json::to_string_pretty(settings).map_err(|e| format!("Failed to serialize settings: {}", e))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, content).map_err(|e| format!("Failed to write settings: {}", e))?;
    fs::rename(&tmp, path).map_err(|e| format!("Failed to write settings: {}", e))
}

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create app data dir: {}", e))?;
    Ok(dir.join("settings.json"))
}

#[tauri::command(async)]
pub fn get_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let path = settings_path(&app)?;
    let _lock = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut settings = read_file(&path)?;

    // Old settings files have no profiles: convert them once and persist the result
    // (y la proporción por sistema de la antigua pestaña Sistemas pasa a cada perfil)
    let legacy = settings.migrate_legacy();
    if settings.migrate_system_configs() || legacy {
        write_file(&path, &settings)?;
    }
    // Las escrituras de gamelist no reciben los ajustes: el orden se les deja a mano
    super::gamelist::set_sort(settings.gamelist_sort.as_deref());
    Ok(settings)
}

#[tauri::command(async)]
pub fn save_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    let path = settings_path(&app)?;
    {
        let _lock = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        write_file(&path, &settings)?;
    }
    super::gamelist::set_sort(settings.gamelist_sort.as_deref());
    Ok(())
}

#[tauri::command(async)]
pub fn export_settings(app: tauri::AppHandle, dest_path: String) -> Result<(), String> {
    let settings = get_settings(app)?;
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&dest_path, content)
        .map_err(|e| format!("Failed to write export file: {}", e))
}

#[tauri::command(async)]
pub fn import_settings(file_path: String) -> Result<AppSettings, String> {
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read import file: {}", e))?;
    let mut settings: AppSettings = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid settings file: {}", e))?;
    // Exports made before profiles existed are migrated the same way
    settings.migrate_legacy();
    settings.migrate_system_configs();
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_while_another_thread_saves_never_sees_half_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        let mut settings = AppSettings::default();
        // Ajustes grandes: así escribirlos tarda y es fácil pillarlo a medias sin el arreglo
        settings.profiles = (0..300).map(|i| crate::models::Profile::default_classic(&format!("/roms/{i}"), None)).collect();
        write_file(&path, &settings).unwrap();

        let writer_path = path.clone();
        let writer_settings = settings.clone();
        let writer = std::thread::spawn(move || {
            for _ in 0..200 {
                let _lock = SETTINGS_LOCK.lock().unwrap();
                write_file(&writer_path, &writer_settings).unwrap();
            }
        });
        for _ in 0..200 {
            let _lock = SETTINGS_LOCK.lock().unwrap();
            assert_eq!(read_file(&path).unwrap().profiles.len(), 300);
        }
        writer.join().unwrap();
        assert!(!path.with_extension("json.tmp").exists(), "no temporary file is left behind");
    }
}
