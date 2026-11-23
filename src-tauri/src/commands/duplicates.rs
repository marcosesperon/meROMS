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
    /// Suggested game to keep ("Mark automatically"), by path
    pub keep: String,
    /// Why it stays: best_version reasons (not_unwanted, region, language, revision, verified,
    /// shortest) + "media" | "data"
    pub keep_reason: String,
    /// Media files of each game (same order as `games`); a broken Classic path doesn't count
    pub media: Vec<usize>,
    /// Favourite or played (same order as `games`): never marked automatically
    pub protected: Vec<bool>,
    /// Different discs of one game ("(Disc 1)", "(Disc 2)"...): not marked automatically
    pub discs: bool,
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
/// lowercase, strip parenthetical tags like (USA), (Rev 1), (En,Fr), [!], etc. and a leading
/// compilation number ("0013 Street Fighter 2" = "Street Fighter 2"; years like 1943 stay),
/// with the same condition as Rename's rule, so names that clash there are grouped here.
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
    let s = s.trim();
    let s = super::rename::leading_number(s).map_or(s, |number| &s[number.len()..]);
    s.trim().to_string()
}

/// A group found by `group_games`: why its games look the same and which ones (indices)
#[derive(Debug, PartialEq)]
struct FoundGroup {
    reason: &'static str,
    detail: Option<String>,
    members: Vec<usize>,
}

/// Union-find: each game points to another one of its group until the root
fn root(parent: &mut [usize], i: usize) -> usize {
    let mut r = i;
    while parent[r] != r {
        r = parent[r];
    }
    // Shorten the way for next time
    let mut j = i;
    while parent[j] != r {
        let next = parent[j];
        parent[j] = r;
        j = next;
    }
    r
}

/// Groups of games that look like the same one. Two games are linked when they share the
/// ScreenScraper ID, the hash (md5/hash) or the normalized name, and a group is everything linked,
/// even through different reasons (A and B share the ID, B and C the name: A, B and C). The reason
/// shown is the strongest link inside the group: ID > hash > name.
fn group_games(games: &[Game]) -> Vec<FoundGroup> {
    let mut parent: Vec<usize> = (0..games.len()).collect();
    // Strength of each kind of link (the lower, the stronger) and the links found per kind
    const KINDS: [&str; 3] = ["screenscraper_id", "hash", "similar_name"];
    let mut links: Vec<(usize, usize, usize)> = Vec::new();
    for (kind, key_of) in [
        (0, (|g: &Game| g.id.clone()) as fn(&Game) -> Option<String>),
        (1, |g: &Game| g.md5.clone().or_else(|| g.hash.clone())),
        (2, |g: &Game| Some(normalize_name(g.name.as_deref().unwrap_or(&g.path)))),
    ] {
        let mut first: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (i, game) in games.iter().enumerate() {
            let Some(key) = key_of(game).filter(|k| !k.trim().is_empty()) else { continue };
            match first.get(&key) {
                Some(&j) => links.push((kind, j, i)),
                None => {
                    first.insert(key, i);
                }
            }
        }
    }
    for &(_, a, b) in &links {
        let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
        if ra != rb {
            parent[rb] = ra;
        }
    }

    // Members of each group (in gamelist order) and its strongest link
    let mut members: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for i in 0..games.len() {
        let r = root(&mut parent, i);
        members.entry(r).or_default().push(i);
    }
    let mut strongest: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for &(kind, a, _) in &links {
        let r = root(&mut parent, a);
        let best = strongest.entry(r).or_insert(kind);
        *best = (*best).min(kind);
    }
    let mut groups: Vec<FoundGroup> = members
        .into_iter()
        .filter(|(_, m)| m.len() > 1)
        .map(|(r, m)| {
            let kind = strongest.get(&r).copied().unwrap_or(2);
            // With the ID as reason, the ID shared by most of them is the detail
            let detail = (kind == 0).then(|| m.iter().find_map(|&i| games[i].id.clone())).flatten();
            FoundGroup { reason: KINDS[kind], detail, members: m }
        })
        .collect();
    // Same order as before: by ID first, then hash, then name
    groups.sort_by_key(|g| KINDS.iter().position(|k| *k == g.reason));
    groups
}

#[tauri::command(async)]
pub fn find_duplicates(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<Vec<DuplicateGroup>, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let (_, system) = super::system_context(&app, &system_path)?;
    let gamelist = crate::commands::gamelist::load_gamelist(app, system_path, task)?;
    let games = &gamelist.games;
    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let new_group = |reason: &str, detail: Option<String>, indices: &[usize]| {
        let members: Vec<Game> = indices.iter().map(|&i| games[i].clone()).collect();
        suggest(reason, detail, members, &system, &settings.scrape_regions(), &settings.scrape_languages())
    };
    for found in group_games(games) {
        groups.push(new_group(found.reason, found.detail, &found.members));
    }

    Ok(groups)
}

