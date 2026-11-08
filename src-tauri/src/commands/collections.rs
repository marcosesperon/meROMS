use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::layout::Layout;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscGroup {
    pub base_name: String,
    pub files: Vec<String>,
}

/// Detect multi-disc games by matching patterns like (Disc 1), (CD1), (Disk 1), etc.
#[tauri::command(async)]
pub fn detect_multidisc_games(app: tauri::AppHandle, system_path: String) -> Result<Vec<DiscGroup>, String> {
    let gamelist = crate::commands::gamelist::load_gamelist(app, system_path, None)?;
    let mut groups: HashMap<String, Vec<(String, u32)>> = HashMap::new();

    for game in &gamelist.games {
        let path = &game.path;
        let name = game.name.as_deref().unwrap_or(path);

        // Try to extract disc number from various patterns
        let disc_patterns = [
            "(Disc ", "(CD", "(Disk ", "(disc ", "(cd", "(disk ",
        ];

        let mut found = false;
        for pattern in &disc_patterns {
            if let Some(pos) = name.find(pattern) {
                let base = name[..pos].trim().to_string();
                let after = &name[pos + pattern.len()..];
                if let Some(num_str) = after.split(|c: char| !c.is_ascii_digit()).next() {
                    if let Ok(num) = num_str.parse::<u32>() {
                        groups.entry(base).or_default().push((path.clone(), num));
                        found = true;
                        break;
                    }
                }
            }
        }

        if !found {
            // Also check path for disc patterns
            let path_lower = path.to_lowercase();
            for pattern in &["(disc ", "(cd", "(disk "] {
                if let Some(pos) = path_lower.find(pattern) {
                    let base = path[..pos].trim().to_string();
                    let after = &path_lower[pos + pattern.len()..];
                    if let Some(num_str) = after.split(|c: char| !c.is_ascii_digit()).next() {
                        if let Ok(num) = num_str.parse::<u32>() {
                            groups.entry(base).or_default().push((path.clone(), num));
                            break;
                        }
                    }
                }
            }
        }
    }

    // Only keep groups with 2+ discs, sorted by disc number
    let mut result: Vec<DiscGroup> = Vec::new();
    for (base_name, mut files) in groups {
        if files.len() >= 2 {
            files.sort_by_key(|(_, num)| *num);
            result.push(DiscGroup {
                base_name,
                files: files.into_iter().map(|(path, _)| path).collect(),
            });
        }
    }
    result.sort_by(|a, b| a.base_name.cmp(&b.base_name));

    Ok(result)
}

/// Generate .m3u playlist files for multi-disc game groups
#[tauri::command(async)]
pub fn generate_m3u_playlists(
    system_path: String,
    groups: Vec<DiscGroup>,
) -> Result<Vec<String>, String> {
    let sys_path = Path::new(&system_path);
    let mut created: Vec<String> = Vec::new();

    for group in &groups {
        let m3u_name = format!("{}.m3u", group.base_name);
        let m3u_path = sys_path.join(&m3u_name);

        let content: String = group
            .files
            .iter()
            .map(|f| f.strip_prefix("./").unwrap_or(f).to_string())
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(&m3u_path, &content)
            .map_err(|e| format!("Failed to write {}: {}", m3u_name, e))?;
        created.push(m3u_name);
    }

    Ok(created)
}

/// Generate a .cfg custom collection where the active structure keeps them:
/// - Classic: in the system folder, with gamelist-style paths (`./game.zip`), replacing the file.
/// - ES-DE: in `<ES-DE>/collections`, with `%ROMPATH%/<system>/game.zip` entries. ES-DE
///   collections span systems, so new entries are merged into an existing file instead.
#[tauri::command(async)]
pub fn generate_collection(
    app: tauri::AppHandle,
    system_path: String,
    name: String,
    game_paths: Vec<String>,
) -> Result<String, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let dir = layout.collections_dir(&system);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create collections folder: {}", e))?;

    let filename = collection_file_name(&layout, &name);
    let output_path = dir.join(&filename);
    let new_entries: Vec<String> = game_paths
        .iter()
        .map(|p| layout.collection_entry(&system, p))
        .collect();

    let content = match layout {
        Layout::Classic => new_entries.join("\n"),
        Layout::Esde(_) => {
            let existing = fs::read_to_string(&output_path).unwrap_or_default();
            merge_collection_entries(&existing, &new_entries)
        }
    };
    fs::write(&output_path, &content)
        .map_err(|e| format!("Failed to write collection: {}", e))?;

    Ok(filename)
}

/// `custom-<name>.cfg`. Classic keeps meROMS' historical underscores; ES-DE shows the
/// file name as the collection name, so spaces are kept there.
pub fn collection_file_name(layout: &Layout, name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    match layout {
        Layout::Classic => format!("custom-{}.cfg", safe.replace(' ', "_")),
        Layout::Esde(_) => format!("custom-{}.cfg", safe.trim()),
    }
}

/// Existing lines first, then new ones not already present; one entry per line.
pub fn merge_collection_entries(existing: &str, new_entries: &[String]) -> String {
    let mut lines: Vec<String> = existing
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();
    for entry in new_entries {
        if !lines.contains(entry) {
            lines.push(entry.clone());
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esde_merges_without_duplicates() {
        let existing = "%ROMPATH%/nes/a.nes\n%ROMPATH%/snes/b.sfc\n";
        let merged = merge_collection_entries(
            existing,
            &["%ROMPATH%/snes/b.sfc".into(), "%ROMPATH%/snes/c.sfc".into()],
        );
        assert_eq!(merged, "%ROMPATH%/nes/a.nes\n%ROMPATH%/snes/b.sfc\n%ROMPATH%/snes/c.sfc\n");
    }

    #[test]
    fn file_names_per_structure() {
        let esde = Layout::Esde(crate::layout::EsdePaths {
            home: "/ficticio/ES-DE".into(),
            media_root: "/ficticio/ES-DE/downloaded_media".into(),
        });
        assert_eq!(collection_file_name(&Layout::Classic, "Mis Juegos"), "custom-Mis_Juegos.cfg");
        assert_eq!(collection_file_name(&esde, "Mis Juegos"), "custom-Mis Juegos.cfg");
        assert_eq!(collection_file_name(&esde, "a/b"), "custom-a_b.cfg");
    }
}
