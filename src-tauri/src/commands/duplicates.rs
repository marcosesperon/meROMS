use crate::models::game::Game;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateGroup {
    pub reason: String,
    pub games: Vec<Game>,
}

/// Normalize a game name for fuzzy comparison:
/// lowercase, strip parenthetical tags like (USA), (Rev 1), (En,Fr), [!], etc.
fn normalize_name(name: &str) -> String {
    let mut s = name.to_lowercase();
    // Remove parenthetical and bracket tags
    while let Some(start) = s.find('(') {
        if let Some(end) = s[start..].find(')') {
            s = format!("{}{}", &s[..start], &s[start + end + 1..]);
        } else {
            break;
        }
    }
    while let Some(start) = s.find('[') {
        if let Some(end) = s[start..].find(']') {
            s = format!("{}{}", &s[..start], &s[start + end + 1..]);
        } else {
            break;
        }
    }
    s.trim().to_string()
}

#[tauri::command]
pub fn find_duplicates(system_path: String) -> Result<Vec<DuplicateGroup>, String> {
    let gamelist = crate::commands::gamelist::load_gamelist(system_path)?;
    let games = &gamelist.games;
    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let mut used_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();

    // 1. Group by same ScreenScraper ID
    let mut id_map: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, game) in games.iter().enumerate() {
        if let Some(id) = &game.id {
            if !id.is_empty() {
                id_map.entry(id.clone()).or_default().push(i);
            }
        }
    }
    for (id, indices) in &id_map {
        if indices.len() > 1 {
            groups.push(DuplicateGroup {
                reason: format!("Same ScreenScraper ID: {}", id),
                games: indices.iter().map(|&i| games[i].clone()).collect(),
            });
            for &i in indices {
                used_indices.insert(i);
            }
        }
    }

    // 2. Group by same hash (md5 or hash)
    let mut hash_map: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, game) in games.iter().enumerate() {
        if used_indices.contains(&i) {
            continue;
        }
        let h = game.md5.as_ref().or(game.hash.as_ref());
        if let Some(hash) = h {
            if !hash.is_empty() {
                hash_map.entry(hash.clone()).or_default().push(i);
            }
        }
    }
    for (_hash, indices) in &hash_map {
        if indices.len() > 1 {
            groups.push(DuplicateGroup {
                reason: "Same file hash".to_string(),
                games: indices.iter().map(|&i| games[i].clone()).collect(),
            });
            for &i in indices {
                used_indices.insert(i);
            }
        }
    }

    // 3. Group by similar name (normalized)
    let mut name_map: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, game) in games.iter().enumerate() {
        if used_indices.contains(&i) {
            continue;
        }
        let name = game.name.as_deref().unwrap_or(&game.path);
        let normalized = normalize_name(name);
        if !normalized.is_empty() {
            name_map.entry(normalized).or_default().push(i);
        }
    }
    for (_name, indices) in &name_map {
        if indices.len() > 1 {
            groups.push(DuplicateGroup {
                reason: "Similar name".to_string(),
                games: indices.iter().map(|&i| games[i].clone()).collect(),
            });
        }
    }

    Ok(groups)
}
