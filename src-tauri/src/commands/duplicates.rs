use std::collections::HashSet;
use std::path::PathBuf;

use crate::layout::{Layout, SystemRef};
use crate::media::{self, MEDIA_KINDS};
use crate::models::game::Game;

use super::progress::Progress;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateGroup {
    /// Why they are grouped: "screenscraper_id" | "hash" | "similar_name" (translated in the UI)
    pub reason: String,
    /// Extra detail for the reason (the ScreenScraper ID)
    pub detail: Option<String>,
    pub games: Vec<Game>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RemoveResult {
    /// Games taken out of the gamelist
    pub removed: usize,
    /// Files sent (or, in a dry run, to be sent) to the trash
    pub files: Vec<String>,
    pub failed: Vec<String>,
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

#[tauri::command(async)]
pub fn find_duplicates(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<Vec<DuplicateGroup>, String> {
    let gamelist = crate::commands::gamelist::load_gamelist(app, system_path, task)?;
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
                reason: "screenscraper_id".to_string(),
                detail: Some(id.clone()),
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
                reason: "hash".to_string(),
                detail: None,
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
                reason: "similar_name".to_string(),
                detail: None,
                games: indices.iter().map(|&i| games[i].clone()).collect(),
            });
        }
    }

    Ok(groups)
}

/// Juegos por tanda al mandarlos a la papelera (cada tanda va de una vez)
const REMOVE_BATCH: usize = 25;

/// Manda a la papelera los ficheros de una tanda de juegos. Normalmente todos de una vez; si
/// eso falla, uno a uno, para que un juego cuya ROM no se pueda mandar se quede en la gamelist.
fn remove_batch(batch: &[(String, Option<PathBuf>, Vec<PathBuf>)], removed_paths: &mut HashSet<String>, failed: &mut Vec<String>) {
    let all: Vec<PathBuf> = batch.iter().flat_map(|(_, rom, media)| rom.iter().chain(media.iter()).cloned()).collect();
    if all.is_empty() || trash::delete_all(&all).is_ok() {
        removed_paths.extend(batch.iter().map(|(game_path, _, _)| game_path.clone()));
        return;
    }
    for (game_path, rom, media_files) in batch {
        // The ROM first: if it can't go to the trash, the game stays
        if let Some(rom) = rom {
            if !rom.exists() {
                // Already sent by the bulk attempt before it failed
            } else if let Err(e) = trash::delete(rom) {
                failed.push(format!("{} ({})", rom.display(), e));
                continue;
            }
        }
        removed_paths.insert(game_path.clone());
        for file in media_files.iter().filter(|f| f.exists()) {
            if let Err(e) = trash::delete(file) {
                failed.push(format!("{} ({})", file.display(), e));
            }
        }
    }
}

/// Remove duplicates for good: take the games out of the gamelist and send their ROM file and
/// all their media to the trash. With `dry_run` nothing is touched: it only lists the files
/// (the UI shows the count before asking for confirmation).
/// A game whose ROM can't be trashed stays in the gamelist, so no ROM is left without entry.
#[tauri::command(async)]
pub fn remove_games_with_files(
    app: tauri::AppHandle,
    system_path: String,
    game_paths: Vec<String>,
    dry_run: bool,
    task: Option<String>,
) -> Result<RemoveResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    let mut gamelist = super::gamelist::read_gamelist_with(&layout, &system, &progress)?;
    let plan = files_to_remove(&layout, &system, &gamelist.games, &game_paths);

    let mut result = RemoveResult::default();
    result.files = plan.iter().flat_map(|(_, rom, media)| rom.iter().chain(media.iter())).map(|p| p.to_string_lossy().to_string()).collect();
    if dry_run {
        result.removed = plan.len();
        return Ok(result);
    }

    let mut removed_paths: HashSet<String> = HashSet::new();
    // Por tandas de juegos: cada tanda va a la papelera de una vez (pocos sonidos en macOS) y,
    // entre una y otra, se avisa del progreso y se mira si el usuario ha cancelado
    let mut done_games = 0;
    for batch in plan.chunks(REMOVE_BATCH) {
        if progress.cancelled() {
            break;
        }
        done_games += batch.len();
        progress.step("trashing", done_games, plan.len(), &batch[0].0);
        remove_batch(batch, &mut removed_paths, &mut result.failed);
    }

    if !removed_paths.is_empty() {
        progress.phase("saving");
        gamelist.games.retain(|g| !removed_paths.contains(&g.path));
        // write_gamelist() backs up the gamelist first
        super::gamelist::write_gamelist(&layout, &system, &gamelist)?;
        // And out of the custom collections and .m3u playlists that listed them
        let removed: Vec<String> = removed_paths.iter().cloned().collect();
        super::rename::remove_references(&layout, &system, &removed);
    }
    result.removed = removed_paths.len();
    Ok(result)
}

