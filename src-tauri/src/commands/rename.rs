use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::models::GameList;
use crate::parser;

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
pub fn list_rename_candidates(system_path: String) -> Result<Vec<RenameEntry>, String> {
    let gamelist = crate::commands::gamelist::load_gamelist(system_path.clone())?;

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
#[tauri::command]
pub fn rename_roms(
    system_path: String,
    renames: Vec<RenameRequest>,
) -> Result<GameList, String> {
    let sys_path = Path::new(&system_path);
    let gamelist_path = sys_path.join("gamelist.xml");

    // Backup before modifying
    crate::commands::backup::backup_gamelist(&system_path)?;

    let xml = fs::read_to_string(&gamelist_path)
        .map_err(|e| format!("Failed to read gamelist.xml: {}", e))?;
    let mut gamelist = parser::parse_gamelist(&xml)?;

    for req in &renames {
        let old_rom = Path::new(&req.current_path);
        let ext = old_rom
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let old_stem = old_rom
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let new_filename = format!("{}.{}", req.new_name, ext);
        let new_relative = format!("./{}", new_filename);

        // Resolve absolute paths
        let old_rel = req.current_path.strip_prefix("./").unwrap_or(&req.current_path);
        let old_abs = sys_path.join(old_rel);
        let new_abs = sys_path.join(&new_filename);

        // Skip if source doesn't exist or destination already exists
        if !old_abs.exists() {
            continue;
        }
        if new_abs.exists() && old_abs != new_abs {
            continue;
        }

        // Rename the ROM file
        if old_abs != new_abs {
            fs::rename(&old_abs, &new_abs)
                .map_err(|e| format!("Failed to rename {}: {}", old_rel, e))?;
        }

        // Update gamelist entry
        for game in &mut gamelist.games {
            if game.path == req.current_path {
                game.path = new_relative.clone();

                // Rename associated media files (same stem → new stem)
                rename_media_field(&mut game.image, sys_path, old_stem, &req.new_name);
                rename_media_field(&mut game.thumbnail, sys_path, old_stem, &req.new_name);
                rename_media_field(&mut game.marquee, sys_path, old_stem, &req.new_name);
                rename_media_field(&mut game.video, sys_path, old_stem, &req.new_name);
                break;
            }
        }
    }

    // Save updated gamelist, preserving unknown fields
    let original_xml = fs::read_to_string(&gamelist_path).ok();
    let xml_out = parser::serialize_gamelist(&gamelist, original_xml.as_deref())?;
    fs::write(&gamelist_path, xml_out)
        .map_err(|e| format!("Failed to write gamelist.xml: {}", e))?;

    Ok(gamelist)
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
