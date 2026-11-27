use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::layout::{Layout, SystemRef};
use crate::media::{self, MEDIA_KINDS};
use crate::models::game::Game;

use super::progress::Progress;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// Some version has several discs ("(Disc 1)", "(Disc 2)"...): each version goes or stays
    /// with all its discs (only for the label)
    pub discs: bool,
    /// Version of each game (same order as `games`): the discs of one version (and its .m3u)
    /// share the number. `keep` stays with every game of its version
    pub versions: Vec<usize>,
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
pub(super) struct FoundGroup {
    pub(super) reason: &'static str,
    pub(super) detail: Option<String>,
    pub(super) members: Vec<usize>,
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
/// ScreenScraper ID, the hash (md5/hash), the DAT game (parent and clones, `dat_groups`: what
/// "Best version" used) or the normalized name (of the title or of the file), and a group is
/// everything linked, even through different reasons (A and B share the ID, B and C the name:
/// A, B and C). The reason shown is the strongest link inside the group: ID > hash > DAT > name.
pub(super) fn group_games(games: &[Game], dat_groups: &std::collections::HashMap<String, String>) -> Vec<FoundGroup> {
    let mut parent: Vec<usize> = (0..games.len()).collect();
    // Strength of each kind of link (the lower, the stronger) and the links found per kind
    const KINDS: [&str; 4] = ["screenscraper_id", "hash", "dat", "similar_name"];
    let stem = |g: &Game| std::path::Path::new(&g.path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut links: Vec<(usize, usize, usize)> = Vec::new();
    for kind in 0..KINDS.len() {
        let mut first: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (i, game) in games.iter().enumerate() {
            let keys: Vec<String> = match kind {
                0 => game.id.clone().into_iter().collect(),
                1 => game.md5.clone().or_else(|| game.hash.clone()).into_iter().collect(),
                2 => dat_groups.get(&game.path).cloned().into_iter().collect(),
                // The title and also the file name (two ROMs with the same base name)
                _ => vec![normalize_name(game.name.as_deref().unwrap_or(&game.path)), normalize_name(&stem(game))],
            };
            for key in keys.into_iter().filter(|k| !k.trim().is_empty()) {
                match first.get(&key) {
                    Some(&j) if j != i => links.push((kind, j, i)),
                    Some(_) => {}
                    None => {
                        first.insert(key, i);
                    }
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
            let kind = strongest.get(&r).copied().unwrap_or(KINDS.len() - 1);
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
    let (a, sp) = (app.clone(), system_path.clone());
    super::system_status::remember(&app, &system_path, "duplicates", "", move || find_duplicates_now(a, sp, task))
}

/// Lo que hace `find_duplicates` cuando no hay nada guardado que valga
fn find_duplicates_now(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<Vec<DuplicateGroup>, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let (_, system) = super::system_context(&app, &system_path)?;
    let gamelist = crate::commands::gamelist::load_gamelist(app.clone(), system_path, task)?;
    // The same ROM twice in the gamelist is not two versions: removing one would trash the ROM
    // the other one uses. Only its first entry counts here (they are merged by repeated_entries.rs)
    let mut seen = HashSet::new();
    let unique: Vec<Game> = gamelist.games.iter().filter(|g| seen.insert(g.path.clone())).cloned().collect();
    let games = &unique;
    // DAT groups (parent and clones, matched by content or by name), if the system has a DAT
    let dat_groups = super::dat::version_groups(&app, &system, games).unwrap_or_default();
    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let new_group = |reason: &str, detail: Option<String>, indices: &[usize]| {
        let members: Vec<Game> = indices.iter().map(|&i| games[i].clone()).collect();
        suggest(reason, detail, members, &system, &settings.scrape_regions(), &settings.scrape_languages())
    };
    // A group that is only the discs of one version is one game, not a duplicate
    groups.extend(group_games(games, &dat_groups).into_iter().filter_map(|found| new_group(found.reason, found.detail, &found.members)));

    Ok(groups)
}

/// Build a group with its suggestion: which version stays, how many media files each game has
/// and which ones are protected (favourite or played). Versions, not games, are compared: the
/// discs of one version (and its .m3u) go or stay together. None = the group is only one version
/// (the discs of one game), so there is nothing to choose.
fn suggest(reason: &str, detail: Option<String>, games: Vec<Game>, system: &SystemRef, regions: &[String], languages: &[String]) -> Option<DuplicateGroup> {
    let versions = version_of_each(&games);
    let count = versions.iter().max().map_or(0, |m| m + 1);
    if count < 2 {
        return None;
    }
    let media: Vec<usize> = games.iter().map(|g| media_count(system, g)).collect();
    // A protected disc protects its whole version
    let protected_version: Vec<bool> = (0..count).map(|v| games.iter().zip(&versions).any(|(g, &w)| w == v && super::best_version::is_protected(g))).collect();
    let protected = versions.iter().map(|&v| protected_version[v]).collect();

    // One sample game per version (its first disc) with the media of all its discs
    let discs_of = |v: usize| games.iter().zip(&versions).filter(|(g, &w)| w == v && disc_number(&g.path).is_some()).filter_map(|(g, _)| disc_number(&g.path)).collect::<HashSet<_>>().len();
    let most_discs = (0..count).map(discs_of).max().unwrap_or(0);
    // Versions with every disc first: one with a disc missing never stays over a complete one
    let candidates: Vec<usize> = (0..count).filter(|&v| discs_of(v) == most_discs).collect();
    let sample = |v: usize| versions.iter().position(|&w| w == v).unwrap_or(0);
    let samples: Vec<Game> = candidates.iter().map(|&v| games[sample(v)].clone()).collect();
    let sample_media: Vec<usize> = candidates.iter().map(|&v| versions.iter().zip(&media).filter(|(&w, _)| w == v).map(|(_, m)| m).sum()).collect();
    let (best, why) = super::best_version::pick_keeper(&samples, &sample_media, regions, languages);
    let why = if candidates.len() < count && candidates.len() == 1 { "complete" } else { why };
    Some(DuplicateGroup {
        reason: reason.to_string(),
        detail,
        keep: games[sample(candidates[best])].path.clone(),
        keep_reason: why.to_string(),
        protected,
        discs: most_discs > 1,
        versions,
        media,
        games,
    })
}

/// The file name without its disc tag, lower case and without extension: the discs of one
/// version share it ("FF7 (USA) (Disc 1).cue" and "(Disc 2)" -> "ff7 (usa)")
fn version_key(path: &str) -> String {
    static DISC_TAG: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)\s*\((disc|disk|cd)\s*([0-9]+|[a-z])\b[^)]*\)").unwrap());
    let stem = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    DISC_TAG.replace_all(&stem, "").trim().to_lowercase()
}

fn is_m3u(path: &str) -> bool {
    std::path::Path::new(path).extension().is_some_and(|e| e.eq_ignore_ascii_case("m3u"))
}

/// Version number of each game. The discs of one version share the key and the extension (a
/// .cue and a .chd of the same disc are two copies, two versions); a .m3u joins the discs with
/// its key. Two games with the same disc number are never the same version.
fn version_of_each(games: &[Game]) -> Vec<usize> {
    let mut versions: Vec<(String, String, HashSet<String>)> = Vec::new();
    let mut out = vec![0; games.len()];
    let ext = |p: &str| std::path::Path::new(p).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    // The discs first, then the .m3u files (they join an existing version)
    let mut order: Vec<usize> = (0..games.len()).collect();
    order.sort_by_key(|&i| is_m3u(&games[i].path));
    for i in order {
        let path = &games[i].path;
        let key = version_key(path);
        let found = if is_m3u(path) {
            versions.iter().position(|(k, _, _)| *k == key)
        } else {
            let (e, disc) = (ext(path), disc_number(path));
            versions.iter().position(|(k, x, discs)| *k == key && *x == e && disc.as_ref().is_some_and(|d| !discs.contains(d)))
        };
        let v = match found {
            Some(v) => v,
            None => {
                versions.push((key, ext(path), HashSet::new()));
                versions.len() - 1
            }
        };
        if let Some(d) = disc_number(path) {
            versions[v].2.insert(d);
        }
        out[i] = v;
    }
    out
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

/// Borra un fichero o carpeta para siempre (sin papelera)
fn delete_for_good(path: &Path) -> Result<(), String> {
    let r = if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
    r.map_err(|e| e.to_string())
}

/// Manda a la papelera (o borra para siempre, con `permanent`) un fichero o carpeta
fn remove_one(path: &Path, permanent: bool) -> Result<(), String> {
    if permanent {
        delete_for_good(path)
    } else {
        trash::delete(path).map_err(|e| e.to_string())
    }
}

/// Manda a la papelera los ficheros de una tanda de juegos. Normalmente todos de una vez; si
/// eso falla, uno a uno, para que un juego cuya ROM no se pueda mandar se quede en la gamelist.
/// Con `permanent` se borran de verdad, siempre uno a uno (no hay borrado «de golpe»).
fn remove_batch(batch: &[(String, Option<PathBuf>, Vec<PathBuf>)], permanent: bool, removed_paths: &mut HashSet<String>, failed: &mut Vec<String>) {
    let all: Vec<PathBuf> = batch.iter().flat_map(|(_, rom, media)| rom.iter().chain(media.iter()).cloned()).collect();
    if all.is_empty() || (!permanent && trash::delete_all(&all).is_ok()) {
        removed_paths.extend(batch.iter().map(|(game_path, _, _)| game_path.clone()));
        return;
    }
    for (game_path, rom, media_files) in batch {
        // The ROM first: if it can't go to the trash, the game stays
        if let Some(rom) = rom {
            if !rom.exists() {
                // Already sent by the bulk attempt before it failed
            } else if let Err(e) = remove_one(rom, permanent) {
                failed.push(format!("{} ({})", rom.display(), e));
                continue;
            }
        }
        removed_paths.insert(game_path.clone());
        for file in media_files.iter().filter(|f| f.exists()) {
            if let Err(e) = remove_one(file, permanent) {
                failed.push(format!("{} ({})", file.display(), e));
            }
        }
    }
}

/// Remove duplicates for good: take the games out of the gamelist and send their ROM file and
/// all their media to the trash. With `dry_run` nothing is touched: it only lists the files
/// (the UI shows the count before asking for confirmation). With `permanent` the files are
/// deleted for good instead of going to the trash ("Versions and duplicates" offers it).
/// A game whose ROM can't be trashed stays in the gamelist, so no ROM is left without entry.
#[tauri::command(async)]
pub fn remove_games_with_files(
    app: tauri::AppHandle,
    system_path: String,
    game_paths: Vec<String>,
    dry_run: bool,
    permanent: Option<bool>,
    task: Option<String>,
) -> Result<RemoveResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    remove_games_in(&layout, &system, &game_paths, dry_run, permanent.unwrap_or(false), &progress)
}

/// The work of `remove_games_with_files`, without the app (tests call it directly)
fn remove_games_in(
    layout: &Layout,
    system: &SystemRef,
    game_paths: &[String],
    dry_run: bool,
    permanent: bool,
    progress: &Progress,
) -> Result<RemoveResult, String> {
    let mut gamelist = super::gamelist::read_gamelist_with(layout, system, progress)?;
    let plan = files_to_remove(layout, system, &gamelist.games, game_paths);

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
        progress.step(if permanent { "deleting" } else { "trashing" }, done_games, plan.len(), &batch[0].0);
        remove_batch(batch, permanent, &mut removed_paths, &mut result.failed);
    }

    if !removed_paths.is_empty() {
        progress.phase("saving");
        gamelist.games.retain(|g| !removed_paths.contains(&g.path));
        // write_gamelist() backs up the gamelist first
        super::gamelist::write_gamelist(layout, system, &gamelist)?;
        // And out of the custom collections and .m3u playlists that listed them
        let removed: Vec<String> = removed_paths.iter().cloned().collect();
        super::rename::remove_references(layout, system, &removed);
    }
    result.removed = removed_paths.len();
    Ok(result)
}

/// For each game to remove: its path, its ROM (file or folder, if it exists) and its media files.
/// Media = every file named after the ROM in each media folder (any extension) and, in Classic,
/// the files its media fields point to, unless a game that stays also uses them.
pub(super) fn files_to_remove(
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
        let groups = group_games(&games, &Default::default());
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], FoundGroup { reason: "screenscraper_id", detail: Some("1268".into()), members: vec![0, 1, 2] });
        assert_eq!(groups[1], FoundGroup { reason: "similar_name", detail: None, members: vec![3, 4] });
    }