/// Build a group with its suggestion: which game stays (best_version::pick_keeper), how many
/// media files each one has and which ones are protected (favourite or played).
fn suggest(reason: &str, detail: Option<String>, games: Vec<Game>, system: &SystemRef, regions: &[String], languages: &[String]) -> DuplicateGroup {
    let media: Vec<usize> = games.iter().map(|g| media_count(system, g)).collect();
    let (keep, why) = super::best_version::pick_keeper(&games, &media, regions, languages);
    DuplicateGroup {
        reason: reason.to_string(),
        detail,
        keep: games[keep].path.clone(),
        keep_reason: why.to_string(),
        protected: games.iter().map(super::best_version::is_protected).collect(),
        discs: different_discs(&games),
        media,
        games,
    }
}

/// Disc number from the file name: "(Disc 2)", "(Disk 2 of 3)", "(CD2)" -> "2"
fn disc_number(path: &str) -> Option<String> {
    let stem = std::path::Path::new(path).file_stem()?.to_string_lossy().to_lowercase();
    ["(disc ", "(disk ", "(cd"].iter().find_map(|tag| {
        let rest = &stem[stem.find(tag)? + tag.len()..];
        let number: String = rest.trim_start().chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        (!number.is_empty()).then_some(number)
    })
}

/// True when the group has at least two different disc numbers (they are not copies)
fn different_discs(games: &[Game]) -> bool {
    let numbers: HashSet<String> = games.iter().filter_map(|g| disc_number(&g.path)).collect();
    numbers.len() > 1
}

/// Media files of a game: its media fields that point to an existing file (in ES-DE the
/// gamelist read already filled them from the files named after the ROM)
fn media_count(system: &SystemRef, game: &Game) -> usize {
    media::media_fields()
        .filter_map(|f| media::get_field(game, f))
        .filter(|v| !v.trim().is_empty() && media::resolve_media_path(system, v).is_file())
        .count()
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

    fn named(path: &str, name: &str, id: Option<&str>) -> Game {
        Game { path: path.into(), name: Some(name.into()), id: id.map(Into::into), ..Default::default() }
    }

    #[test]
    fn the_compilation_number_is_left_out_of_the_name_but_not_years() {
        assert_eq!(normalize_name("0013 Street Fighter 2"), "street fighter 2");
        assert_eq!(normalize_name("478 Red Pig (Chinese)"), "red pig");
        assert_eq!(normalize_name("1943 The Battle of Midway"), "1943 the battle of midway");
        assert_eq!(normalize_name("1942"), "1942");
    }

    #[test]
    fn games_linked_by_different_reasons_end_up_in_one_group() {
        let games = vec![
            named("./0082 Castlevania 1.nes", "Castlevania", Some("1268")),
            named("./CASTLE1.NES", "Castlevania", Some("1268")),
            // Sin ID: solo se parece por el nombre a los de arriba
            named("./069 Castlevania.nes", "069 Castlevania", None),
            named("./0013 Street Fighter 2.nes", "0013 Street Fighter 2", None),
            named("./Street Fighter 2.nes", "Street Fighter 2", Some("143329")),
            named("./Otro.nes", "Otro", Some("5")),
        ];
        let groups = group_games(&games);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], FoundGroup { reason: "screenscraper_id", detail: Some("1268".into()), members: vec![0, 1, 2] });
        assert_eq!(groups[1], FoundGroup { reason: "similar_name", detail: None, members: vec![3, 4] });
    }

    #[test]
    fn discs_of_one_game_are_told_apart_from_copies() {
        assert_eq!(disc_number("./FF7 (USA) (Disc 2).cue").as_deref(), Some("2"));
        assert_eq!(disc_number("./FF7 (Disk 1 of 3).chd").as_deref(), Some("1"));
        assert_eq!(disc_number("./Game (CD2).iso").as_deref(), Some("2"));
        assert_eq!(disc_number("./Sonic (USA).md"), None);
        assert!(different_discs(&[game("./FF7 (Disc 1).cue"), game("./FF7 (Disc 2).cue")]));
        // Same disc in two regions: copies, so they can be marked
        assert!(!different_discs(&[game("./FF7 (USA) (Disc 1).cue"), game("./FF7 (Europe) (Disc 1).cue")]));
        assert!(!different_discs(&[game("./Sonic (USA).md"), game("./Sonic (Europe).md")]));
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
