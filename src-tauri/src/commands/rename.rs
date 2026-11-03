use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::media;
use crate::models::GameList;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameEntry {
    /// Current relative ROM path (e.g. "./Super Mario World.sfc")
    pub current_path: String,
    /// Current filename without extension
    pub current_name: String,
    /// Game title from gamelist.xml (None if no title)
    pub game_title: Option<String>,
    /// Proposed new filename (sanitized title)
    pub proposed_name: String,
    /// Whether a rename is possible (has title and title differs from current name)
    pub can_rename: bool,
}

/// Characters not allowed in filenames.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// List all ROMs with their current filename and gamelist title.
#[tauri::command]
pub fn list_rename_candidates(app: tauri::AppHandle, system_path: String) -> Result<Vec<RenameEntry>, String> {
    let gamelist = crate::commands::gamelist::load_gamelist(app, system_path)?;

    let mut entries = Vec::new();

    for game in &gamelist.games {
        let rom_path = Path::new(&game.path);
        let ext = rom_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let current_name = rom_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&game.path)
            .to_string();

        let game_title = game.name.clone();
        let proposed = if let Some(ref title) = game_title {
            sanitize_filename(title)
        } else {
            current_name.clone()
        };

        let can_rename = game_title.is_some() && proposed != current_name && !ext.is_empty();

        entries.push(RenameEntry {
            current_path: game.path.clone(),
            current_name,
            game_title,
            proposed_name: proposed,
            can_rename,
        });
    }

    entries.sort_by(|a, b| a.current_name.to_lowercase().cmp(&b.current_name.to_lowercase()));
    Ok(entries)
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameRequest {
    pub current_path: String,
    pub new_name: String,
}

/// Rename ROM files and update all references in gamelist.xml.
/// Media are renamed too: the unified `<folder>/<ROM name>.<ext>` files in both structures,
/// plus (Classic) any other file the gamelist points to that is named after the ROM.
#[tauri::command]
pub fn rename_roms(
    app: tauri::AppHandle,
    system_path: String,
    renames: Vec<RenameRequest>,
) -> Result<GameList, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let sys_path = system.rom_dir.clone();

    // write_gamelist() backs the gamelist up before saving
    let mut gamelist = crate::commands::gamelist::read_gamelist(&layout, &system)?;

    for req in &renames {
        let old_rel = req.current_path.strip_prefix("./").unwrap_or(&req.current_path);
        let old_rom = Path::new(old_rel);
        let ext = old_rom
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let old_stem = old_rom
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // Keep the ROM in its own subfolder (only the file name changes)
        let new_filename = format!("{}.{}", req.new_name, ext);
        let new_rel = match old_rom.parent().filter(|p| !p.as_os_str().is_empty()) {
            Some(parent) => parent.join(&new_filename).to_string_lossy().replace('\\', "/"),
            None => new_filename.clone(),
        };
        let new_relative = format!("./{}", new_rel);

        // Resolve absolute paths
        let old_abs = sys_path.join(old_rel);
        let new_abs = sys_path.join(&new_rel);

        // Skip if source doesn't exist or destination already exists
        if !old_abs.exists() {
            continue;
        }
        if new_abs.exists() && old_abs != new_abs {
            continue;
        }
        let rom_is_dir = old_abs.is_dir();

        // Rename the ROM file
        if old_abs != new_abs {
            fs::rename(&old_abs, &new_abs)
                .map_err(|e| format!("Failed to rename {}: {}", old_rel, e))?;
        }

        let moved = media::rename_rom_media(&layout, &system, &req.current_path, &new_relative, rom_is_dir);

        // Update gamelist entry
        for game in &mut gamelist.games {
            if game.path == req.current_path {
                game.path = new_relative.clone();

                if layout.writes_media_tags() {
                    for field in media::media_fields() {
                        update_media_field(game, field, &layout, &system, &moved, old_stem, &req.new_name);
                    }
                }
                break;
            }
        }
    }

    crate::commands::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    // Re-read so ES-DE media fields reflect the renamed files
    crate::commands::gamelist::read_gamelist(&layout, &system)
}

/// Classic: point a media field to its renamed file. Unified files were already moved
/// (`moved`); anything else named after the ROM (legacy suffixes) is renamed here.
fn update_media_field(
    game: &mut crate::models::Game,
    field: &str,
    layout: &crate::layout::Layout,
    system: &crate::layout::SystemRef,
    moved: &[(std::path::PathBuf, std::path::PathBuf)],
    old_stem: &str,
    new_stem: &str,
) {
    let Some(current) = media::get_field(game, field).cloned() else { return };
    let current_abs = media::resolve_media_path(system, &current);

    if let Some((_, new)) = moved.iter().find(|(old, _)| *old == current_abs) {
        media::set_field(game, field, Some(media::media_ref(layout, system, new)));
        return;
    }

    let mut value = Some(current);
    rename_media_field(&mut value, &system.rom_dir, old_stem, new_stem);
    media::set_field(game, field, value);
}

/// Rename a media file if its stem matches the old ROM stem.
fn rename_media_field(
    field: &mut Option<String>,
    sys_path: &Path,
    old_stem: &str,
    new_stem: &str,
) {
    let Some(media_path) = field.as_ref() else {
        return;
    };

    let media_rel = media_path.strip_prefix("./").unwrap_or(media_path);
    let media_p = Path::new(media_rel);

    let media_filestem = media_p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // Check if the media filename starts with the old ROM stem
    // Handles both "Mario.png" and "Mario-image.png" patterns
    if !media_filestem.starts_with(old_stem) {
        return;
    }

    let suffix = &media_filestem[old_stem.len()..]; // e.g. "", "-image", "-box-2D"
    let media_ext = media_p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let parent = media_p.parent().and_then(|p| p.to_str()).unwrap_or("");

    let new_media_filename = format!("{}{}.{}", new_stem, suffix, media_ext);
    let new_media_rel = if parent.is_empty() {
        new_media_filename.clone()
    } else {
        format!("{}/{}", parent, new_media_filename)
    };

    let old_abs = sys_path.join(media_rel);
    let new_abs = sys_path.join(&new_media_rel);

    if old_abs.exists() && old_abs != new_abs {
        if fs::rename(&old_abs, &new_abs).is_ok() {
            *field = Some(format!("./{}", new_media_rel));
        }
    } else {
        // File might not exist, just update the path reference
        *field = Some(format!("./{}", new_media_rel));
    }
}
