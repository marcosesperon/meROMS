use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::{Layout, SystemRef};
use crate::media::{self, MediaIndex, MEDIA_KINDS};
use crate::models::GameList;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedMediaFile {
    /// Absolute path to the file
    pub path: String,
    /// Path relative to the system's media base (e.g. "covers/Mario.png")
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
    "manuals",
    "wheels",
    "media/images",
    "media/screenshots",
    "media/box2dfront",
];

const MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg",
    "mp4", "mkv", "avi", "mov", "wmv", "webm", "pdf",
];

/// Collect all media file paths referenced in the gamelist.
fn collect_referenced_paths(gamelist: &GameList, system_path: &Path) -> HashSet<String> {
    let mut referenced = HashSet::new();

    for game in &gamelist.games {
        // Every media field counts (fanart, titleshot, mix… are referenced too)
        for field in media::media_fields() {
            if let Some(media_path) = media::get_field(game, field) {
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

/// Find media files nobody uses:
/// - Classic: files in the media folders not referenced by any game in the gamelist.
/// - ES-DE: files whose name matches no ROM (ES-DE matches media by ROM name).
#[tauri::command]
pub fn find_unused_media(app: tauri::AppHandle, system_path: String) -> Result<Vec<UnusedMediaFile>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let mut unused = match layout {
        Layout::Classic => find_unused_classic(&layout, &system, &system_path)?,
        Layout::Esde(_) => find_unused_esde(&layout, &system)?,
    };
    unused.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(unused)
}

fn find_unused_esde(layout: &Layout, system: &SystemRef) -> Result<Vec<UnusedMediaFile>, String> {
    let gamelist = crate::commands::gamelist::read_gamelist(layout, system)?;
    let rom_stems = rom_stems(system, &gamelist);
    let index = MediaIndex::build(layout, system);
    let media_base = layout.media_type_dir(system, "");

    let mut unused = Vec::new();
    for kind in MEDIA_KINDS.iter().filter(|k| k.in_esde) {
        for (stem, file) in index.files_of(kind) {
            if rom_stems.contains(stem) {
                continue;
            }
            let relative = file
                .strip_prefix(&media_base)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file.to_string_lossy().to_string());
            unused.push(UnusedMediaFile {
                path: file.to_string_lossy().to_string(),
                relative,
                size: fs::metadata(file).map(|m| m.len()).unwrap_or(0),
            });
        }
    }
    Ok(unused)
}

/// Media base names ES-DE would match: every gamelist entry plus every file or folder
/// found in the ROM folder (ES-DE lists ROMs even without a gamelist entry).
fn rom_stems(system: &SystemRef, gamelist: &GameList) -> HashSet<PathBuf> {
    let mut stems: HashSet<PathBuf> = gamelist
        .games
        .iter()
        .map(|g| crate::layout::rom_media_stem(system, &g.path))
        .collect();
    collect_rom_stems(&system.rom_dir, &system.rom_dir, &mut stems);
    stems
}

fn collect_rom_stems(root: &Path, dir: &Path, stems: &mut HashSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(root) else { continue };
        if path.is_dir() {
            // A folder can be a game folder or a "directory interpreted as file": keep both names
            stems.insert(rel.to_path_buf());
            collect_rom_stems(root, &path, stems);
        } else if let Some(stem) = rel.file_stem() {
            stems.insert(rel.with_file_name(stem));
        }
    }
}

fn find_unused_classic(layout: &Layout, system: &SystemRef, system_path: &str) -> Result<Vec<UnusedMediaFile>, String> {
    let sys_path = Path::new(system_path);

    // Load gamelist
    let gamelist = crate::commands::gamelist::read_gamelist(layout, system)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::EsdePaths;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn esde_unused_means_no_rom_with_that_name() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Esde(EsdePaths {
            home: tmp.path().join("ES-DE"),
            media_root: tmp.path().join("ES-DE/downloaded_media"),
        });
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        // A ROM on disk without gamelist entry still "uses" its media
        touch(&system.rom_dir.join("Sin Entrada.sfc"));
        let media = tmp.path().join("ES-DE/downloaded_media/snes");
        touch(&media.join("covers/Sin Entrada.png"));
        touch(&media.join("covers/Huerfano.png"));
        touch(&media.join("fanart/Huerfano.jpg"));

        let unused = find_unused_esde(&layout, &system).unwrap();
        let mut names: Vec<_> = unused.iter().map(|u| u.relative.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["covers/Huerfano.png", "fanart/Huerfano.jpg"]);
    }

    #[test]
    fn classic_counts_every_media_field_as_used() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("fanart/Juego.png"));
        let gamelist = GameList {
            games: vec![crate::models::Game {
                path: "./Juego.sfc".into(),
                fanart: Some("./fanart/Juego.png".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        crate::commands::gamelist::write_gamelist(&Layout::Classic, &system, &gamelist).unwrap();

        let unused = find_unused_classic(&Layout::Classic, &system, system.rom_dir.to_str().unwrap()).unwrap();
        assert!(unused.is_empty(), "fanart is referenced, so it is not unused: {:?}", unused);
    }
}
