use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::models::GameList;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedMediaFile {
    /// Absolute path to the file
    pub path: String,
    /// Path relative to system folder (e.g. "covers/Mario.png")
    pub relative: String,
    /// File size in bytes
    pub size: u64,
}

/// Media subdirectories to scan for unused files.
const MEDIA_DIRS: &[&str] = &[
    "images",
    "covers",
    "screenshots",
    "marquees",
    "titlescreens",
    "fanart",
    "3dboxes",
    "backcovers",
    "miximages",
    "videos",
    "physicalmedia",
    "media/images",
    "media/screenshots",
    "media/box2dfront",
];

const MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg",
    "mp4", "mkv", "avi", "mov", "wmv", "webm",
];

/// Collect all media file paths referenced in the gamelist.
fn collect_referenced_paths(gamelist: &GameList, system_path: &Path) -> HashSet<String> {
    let mut referenced = HashSet::new();

    for game in &gamelist.games {
        for field in [&game.image, &game.thumbnail, &game.marquee, &game.video] {
            if let Some(media_path) = field {
                // Resolve relative paths to absolute
                let resolved = if media_path.starts_with("./") {
                    system_path
                        .join(media_path.strip_prefix("./").unwrap_or(media_path))
                        .to_string_lossy()
                        .to_string()
                } else if media_path.starts_with('/') {
                    media_path.clone()
                } else {
                    system_path.join(media_path).to_string_lossy().to_string()
                };
                // Canonicalize for consistent comparison
                if let Ok(canonical) = fs::canonicalize(&resolved) {
                    referenced.insert(canonical.to_string_lossy().to_string());
                } else {
                    referenced.insert(resolved);
                }
            }
        }
    }

    referenced
}

/// Scan a directory recursively for media files.
fn scan_media_files(dir: &Path) -> Vec<(String, String, u64)> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return files;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(scan_media_files(&path));
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if MEDIA_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let abs = path.to_string_lossy().to_string();
                    files.push((abs, ext.to_string(), size));
                }
            }
        }
    }

    files
}

/// Find all media files in a system folder that are not referenced by any game in the gamelist.
#[tauri::command]
pub fn find_unused_media(system_path: String) -> Result<Vec<UnusedMediaFile>, String> {
    let sys_path = Path::new(&system_path);

    // Load gamelist
    let gamelist = crate::commands::gamelist::load_gamelist(system_path.clone())?;
    let referenced = collect_referenced_paths(&gamelist, sys_path);

    let mut unused = Vec::new();

    // Scan all known media directories
    for subdir in MEDIA_DIRS {
        let dir = sys_path.join(subdir);
        if !dir.exists() {
            continue;
        }

        for (abs_path, _ext, size) in scan_media_files(&dir) {
            let canonical = fs::canonicalize(&abs_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| abs_path.clone());

            if !referenced.contains(&canonical) {
                let relative = abs_path
                    .strip_prefix(&format!("{}/", system_path))
                    .or_else(|| abs_path.strip_prefix(&system_path))
                    .unwrap_or(&abs_path)
                    .to_string();

                unused.push(UnusedMediaFile {
                    path: abs_path,
                    relative,
                    size,
                });
            }
        }
    }

    unused.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(unused)
}

/// Delete a list of files by their absolute paths.
#[tauri::command]
pub fn delete_files(paths: Vec<String>) -> Result<usize, String> {
    let mut deleted = 0;
    for path in &paths {
        match fs::remove_file(path) {
            Ok(_) => deleted += 1,
            Err(e) => eprintln!("Failed to delete {}: {}", path, e),
        }
    }
    Ok(deleted)
}
