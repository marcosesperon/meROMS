use std::fs;
use std::path::PathBuf;
use tauri::Manager;

use crate::models::AppSettings;

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
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read settings: {}", e))?;
    let mut settings: AppSettings = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings: {}", e))?;

    // Old settings files have no profiles: convert them once and persist the result
    // (y la proporción por sistema de la antigua pestaña Sistemas pasa a cada perfil)
    let legacy = settings.migrate_legacy();
    if settings.migrate_system_configs() || legacy {
        save_settings(app, settings.clone())?;
    }
    // Las escrituras de gamelist no reciben los ajustes: el orden se les deja a mano
    super::gamelist::set_sort(settings.gamelist_sort.as_deref());
    Ok(settings)
}

#[tauri::command(async)]
pub fn save_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    let path = settings_path(&app)?;
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Failed to write settings: {}", e))?;
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
