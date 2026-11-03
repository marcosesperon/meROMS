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

/// Leftover media of ONE game (game detail panel): files named after its ROM that it
/// doesn't use.
/// - ES-DE: in each media folder, the files with the ROM name that lose against the one
///   ES-DE shows (same extension preference as `MediaIndex`, e.g. the .jpg next to a .png).
/// - Classic: files in the media folders named after the ROM (or with an old suffix such as
///   "-image") that no media field of any game points to.
#[tauri::command]
pub fn find_game_leftover_media(
    app: tauri::AppHandle,
    system_path: String,
    game_path: String,
) -> Result<Vec<UnusedMediaFile>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let mut leftovers = match layout {
        Layout::Classic => {
            let gamelist = crate::commands::gamelist::read_gamelist(&layout, &system)?;
            game_leftovers_classic(&system, &gamelist, &game_path)
        }
        Layout::Esde(_) => game_leftovers_esde(&layout, &system, &game_path),
    };
    leftovers.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(leftovers)
}

fn game_leftovers_esde(layout: &Layout, system: &SystemRef, game_path: &str) -> Vec<UnusedMediaFile> {
    let stem = crate::layout::rom_media_stem(system, game_path);
    let media_base = layout.media_type_dir(system, "");
    let mut leftovers = Vec::new();

    for kind in MEDIA_KINDS.iter().filter(|k| k.in_esde) {
        // Files of this type named after the ROM, in ES-DE's preference order
        let dir = layout.media_type_dir(system, kind.folder);
        let found: Vec<PathBuf> = files_named(&dir.join(&stem))
            .into_iter()
            .filter_map(|file| {
                let ext = media::extension_lower(&file)?;
                let priority = kind.extensions().iter().position(|e| *e == ext)?;
                Some((priority, file))
            })
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_values()
            .collect();
        // The first one is what ES-DE shows; the rest are leftovers
        for file in found.into_iter().skip(1) {
            leftovers.push(unused_file(&file, &media_base));
        }
    }
    leftovers
}

fn game_leftovers_classic(system: &SystemRef, gamelist: &GameList, game_path: &str) -> Vec<UnusedMediaFile> {
    let stem = crate::layout::rom_media_stem(system, game_path);
    let Some(rom_name) = stem.file_name().and_then(|n| n.to_str()) else { return Vec::new() };
    let referenced = collect_referenced_paths(gamelist, &system.rom_dir);

    // Names that belong to OTHER ROMs, so "Mario" never takes "Mario-cart"'s files
    let other_names: HashSet<String> = gamelist
        .games
        .iter()
        .filter(|g| g.path != game_path)
        .filter_map(|g| {
            let other = crate::layout::rom_media_stem(system, &g.path);
            other.file_name().map(|n| n.to_string_lossy().to_lowercase())
        })
        .collect();
    let suffixes: HashSet<&str> = super::images::legacy_targets()
        .into_iter()
        .flat_map(|(_, _, suffixes)| suffixes)
        .collect();
    let belongs_to_rom = |file_stem: &str| -> bool {
        if other_names.contains(&file_stem.to_lowercase()) {
            return false;
        }
        file_stem == rom_name
            || file_stem
                .strip_prefix(rom_name)
                .is_some_and(|rest| suffixes.contains(rest))
    };

    // Every folder where meROMS (or older tools) may have left media
    let mut folders: Vec<&str> = MEDIA_DIRS.to_vec();
    for (dirs, _, _) in super::images::legacy_targets() {
        folders.extend(dirs);
    }
    folders.sort_unstable();
    folders.dedup();

    let sub = stem.parent().unwrap_or(Path::new(""));
    let mut leftovers = Vec::new();
    for folder in folders {
        let dir = system.rom_dir.join(folder).join(sub);
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for path in entries.flatten().map(|e| e.path()).filter(|p| p.is_file()) {
            let is_media = media::extension_lower(&path).is_some_and(|ext| MEDIA_EXTENSIONS.contains(&ext.as_str()));
            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !is_media || !belongs_to_rom(file_stem) {
                continue;
            }
            let canonical = fs::canonicalize(&path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());
            if !referenced.contains(&canonical) {
                leftovers.push(unused_file(&path, &system.rom_dir));
            }
        }
    }
    leftovers
}

/// Files in the same folder as `stem_path` whose name (without extension) is its file name.
fn files_named(stem_path: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (stem_path.parent(), stem_path.file_name()) else { return Vec::new() };
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.file_stem() == Some(name))
        .collect()
}

fn unused_file(path: &Path, base: &Path) -> UnusedMediaFile {
    UnusedMediaFile {
        path: path.to_string_lossy().to_string(),
        relative: path
            .strip_prefix(base)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string()),
        size: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    }
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
    fn esde_game_leftovers_are_the_formats_es_de_does_not_show() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Esde(EsdePaths {
            home: tmp.path().join("ES-DE"),
            media_root: tmp.path().join("ES-DE/downloaded_media"),
        });
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        touch(&system.rom_dir.join("Juego.sfc"));
        let media = tmp.path().join("ES-DE/downloaded_media/snes");
        touch(&media.join("covers/Juego.png"));
        touch(&media.join("covers/Juego.jpg"));
        touch(&media.join("fanart/Juego.jpg"));
        touch(&media.join("covers/Otro.jpg"));

        let leftovers = game_leftovers_esde(&layout, &system, "./Juego.sfc");
        let names: Vec<_> = leftovers.iter().map(|u| u.relative.clone()).collect();
        assert_eq!(names, vec!["covers/Juego.jpg"]);
    }

    #[test]
    fn classic_game_leftovers_skip_used_files_and_other_roms() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("covers/Juego.png"));
        touch(&system.rom_dir.join("covers/Juego.jpg"));
        touch(&system.rom_dir.join("images/Juego-image.png"));
        touch(&system.rom_dir.join("images/Juego-cart.png"));
        touch(&system.rom_dir.join("images/Juego-thumb.png"));
        touch(&system.rom_dir.join("covers/Otro.png"));
        let gamelist = GameList {
            games: vec![
                crate::models::Game {
                    path: "./Juego.sfc".into(),
                    image: Some("./covers/Juego.png".into()),
                    thumbnail: Some("./images/Juego-thumb.png".into()),
                    ..Default::default()
                },
                // Another ROM whose name looks like "Juego" + a suffix
                crate::models::Game { path: "./Juego-cart.sfc".into(), ..Default::default() },
            ],
            ..Default::default()
        };

        let leftovers = game_leftovers_classic(&system, &gamelist, "./Juego.sfc");
        let mut names: Vec<_> = leftovers.iter().map(|u| u.relative.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["covers/Juego.jpg", "images/Juego-image.png"]);
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
