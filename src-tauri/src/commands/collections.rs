use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscGroup {
    pub base_name: String,
    pub files: Vec<String>,
}

/// Detect multi-disc games by matching patterns like (Disc 1), (CD1), (Disk 1), etc.
#[tauri::command]
pub fn detect_multidisc_games(system_path: String) -> Result<Vec<DiscGroup>, String> {
    let gamelist = crate::commands::gamelist::load_gamelist(system_path)?;
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
#[tauri::command]
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

/// Generate a .cfg collection file (EmulationStation format)
#[tauri::command]
pub fn generate_collection(
    name: String,
    game_paths: Vec<String>,
    output_dir: String,
) -> Result<String, String> {
    let filename = format!("custom-{}.cfg", name.replace(' ', "_"));
    let output_path = Path::new(&output_dir).join(&filename);

    let content = game_paths.join("\n");
    fs::write(&output_path, &content)
        .map_err(|e| format!("Failed to write collection: {}", e))?;

    Ok(filename)
}
