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

/// Una carpeta de medios del sistema con cuántos ficheros tiene
#[derive(Clone, Serialize)]
pub struct MediaFolderInfo {
    pub folder: String,
    pub path: String,
    pub files: usize,
}

/// Dónde está cada cosa de un sistema según el perfil activo (para enseñarlo en su ficha)
#[derive(Clone, Serialize)]
pub struct SystemLocations {
    /// "classic" | "esde"
    pub structure: String,
    pub rom_dir: String,
    pub gamelist_path: String,
    pub gamelist_exists: bool,
    pub backups_dir: String,
    /// Carpeta base de los medios (en clásica, la propia carpeta de ROMs)
    pub media_dir: String,
    /// Carpetas de medios que existen, con su número de ficheros
    pub media_folders: Vec<MediaFolderInfo>,
    pub collections_dir: String,
}

#[tauri::command]
pub fn system_locations(app: AppHandle, system_path: String) -> Result<SystemLocations, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    Ok(locations(&layout, &system))
}

fn locations(layout: &crate::layout::Layout, system: &SystemRef) -> SystemLocations {
    let text = |p: &Path| p.to_string_lossy().to_string();
    let gamelist = layout.gamelist_path(system);

    // Carpetas de los tipos de medio y, en clásica, también las de nombres antiguos (images/…)
    let mut folders: Vec<String> = crate::media::MEDIA_KINDS.iter().map(|k| k.folder.to_string()).collect();
    if layout.writes_media_tags() {
        for (dirs, _, _) in super::images::legacy_targets() {
            for d in dirs {
                if !folders.iter().any(|f| f == d) {
                    folders.push(d.to_string());
                }
            }
        }
    }
    let media_folders = folders
        .into_iter()
        .filter_map(|folder| {
            let path = layout.media_type_dir(system, &folder);
            path.is_dir().then(|| MediaFolderInfo {
                files: crate::media::list_files_recursive(&path).len(),
                path: text(&path),
                folder,
            })
        })
        .collect();

    SystemLocations {
        structure: if layout.writes_media_tags() { "classic" } else { "esde" }.to_string(),
        rom_dir: text(&system.rom_dir),
        gamelist_exists: gamelist.is_file(),
        gamelist_path: text(&gamelist),
        backups_dir: text(&layout.backups_dir(system)),
        // La carpeta que contiene las de cada tipo (sin la barra final que dejaría un nombre vacío)
        media_dir: layout.media_type_dir(system, "covers").parent().map(text).unwrap_or_default(),
        media_folders,
        collections_dir: text(&layout.collections_dir(system)),
    }
}

#[cfg(test)]
mod location_tests {
    use super::*;
    use crate::layout::{EsdePaths, Layout};

    #[test]
    fn esde_locations_point_to_the_esde_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Esde(EsdePaths { home: tmp.path().join("ES-DE"), media_root: tmp.path().join("ES-DE/downloaded_media") });
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/cps2"));
        fs::create_dir_all(tmp.path().join("ES-DE/downloaded_media/cps2/covers")).unwrap();
        fs::write(tmp.path().join("ES-DE/downloaded_media/cps2/covers/a.png"), b"x").unwrap();

        let loc = locations(&layout, &system);
        assert_eq!(loc.structure, "esde");
        assert!(loc.gamelist_path.ends_with(&format!("gamelists{}cps2{}gamelist.xml", std::path::MAIN_SEPARATOR, std::path::MAIN_SEPARATOR)));
        assert!(!loc.gamelist_exists);
        assert_eq!(loc.media_folders.len(), 1);
        assert_eq!((loc.media_folders[0].folder.as_str(), loc.media_folders[0].files), ("covers", 1));
    }

    #[test]
    fn classic_lists_legacy_media_folders_too() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        fs::create_dir_all(system.rom_dir.join("images")).unwrap();
        fs::write(system.rom_dir.join("images/a-image.png"), b"x").unwrap();
        fs::write(system.rom_dir.join("gamelist.xml"), b"<gameList/>").unwrap();

        let loc = locations(&Layout::Classic, &system);
        assert_eq!(loc.structure, "classic");
        assert!(loc.gamelist_exists);
        assert_eq!(loc.media_dir, system.rom_dir.to_string_lossy().to_string());
        assert!(loc.media_folders.iter().any(|f| f.folder == "images" && f.files == 1));
    }
}
