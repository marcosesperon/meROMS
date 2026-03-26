use std::fs;
use std::path::Path;
use serde_json::json;

/// Export gamelist to RetroArch .lpl playlist format
fn export_retroarch(
    gamelist: &crate::models::GameList,
    system_path: &str,
    output_path: &str,
    system_name: &str,
) -> Result<String, String> {
    let items: Vec<serde_json::Value> = gamelist
        .games
        .iter()
        .map(|game| {
            let rom_path = if game.path.starts_with("./") {
                Path::new(system_path)
                    .join(&game.path[2..])
                    .to_string_lossy()
                    .to_string()
            } else {
                game.path.clone()
            };
            json!({
                "path": rom_path,
                "label": game.name.as_deref().unwrap_or(&game.path),
                "core_path": "DETECT",
                "core_name": "DETECT",
                "crc32": game.crc32.as_deref().unwrap_or("00000000|crc"),
                "db_name": format!("{}.lpl", system_name)
            })
        })
        .collect();

    let playlist = json!({
        "version": "1.5",
        "default_core_path": "",
        "default_core_name": "",
        "label_display_mode": 0,
        "right_thumbnail_mode": 0,
        "left_thumbnail_mode": 0,
        "sort_mode": 0,
        "items": items
    });

    let filename = format!("{}.lpl", system_name);
    let dest = Path::new(output_path).join(&filename);
    let content = serde_json::to_string_pretty(&playlist)
        .map_err(|e| format!("JSON serialization failed: {}", e))?;
    fs::write(&dest, content).map_err(|e| format!("Failed to write: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn export_gamelist(
    system_path: String,
    format: String,
    output_path: String,
) -> Result<String, String> {
    let gamelist = crate::commands::gamelist::load_gamelist(system_path.clone())?;

    let system_name = Path::new(&system_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    match format.as_str() {
        "retroarch" => export_retroarch(&gamelist, &system_path, &output_path, &system_name),
        _ => Err(format!("Unsupported export format: {}", format)),
    }
}