/// For each game to remove: its path, its ROM (file or folder, if it exists) and its media files.
/// Media = every file named after the ROM in each media folder (any extension) and, in Classic,
/// the files its media fields point to, unless a game that stays also uses them.
fn files_to_remove(
    layout: &Layout,
    system: &SystemRef,
    games: &[Game],
    game_paths: &[String],
) -> Vec<(String, Option<PathBuf>, Vec<PathBuf>)> {
    // Media used by the games that stay (Classic): never trash those
    let kept_media: HashSet<PathBuf> = if layout.writes_media_tags() {
        games
            .iter()
            .filter(|g| !game_paths.contains(&g.path))
            .flat_map(|g| media::media_fields().filter_map(move |f| media::get_field(g, f)))
            .map(|v| media::resolve_media_path(system, v))
            .collect()
    } else {
        HashSet::new()
    };

    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut out = Vec::new();
    for game in games.iter().filter(|g| game_paths.contains(&g.path)) {
        let rom = media::resolve_media_path(system, &game.path);
        let rom = rom.exists().then_some(rom);

        let mut media_files = Vec::new();
        let mut add = |file: PathBuf, media_files: &mut Vec<PathBuf>| {
            if file.is_file() && !kept_media.contains(&file) && seen.insert(file.clone()) {
                media_files.push(file);
            }
        };
        for kind in MEDIA_KINDS {
            for ext in kind.extensions() {
                add(layout.media_file_path(system, kind.folder, &game.path, ext), &mut media_files);
            }
        }
        if layout.writes_media_tags() {
            for field in media::media_fields() {
                if let Some(value) = media::get_field(game, field) {
                    add(media::resolve_media_path(system, value), &mut media_files);
                }
            }
        }
        out.push((game.path.clone(), rom, media_files));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    fn game(path: &str) -> Game {
        Game { path: path.into(), name: Some(path.into()), ..Default::default() }
    }

    fn names(files: &[PathBuf], base: &Path) -> Vec<String> {
        let mut v: Vec<String> = files.iter().map(|f| f.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/")).collect();
        v.sort();
        v
    }

    #[test]
    fn esde_collects_the_rom_and_every_media_named_after_it() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Esde(crate::layout::EsdePaths {
            home: tmp.path().join("ES-DE"),
            media_root: tmp.path().join("ES-DE/downloaded_media"),
        });
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/mastersystem"));
        let media_dir = tmp.path().join("ES-DE/downloaded_media/mastersystem");
        touch(&system.rom_dir.join("SDI.sms"));
        touch(&media_dir.join("covers/SDI.png"));
        touch(&media_dir.join("covers/SDI.jpg"));
        touch(&media_dir.join("videos/SDI.mp4"));
        touch(&media_dir.join("covers/Otro.png"));

        let plan = files_to_remove(&layout, &system, &[game("./SDI.sms"), game("./Otro.sms")], &["./SDI.sms".into()]);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].1.as_deref(), Some(system.rom_dir.join("SDI.sms").as_path()));
        assert_eq!(names(&plan[0].2, &media_dir), vec!["covers/SDI.jpg", "covers/SDI.png", "videos/SDI.mp4"]);
    }

    #[test]
    fn classic_keeps_media_shared_with_a_game_that_stays() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("mastersystem"));
        touch(&system.rom_dir.join("Global Defense.sms"));
        touch(&system.rom_dir.join("SDI.sms"));
        touch(&system.rom_dir.join("images/SDI-image.png"));
        touch(&system.rom_dir.join("images/compartida.png"));
        touch(&system.rom_dir.join("covers/SDI.png"));
        let mut to_remove = game("./SDI.sms");
        to_remove.image = Some("./images/SDI-image.png".into());
        to_remove.thumbnail = Some("./images/compartida.png".into());
        let mut stays = game("./Global Defense.sms");
        stays.thumbnail = Some("./images/compartida.png".into());

        let plan = files_to_remove(&Layout::Classic, &system, &[to_remove, stays], &["./SDI.sms".into()]);
        assert_eq!(names(&plan[0].2, &system.rom_dir), vec!["covers/SDI.png", "images/SDI-image.png"]);
    }

    #[test]
    fn a_missing_rom_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("mastersystem"));
        let plan = files_to_remove(&Layout::Classic, &system, &[game("./Nada.sms")], &["./Nada.sms".into()]);
        assert!(plan[0].1.is_none());
        assert!(plan[0].2.is_empty());
    }
}
