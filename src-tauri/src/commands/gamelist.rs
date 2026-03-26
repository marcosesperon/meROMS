use std::fs;
use std::path::Path;

use crate::models::system::extensions_for_system;
use crate::models::{Game, GameList};
use crate::models::Folder;
use crate::parser;

#[tauri::command]
pub fn load_gamelist(system_path: String) -> Result<GameList, String> {
    let gamelist_path = Path::new(&system_path).join("gamelist.xml");

    if !gamelist_path.exists() {
        return Ok(GameList::default());
    }

    let xml = fs::read_to_string(&gamelist_path)
        .map_err(|e| format!("Failed to read gamelist.xml: {}", e))?;

    parser::parse_gamelist(&xml)
}

fn save_gamelist_to_disk(system_path: &str, gamelist: &GameList) -> Result<(), String> {
    // Backup before writing
    crate::commands::backup::backup_gamelist(system_path)?;

    let gamelist_path = Path::new(system_path).join("gamelist.xml");

    // Read original XML to preserve unknown fields during merge
    let original_xml = fs::read_to_string(&gamelist_path).ok();
    let xml = parser::serialize_gamelist(gamelist, original_xml.as_deref())?;

    fs::write(&gamelist_path, xml)
        .map_err(|e| format!("Failed to write gamelist.xml: {}", e))
}

#[tauri::command]
pub fn save_gamelist(system_path: String, gamelist: GameList) -> Result<(), String> {
    save_gamelist_to_disk(&system_path, &gamelist)
}

#[tauri::command]
pub fn add_game(system_path: String, game: Game) -> Result<GameList, String> {
    let mut gamelist = load_gamelist(system_path.clone())?;
    gamelist.games.push(game);
    save_gamelist_to_disk(&system_path, &gamelist)?;
    Ok(gamelist)
}

#[tauri::command]
pub fn remove_game(system_path: String, game_path: String) -> Result<GameList, String> {
    let mut gamelist = load_gamelist(system_path.clone())?;
    gamelist.games.retain(|g| g.path != game_path);
    save_gamelist_to_disk(&system_path, &gamelist)?;
    Ok(gamelist)
}

/// Scans the system folder for ROM files based on the folder name,
/// generates a gamelist.xml with one entry per ROM, and saves it.
#[tauri::command]
pub fn generate_gamelist(system_path: String) -> Result<GameList, String> {
    let path = Path::new(&system_path);
    let folder_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let extensions = extensions_for_system(folder_name);

    let entries = fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    let mut games: Vec<Game> = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let file_path = entry.path();

        if !file_path.is_file() {
            continue;
        }

        let Some(ext) = file_path.extension().and_then(|e| e.to_str()) else {
            continue;
        };

        let dot_ext = format!(".{}", ext.to_lowercase());
        if !extensions.contains(&dot_ext.as_str()) {
            continue;
        }

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Derive a readable name from the filename (strip extension)
        let display_name = file_path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or(&file_name)
            .to_string();

        let relative_path = format!("./{}", file_name);

        // Check if an image exists in common image subdirectories
        let image = find_existing_image(path, &display_name);

        games.push(Game {
            path: relative_path,
            name: Some(display_name),
            image,
            ..Default::default()
        });
    }

    games.sort_by(|a, b| {
        a.name
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .cmp(&b.name.as_deref().unwrap_or("").to_lowercase())
    });

    let gamelist = GameList { games, ..Default::default() };
    save_gamelist_to_disk(&system_path, &gamelist)?;
    Ok(gamelist)
}

/// Looks for an existing image in common subdirectories (images, media/images, etc.)
fn find_existing_image(system_dir: &Path, rom_stem: &str) -> Option<String> {
    let image_dirs = ["images", "media/images", "media/screenshots", "media/box2dfront"];
    let image_exts = ["png", "jpg", "jpeg", "gif", "webp"];

    for dir in &image_dirs {
        let img_dir = system_dir.join(dir);
        if !img_dir.is_dir() {
            continue;
        }
        for ext in &image_exts {
            // Try exact stem match
            let candidate = img_dir.join(format!("{}.{}", rom_stem, ext));
            if candidate.exists() {
                return Some(format!("./{}/{}.{}", dir, rom_stem, ext));
            }
            // Try stem-image suffix (common in scraped sets)
            let candidate = img_dir.join(format!("{}-image.{}", rom_stem, ext));
            if candidate.exists() {
                return Some(format!("./{}/{}-image.{}", dir, rom_stem, ext));
            }
        }
    }
    None
}

#[tauri::command]
pub fn add_folder(system_path: String, folder: Folder) -> Result<GameList, String> {
    let mut gamelist = load_gamelist(system_path.clone())?;
    gamelist.folders.push(folder);
    save_gamelist_to_disk(&system_path, &gamelist)?;
    Ok(gamelist)
}

#[tauri::command]
pub fn remove_folder(system_path: String, folder_path: String) -> Result<GameList, String> {
    let mut gamelist = load_gamelist(system_path.clone())?;
    gamelist.folders.retain(|f| f.path != folder_path);
    save_gamelist_to_disk(&system_path, &gamelist)?;
    Ok(gamelist)
}