    #[test]
    fn the_dat_and_the_file_name_also_link_versions() {
        let games = vec![
            named("./Street Fighter II (USA).sfc", "Street Fighter II", None),
            named("./Street Fighter II Turbo (Japan).sfc", "SF2 Turbo", None),
            // Different titles, same file name without tags
            named("./Contra (USA).nes", "Contra", None),
            named("./Contra (Europe).nes", "Probotector", None),
        ];
        let dat: std::collections::HashMap<String, String> =
            [("./Street Fighter II (USA).sfc", "sf2"), ("./Street Fighter II Turbo (Japan).sfc", "sf2")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        let groups = group_games(&games, &dat);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], FoundGroup { reason: "dat", detail: None, members: vec![0, 1] });
        assert_eq!(groups[1], FoundGroup { reason: "similar_name", detail: None, members: vec![2, 3] });
    }

    #[test]
    fn discs_of_one_game_are_told_apart_from_copies() {
        assert_eq!(disc_number("./FF7 (USA) (Disc 2).cue").as_deref(), Some("2"));
        assert_eq!(disc_number("./FF7 (Disk 1 of 3).chd").as_deref(), Some("1"));
        assert_eq!(disc_number("./Game (CD2).iso").as_deref(), Some("2"));
        assert_eq!(disc_number("./Sonic (USA).md"), None);
        // Discs of one version share it; the same disc in two regions or two formats are two versions
        let v = |paths: &[&str]| version_of_each(&paths.iter().map(|p| game(p)).collect::<Vec<_>>());
        assert_eq!(v(&["./FF7 (Disc 1).cue", "./FF7 (Disc 2).cue", "./FF7.m3u"]), vec![0, 0, 0]);
        assert_eq!(v(&["./FF7 (USA) (Disc 1).cue", "./FF7 (Europe) (Disc 1).cue"]), vec![0, 1]);
        assert_eq!(v(&["./FF7 (Disc 1).cue", "./FF7 (Disc 1).chd"]), vec![0, 1]);
        assert_eq!(v(&["./Sonic (USA).md", "./Sonic (Europe).md"]), vec![0, 1]);
        assert_eq!(version_key("./Discovery (Discovery Channel).iso"), "discovery (discovery channel)");
    }

    #[test]
    fn a_missing_rom_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("mastersystem"));
        let plan = files_to_remove(&Layout::Classic, &system, &[game("./Nada.sms")], &["./Nada.sms".into()]);
        assert!(plan[0].1.is_none());
        assert!(plan[0].2.is_empty());
    }

    fn suggest_for(paths: &[&str]) -> Option<DuplicateGroup> {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path());
        let games = paths.iter().map(|p| game(p)).collect();
        suggest("similar_name", None, games, &system, &["eu".to_string()], &["es".to_string()])
    }

    #[test]
    fn the_discs_of_one_version_are_not_a_duplicate() {
        assert!(suggest_for(&["./Atlantis (Europe) (Disc 1).chd", "./Atlantis (Europe) (Disc 2).chd", "./Atlantis (Europe) (Disc 3).chd"]).is_none());
    }

    #[test]
    fn versions_with_several_discs_go_or_stay_whole() {
        let g = suggest_for(&["./Legend (USA) (Disc 1).chd", "./Legend (USA) (Disc 2).chd", "./Legend (Europe) (Disc 1).chd", "./Legend (Europe) (Disc 2).chd"]).unwrap();
        assert!(g.discs);
        assert_eq!(g.versions, vec![0, 0, 1, 1]);
        assert_eq!(g.keep, "./Legend (Europe) (Disc 1).chd", "the user's region");
        // A complete version stays over the user's region with a disc missing
        let g = suggest_for(&["./Legend (USA) (Disc 1).chd", "./Legend (USA) (Disc 2).chd", "./Legend (Europe) (Disc 1).chd"]).unwrap();
        assert_eq!(g.keep, "./Legend (USA) (Disc 1).chd");
        assert_eq!(g.keep_reason, "complete");
    }

    #[test]
    fn a_protected_disc_protects_its_version() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path());
        let mut games: Vec<Game> = ["./L (USA) (Disc 1).chd", "./L (USA) (Disc 2).chd", "./L (Europe) (Disc 1).chd", "./L (Europe) (Disc 2).chd"].iter().map(|p| game(p)).collect();
        games[1].favorite = Some("true".into());
        let g = suggest("similar_name", None, games, &system, &["eu".to_string()], &[]).unwrap();
        assert_eq!(g.protected, vec![true, true, false, false]);
    }

    #[test]
    fn permanent_removal_deletes_the_files_without_the_trash() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("mastersystem"));
        touch(&system.rom_dir.join("SDI.sms"));
        touch(&system.rom_dir.join("covers/SDI.png"));
        touch(&system.rom_dir.join("Global Defense.sms"));
        let gamelist = crate::models::gamelist::GameList { games: vec![game("./SDI.sms"), game("./Global Defense.sms")], ..Default::default() };
        crate::commands::gamelist::write_gamelist(&Layout::Classic, &system, &gamelist).unwrap();

        let r = remove_games_in(&Layout::Classic, &system, &["./SDI.sms".into()], false, true, &Progress::none()).unwrap();
        assert_eq!(r.removed, 1);
        assert!(r.failed.is_empty());
        // Borrados de verdad: ni la ROM ni su portada siguen ahí
        assert!(!system.rom_dir.join("SDI.sms").exists());
        assert!(!system.rom_dir.join("covers/SDI.png").exists());
        assert!(system.rom_dir.join("Global Defense.sms").exists());
        let left = crate::commands::gamelist::read_gamelist(&Layout::Classic, &system).unwrap();
        assert_eq!(left.games.iter().map(|g| g.path.as_str()).collect::<Vec<_>>(), vec!["./Global Defense.sms"]);
    }
}
