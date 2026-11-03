use std::fs;
use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::layout::SystemRef;
use crate::models::system::{extensions_for_system, system_full_name, SystemInfo};
use crate::parser;

#[derive(Clone, Serialize)]
pub struct ScanProgress {
    pub current: usize,
    pub total: usize,
    pub current_name: String,
}

/// Count ROM files in a directory based on the system's known extensions.
fn count_roms(dir: &Path, folder_name: &str) -> usize {
    let extensions = extensions_for_system(folder_name);
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };

    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            if !path.is_file() {
                return false;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                return false;
            };
            let dot_ext = format!(".{}", ext.to_lowercase());
            extensions.contains(&dot_ext.as_str())
        })
        .count()
}

#[tauri::command]
pub fn scan_systems(root_path: String, app: AppHandle) -> Result<Vec<SystemInfo>, String> {
    let root = Path::new(&root_path);
    if !root.is_dir() {
        return Err(format!("Path is not a directory: {}", root_path));
    }
    let layout = super::active_layout(&app)?;

    // First pass: collect valid directories to get total count
    let dirs: Vec<_> = fs::read_dir(root)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            if !path.is_dir() {
                return false;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // ES-DE's own folders (gamelists/, downloaded_media/...) when it lives in the ROM folder
            !name.starts_with('.') && !layout.is_own_folder(&path)
        })
        .collect();

    let total = dirs.len();
    let mut systems = Vec::new();

    for (i, entry) in dirs.into_iter().enumerate() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let full_name = system_full_name(&name)
            .unwrap_or(&name)
            .to_string();

        // Emit progress event
        let _ = app.emit("scan-progress", ScanProgress {
            current: i + 1,
            total,
            current_name: full_name.clone(),
        });

        // Classic: <system>/gamelist.xml — ES-DE: <ES-DE>/gamelists/<system>/gamelist.xml
        let gamelist_path = layout.gamelist_path(&SystemRef::from_rom_dir(&path));
        let has_gamelist = gamelist_path.exists();

        let game_count = if has_gamelist {
            match fs::read_to_string(&gamelist_path) {
                Ok(xml) => parser::parse_gamelist(&xml)
                    .map(|gl| gl.games.len())
                    .unwrap_or(0),
                Err(_) => 0,
            }
        } else {
            0
        };

        let rom_count = count_roms(&path, &name);

        systems.push(SystemInfo {
            name,
            full_name,
            path: path.to_string_lossy().to_string(),
            has_gamelist,
            game_count,
            rom_count,
        });
    }

    systems.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(systems)
}
