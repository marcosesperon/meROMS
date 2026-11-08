use std::fs;
use std::path::Path;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::layout::{Layout, SystemRef};
use crate::media;
use crate::models::GameList;
use crate::rename_rules::CompiledRules;

use super::progress::Progress;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameEntry {
    /// Current relative ROM path (e.g. "./Super Mario World.sfc")
    pub current_path: String,
    /// Current filename without extension
    pub current_name: String,
    /// Game title from gamelist.xml (None if no title)
    pub game_title: Option<String>,
    /// Proposed new filename (sanitized title, or the name cleaned by the rules)
    pub proposed_name: String,
    /// Whether a rename makes sense (there is a proposal and it differs from the current name)
    pub can_rename: bool,
    /// Tracks of a .cue/.gdi renamed along with it (the ones named after the game)
    #[serde(default)]
    pub tracks: usize,
    /// Why the proposal can't be applied as is: "duplicate" (another ROM would get the
    /// same name) or "exists" (a different file already has that name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<String>,
}

/// Where the proposed name comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RenameSource {
    /// Game title from the gamelist (meROMS' original behaviour)
    #[default]
    Title,
    /// Current file name cleaned with the regex rules (ES-DE-Roms-Renamer style)
    Rules,
    /// Official name from the system's DAT, for ROMs recognised by content (CRC in the cache)
    Dat,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenameResult {
    pub gamelist: GameList,
    pub renamed: usize,
    /// ROMs not renamed (source missing or destination taken)
    pub skipped: Vec<String>,
}

/// Characters not allowed in filenames (on any of the usual systems).
fn is_forbidden(c: char) -> bool {
    matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
}

/// Characters not allowed in filenames are replaced by a hyphen. A title separator like
/// "Contra III: The Alien Wars" becomes "Contra III - The Alien Wars" (not "Contra III_ The…").
fn sanitize_filename(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len());
    for (i, &c) in chars.iter().enumerate() {
        if !is_forbidden(c) {
            out.push(c);
            continue;
        }
        // "palabra: siguiente" → "palabra - siguiente" (espacio antes del guion si falta)
        let followed_by_space = chars.get(i + 1).is_some_and(|n| n.is_whitespace());
        if followed_by_space && !out.ends_with(' ') && !out.is_empty() {
            out.push(' ');
        }
        out.push('-');
    }
    // Sin espacios dobles ni guiones colgando al principio o al final
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim_matches(|c: char| c == '-' || c.is_whitespace()).to_string()
}

/// List ROMs with a proposed new name, from the gamelist title or the cleanup rules.
/// With `game_path` only that game is proposed (game detail panel): the other ROMs keep
/// their names, so only files already on disk can clash with it.
#[tauri::command(async)]
pub fn list_rename_candidates(
    app: tauri::AppHandle,
    system_path: String,
    source: Option<RenameSource>,
    game_path: Option<String>,
    task: Option<String>,
) -> Result<Vec<RenameEntry>, String> {
    let settings = crate::commands::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    let gamelist = crate::commands::gamelist::read_gamelist_with(&layout, &system, &progress)?;
    let rules = CompiledRules::new(&settings.rename_rules_or_default());
    let source = source.unwrap_or_default();
    let wanted = |game: &&crate::models::Game| game_path.as_ref().is_none_or(|only| &game.path == only);
    let rom_of = |game: &crate::models::Game| system.rom_dir.join(game.path.strip_prefix("./").unwrap_or(&game.path));

    // «Nombre del DAT»: el del juego que el DAT reconoce por contenido (solo lo que hay en caché)
    let dat_names = if source == RenameSource::Dat {
        let roms: Vec<_> = gamelist.games.iter().filter(wanted).map(rom_of).collect();
        crate::commands::dat::names_by_content(&app, &system, &roms)?
    } else {
        HashMap::new()
    };

    let chosen: Vec<_> = gamelist.games.iter().filter(wanted).collect();
    let mut entries: Vec<RenameEntry> = Vec::with_capacity(chosen.len());
    for (i, game) in chosen.iter().enumerate() {
        progress.check()?;
        progress.step("games", i + 1, chosen.len(), &game.path);
        entries.push(propose(game, source, &rules, dat_names.get(&rom_of(game)).map(String::as_str)));
    }
    progress.phase("checking");
    mark_conflicts(&mut entries, &system.rom_dir);
    mark_track_conflicts(&mut entries, &system.rom_dir);

    entries.sort_by(|a, b| a.current_name.to_lowercase().cmp(&b.current_name.to_lowercase()));
    Ok(entries)
}

fn propose(game: &crate::models::Game, source: RenameSource, rules: &CompiledRules, dat_name: Option<&str>) -> RenameEntry {
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
    let proposed = match source {
        RenameSource::Title => game_title.as_deref().map(sanitize_filename),
        RenameSource::Rules => Some(sanitize_filename(&rules.apply(&current_name))),
        RenameSource::Dat => dat_name.map(sanitize_filename),
    }
    .filter(|p| !p.is_empty())
    .unwrap_or_else(|| current_name.clone());

    let can_rename = proposed != current_name && !ext.is_empty();

    RenameEntry {
        current_path: game.path.clone(),
        current_name,
        game_title,
        proposed_name: proposed,
        can_rename,
        tracks: 0,
        conflict: None,
    }
}

/// Flag proposals that can't be applied: two ROMs ending with the same name, or a
/// different file already using it. Compared case-insensitively (macOS/Windows disks are).
fn mark_conflicts(entries: &mut [RenameEntry], rom_dir: &Path) {
    let target_of = |e: &RenameEntry| -> String {
        let current = Path::new(e.current_path.strip_prefix("./").unwrap_or(&e.current_path));
        let ext = current.extension().and_then(|x| x.to_str()).unwrap_or("");
        current
            .with_file_name(format!("{}.{}", e.proposed_name, ext))
            .to_string_lossy()
            .to_lowercase()
    };

    let mut counts: HashMap<String, usize> = HashMap::new();
    for e in entries.iter() {
        // ROMs that keep their name also "occupy" it
        let key = if e.can_rename { target_of(e) } else { e.current_path.trim_start_matches("./").to_lowercase() };
        *counts.entry(key).or_default() += 1;
    }

    for e in entries.iter_mut().filter(|e| e.can_rename) {
        let target = target_of(e);
        let own = e.current_path.trim_start_matches("./").to_lowercase();
        if counts.get(&target).copied().unwrap_or(0) > 1 {
            e.conflict = Some("duplicate".into());
        } else if target != own && rom_dir.join(&target).exists() {
            e.conflict = Some("exists".into());
        }
    }
}

/// Check a name typed by hand for one ROM (game detail panel).
/// Returns None if it can be used, or why not: "invalid" (empty or with characters not
/// allowed in file names) or "exists" (another file already has that name).
#[tauri::command(async)]
pub fn check_rename_target(
    app: tauri::AppHandle,
    system_path: String,
    game_path: String,
    new_name: String,
) -> Result<Option<String>, String> {
    let (_, system) = super::system_context(&app, &system_path)?;
    Ok(target_conflict(&system.rom_dir, &game_path, &new_name))
}

fn target_conflict(rom_dir: &Path, game_path: &str, new_name: &str) -> Option<String> {
    // Vacío, con caracteres prohibidos o con espacios al principio/final
    if new_name.trim().is_empty() || new_name.trim() != new_name || new_name.chars().any(is_forbidden) {
        return Some("invalid".into());
    }
    let mut entry = [RenameEntry {
        current_path: game_path.to_string(),
        current_name: String::new(),
        game_title: None,
        proposed_name: new_name.to_string(),
        can_rename: true,
        tracks: 0,
        conflict: None,
    }];
    mark_conflicts(&mut entry, rom_dir);
    mark_track_conflicts(&mut entry, rom_dir);
    entry[0].conflict.take()
}

/// Una pista de un .cue/.gdi que se renombra con él
#[derive(Debug, Clone, PartialEq)]
struct TrackRename {
    from: std::path::PathBuf,
    to: std::path::PathBuf,
    /// Cómo está escrita en la hoja y cómo quedará
    old_ref: String,
    new_ref: String,
}

/// Pistas de un .cue/.gdi cuyo nombre empieza por el nombre antiguo del juego (seguido del
/// final, un espacio, un punto o un paréntesis): «Juego (USA) (Track 1).bin» → «Juego (Track 1).bin».
/// Las de nombre genérico (track01.bin) no. Si la hoja no se lee como UTF-8, ninguna.
fn track_renames(rom: &Path, old_stem: &str, new_stem: &str) -> Vec<TrackRename> {
    if !matches!(media::extension_lower(rom).as_deref(), Some("cue" | "gdi")) || old_stem == new_stem || old_stem.is_empty() {
        return Vec::new();
    }
    let Some(dir) = rom.parent() else { return Vec::new() };
    super::move_games::sheet_names(rom)
        .into_iter()
        .filter_map(|old_ref| {
            let cut = old_ref.rfind(['/', '\\']).map_or(0, |i| i + 1);
            let (folder, file) = old_ref.split_at(cut);
            let rest = file.strip_prefix(old_stem)?;
            if !(rest.is_empty() || rest.starts_with([' ', '.', '('])) {
                return None;
            }
            let new_ref = format!("{}{}{}", folder, new_stem, rest);
            let from = dir.join(old_ref.replace('\\', "/"));
            from.is_file().then(|| TrackRename { to: dir.join(new_ref.replace('\\', "/")), from, old_ref, new_ref })
        })
        .collect()
}

/// Cuenta las pistas de cada propuesta y marca conflicto si el nombre nuevo de alguna ya existe
fn mark_track_conflicts(entries: &mut [RenameEntry], rom_dir: &Path) {
    for e in entries.iter_mut().filter(|e| e.can_rename) {
        let rom = rom_dir.join(e.current_path.strip_prefix("./").unwrap_or(&e.current_path));
        let old_stem = rom.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let tracks = track_renames(&rom, &old_stem, &e.proposed_name);
        e.tracks = tracks.len();
        if e.conflict.is_none() && tracks.iter().any(|t| t.to.exists() && t.to != t.from) {
            e.conflict = Some("exists".into());
        }
    }
}

/// Cambia en una línea de la hoja el nombre antiguo de una pista por el nuevo: entre comillas,
/// o como palabra suelta (sin comillas)
fn replace_ref(line: &str, old: &str, new: &str) -> String {
    let quoted = format!("\"{}\"", old);
    if line.contains(&quoted) {
        return line.replacen(&quoted, &format!("\"{}\"", new), 1);
    }
    let words: Vec<&str> = line.split(' ').collect();
    if words.contains(&old) {
        return words.iter().map(|w| if *w == old { new } else { *w }).collect::<Vec<_>>().join(" ");
    }
    line.to_string()
}

/// Reescribe la hoja (.cue/.gdi) con los nombres nuevos de las pistas, conservando el resto
/// (y los finales de línea). Apunta el contenido de antes para «Deshacer».
fn rewrite_sheet(sheet: &Path, tracks: &[TrackRename]) {
    let Ok(content) = fs::read_to_string(sheet) else { return };
    let mut out = String::with_capacity(content.len());
    for piece in content.split_inclusive('\n') {
        let (line, ending) = match piece.strip_suffix("\r\n") {
            Some(l) => (l, "\r\n"),
            None => match piece.strip_suffix('\n') {
                Some(l) => (l, "\n"),
                None => (piece, ""),
            },
        };
        let mut changed = line.to_string();
        for t in tracks {
            changed = replace_ref(&changed, &t.old_ref, &t.new_ref);
        }
        out.push_str(&changed);
        out.push_str(ending);
    }
    if out != content {
        media::journal_rewrite(sheet, content.into_bytes());
        let _ = fs::write(sheet, out);
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameRequest {
    pub current_path: String,
    pub new_name: String,
}

/// Rename ROM files and update all references in gamelist.xml.
/// Media are renamed too: the unified `<folder>/<ROM name>.<ext>` files in both structures,
/// plus (Classic) any other file the gamelist points to that is named after the ROM.
#[tauri::command(async)]
pub fn rename_roms(
    app: tauri::AppHandle,
    system_path: String,
    renames: Vec<RenameRequest>,
    task: Option<String>,
) -> Result<RenameResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let sys_path = system.rom_dir.clone();
    let progress = Progress::new(&app, task);

    // write_gamelist() backs the gamelist up before saving
    let mut gamelist = crate::commands::gamelist::read_gamelist_with(&layout, &system, &progress)?;
    // Para «Deshacer»: la gamelist de antes y el diario de ficheros movidos
    let before = gamelist.clone();
    media::journal_begin();
    let mut done: Vec<(String, String)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for (i, req) in renames.iter().enumerate() {
        // Cancelado: se para aquí y se guarda lo ya renombrado (Deshacer lo cubre)
        if progress.cancelled() {
            break;
        }
        progress.step("renaming", i + 1, renames.len(), &req.current_path);
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
        if !old_abs.exists() || (new_abs.exists() && old_abs != new_abs) {
            skipped.push(old_rel.to_string());
            continue;
        }
        let rom_is_dir = old_abs.is_dir();

        // Pistas de un .cue/.gdi con el nombre del juego: si alguna chocaría, el juego se salta
        let tracks = track_renames(&old_abs, old_stem, &req.new_name);
        if tracks.iter().any(|t| t.to.exists() && t.to != t.from) {
            skipped.push(old_rel.to_string());
            continue;
        }

        // Rename the ROM file
        if old_abs != new_abs {
            fs::rename(&old_abs, &new_abs)
                .map_err(|e| format!("Failed to rename {}: {}", old_rel, e))?;
            media::journal_move(&old_abs, &new_abs);
        }

        // …y sus pistas, con la hoja reescrita con los nombres nuevos
        let mut renamed_tracks = Vec::new();
        for t in tracks {
            if fs::rename(&t.from, &t.to).is_ok() {
                media::journal_move(&t.from, &t.to);
                renamed_tracks.push(t);
            }
        }
        if !renamed_tracks.is_empty() {
            rewrite_sheet(&new_abs, &renamed_tracks);
        }

        let moved = media::rename_rom_media(&layout, &system, &req.current_path, &new_relative, rom_is_dir);
        done.push((req.current_path.clone(), new_relative.clone()));

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

    progress.phase("saving");
    crate::commands::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    update_references(&layout, &system, &done);

    let sys = system.rom_dir.to_string_lossy().to_string();
    super::undo::record(super::undo::UndoOp {
        kind: "rename".into(),
        system: sys.clone(),
        files: media::journal_take(),
        games: super::undo::changes_by_path(&sys, &before, &sys, &gamelist, &done),
        refs: super::undo::RefUndo::Rename { system: sys.clone(), pairs: done.clone() },
    });

    Ok(RenameResult {
        // Re-read so ES-DE media fields reflect the renamed files
        gamelist: crate::commands::gamelist::read_gamelist(&layout, &system)?,
        renamed: done.len(),
        skipped,
    })
}

/// After renaming ROMs, fix what points to them by file name:
/// custom collections (where the active structure keeps them) and .m3u playlists.
pub(super) fn update_references(layout: &Layout, system: &SystemRef, renamed: &[(String, String)]) {
    if renamed.is_empty() {
        return;
    }
    // Game statuses are kept by ROM path: they follow the new name
    super::game_status::remap(&system.rom_dir, &system.rom_dir, renamed);

    // Collections: entries are written exactly as collection_entry() builds them
    let collection_renames: HashMap<String, String> = renamed
        .iter()
        .map(|(old, new)| (layout.collection_entry(system, old), layout.collection_entry(system, new)))
        .collect();
    let collections_dir = layout.collections_dir(system);
    if let Ok(entries) = fs::read_dir(&collections_dir) {
        for path in entries.flatten().map(|e| e.path()) {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if name.starts_with("custom-") && name.ends_with(".cfg") {
                rewrite_lines(&path, |line| collection_renames.get(line.trim()).cloned());
            }
        }
    }

    // .m3u playlists: lines are paths relative to the playlist's own folder
    let renamed_abs: HashMap<std::path::PathBuf, String> = renamed
        .iter()
        .map(|(old, new)| {
            let new_name = Path::new(new).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            (system.rom_dir.join(crate::layout::strip_dot_slash(old)), new_name)
        })
        .collect();
    for m3u in media::list_files_recursive(&system.rom_dir)
        .into_iter()
        .filter(|p| media::extension_lower(p).as_deref() == Some("m3u"))
    {
        let Some(dir) = m3u.parent().map(Path::to_path_buf) else { continue };
        rewrite_lines(&m3u, |line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            let new_name = renamed_abs.get(&dir.join(trimmed))?;
            Some(Path::new(trimmed).with_file_name(new_name).to_string_lossy().replace('\\', "/"))
        });
    }
}

/// After removing games (Duplicates, Games without ROM): drop the lines that pointed to their
/// ROMs from the custom collections and from the .m3u playlists of the system folder.
/// A playlist left without discs stays (empty), it isn't deleted.
pub(super) fn remove_references(layout: &Layout, system: &SystemRef, removed: &[String]) {
    if removed.is_empty() {
        return;
    }
    super::game_status::forget(&system.rom_dir, removed);

    let entries: std::collections::HashSet<String> = removed.iter().map(|p| layout.collection_entry(system, p)).collect();
    if let Ok(files) = fs::read_dir(layout.collections_dir(system)) {
        for path in files.flatten().map(|e| e.path()) {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if name.starts_with("custom-") && name.ends_with(".cfg") {
                remove_lines(&path, |line| entries.contains(line.trim()));
            }
        }
    }

    let removed_abs: std::collections::HashSet<std::path::PathBuf> =
        removed.iter().map(|p| system.rom_dir.join(crate::layout::strip_dot_slash(p))).collect();
    for m3u in media::list_files_recursive(&system.rom_dir)
        .into_iter()
        .filter(|p| media::extension_lower(p).as_deref() == Some("m3u"))
    {
        let Some(dir) = m3u.parent().map(Path::to_path_buf) else { continue };
        remove_lines(&m3u, |line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && removed_abs.contains(&dir.join(trimmed))
        });
    }
}

/// Remove the lines of a text file for which `drop` is true (keeping its line endings).
/// Only writes if something was removed.
pub(super) fn remove_lines(path: &Path, drop: impl Fn(&str) -> bool) {
    let Ok(content) = fs::read_to_string(path) else { return };
    let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let kept: Vec<&str> = content.lines().filter(|l| !drop(l)).collect();
    if kept.len() == content.lines().count() {
        return;
    }
    let mut out = kept.join(newline);
    if !kept.is_empty() && content.ends_with('\n') {
        out.push_str(newline);
    }
    let _ = fs::write(path, out);
}

/// Rewrite a text file line by line (keeping its line endings); `change` returns the
/// new content for a line, or None to leave it as is. Only writes if something changed.
pub(super) fn rewrite_lines(path: &Path, change: impl Fn(&str) -> Option<String>) {
    let Ok(content) = fs::read_to_string(path) else { return };
    let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let mut changed = false;
    let lines: Vec<String> = content
        .lines()
        .map(|line| match change(line) {
            Some(new) if new != line => {
                changed = true;
                new
            }
            _ => line.to_string(),
        })
        .collect();
    if changed {
        let mut out = lines.join(newline);
        if content.ends_with('\n') {
            out.push_str(newline);
        }
        let _ = fs::write(path, out);
    }
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
            media::journal_move(&old_abs, &new_abs);
            *field = Some(format!("./{}", new_media_rel));
        }
    } else {
        // File might not exist, just update the path reference
        *field = Some(format!("./{}", new_media_rel));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Game;
    use crate::rename_rules::default_rules;

    fn entry_for(path: &str, source: RenameSource) -> RenameEntry {
        let game = Game { path: path.into(), name: Some("Titulo".into()), ..Default::default() };
        propose(&game, source, &CompiledRules::new(&default_rules()), None)
    }

    #[test]
    fn dat_source_proposes_the_official_name_only_for_recognised_roms() {
        let game = Game { path: "./smw.sfc".into(), ..Default::default() };
        let rules = CompiledRules::new(&default_rules());
        let e = propose(&game, RenameSource::Dat, &rules, Some("Super Mario World (USA)"));
        assert_eq!(e.proposed_name, "Super Mario World (USA)");
        assert!(e.can_rename);
        // No reconocida por contenido: se queda como está
        let e = propose(&game, RenameSource::Dat, &rules, None);
        assert_eq!(e.proposed_name, "smw");
        assert!(!e.can_rename);
    }

    #[test]
    fn typed_name_is_checked_against_disk_and_invalid_characters() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Otro.sfc"), b"x").unwrap();
        fs::write(tmp.path().join("Juego.sfc"), b"x").unwrap();
        assert_eq!(target_conflict(tmp.path(), "./Juego.sfc", "Nuevo"), None);
        assert_eq!(target_conflict(tmp.path(), "./Juego.sfc", "Otro").as_deref(), Some("exists"));
        assert_eq!(target_conflict(tmp.path(), "./Juego.sfc", "Mal: nombre").as_deref(), Some("invalid"));
        assert_eq!(target_conflict(tmp.path(), "./Juego.sfc", "").as_deref(), Some("invalid"));
        // Keeping the same name is not a clash with itself
        assert_eq!(target_conflict(tmp.path(), "./Juego.sfc", "Juego"), None);
    }

    #[test]
    fn forbidden_characters_become_hyphens() {
        assert_eq!(sanitize_filename("Contra III: The Alien Wars"), "Contra III - The Alien Wars");
        assert_eq!(sanitize_filename("Donkey Kong Country 2: Diddy's Kong Quest"), "Donkey Kong Country 2 - Diddy's Kong Quest");
        assert_eq!(sanitize_filename("AC/DC"), "AC-DC");
        assert_eq!(sanitize_filename("Who? Me"), "Who - Me");
        assert_eq!(sanitize_filename("Juego Normal"), "Juego Normal");
    }

    #[test]
    fn rules_source_cleans_the_file_name() {
        let e = entry_for("./Super Juego (USA) (Rev 1).sfc", RenameSource::Rules);
        assert_eq!(e.proposed_name, "Super Juego");
        assert!(e.can_rename);
        let e = entry_for("./Super Juego (USA).sfc", RenameSource::Title);
        assert_eq!(e.proposed_name, "Titulo");
    }

    #[test]
    fn same_target_twice_is_a_duplicate_and_existing_files_are_flagged() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Otro.sfc"), b"x").unwrap();
        let mut entries = vec![
            entry_for("./Juego (USA).sfc", RenameSource::Rules),
            entry_for("./Juego (Europe).sfc", RenameSource::Rules),
            entry_for("./Otro (Japan).sfc", RenameSource::Rules),
            entry_for("./Bien (USA).sfc", RenameSource::Rules),
        ];
        mark_conflicts(&mut entries, tmp.path());
        assert_eq!(entries[0].conflict.as_deref(), Some("duplicate"));
        assert_eq!(entries[1].conflict.as_deref(), Some("duplicate"));
        assert_eq!(entries[2].conflict.as_deref(), Some("exists"));
        assert_eq!(entries[3].conflict, None);
    }

    #[test]
    fn removed_roms_leave_collections_and_m3u() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("psx"));
        fs::create_dir_all(&system.rom_dir).unwrap();
        fs::write(system.rom_dir.join("custom-Favoritos.cfg"), "./Uno.cue\r\n./Dos.cue\r\n").unwrap();
        fs::write(system.rom_dir.join("Juego.m3u"), "Juego (Disc 1).cue\nJuego (Disc 2).cue\n").unwrap();

        remove_references(&Layout::Classic, &system, &["./Uno.cue".into(), "./Juego (Disc 2).cue".into()]);
        assert_eq!(fs::read_to_string(system.rom_dir.join("custom-Favoritos.cfg")).unwrap(), "./Dos.cue\r\n");
        assert_eq!(fs::read_to_string(system.rom_dir.join("Juego.m3u")).unwrap(), "Juego (Disc 1).cue\n");
    }

    #[test]
    fn collections_and_m3u_follow_the_renamed_roms() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("psx"));
        fs::create_dir_all(&system.rom_dir).unwrap();
        fs::write(system.rom_dir.join("custom-Favs.cfg"), "./Juego (USA) (Disc 1).cue\n./Otro.cue\n").unwrap();
        fs::write(system.rom_dir.join("Juego.m3u"), "Juego (USA) (Disc 1).cue\r\nJuego (USA) (Disc 2).cue\r\n").unwrap();

        let renamed = vec![("./Juego (USA) (Disc 1).cue".to_string(), "./Juego (Disc 1).cue".to_string())];
        update_references(&Layout::Classic, &system, &renamed);

        assert_eq!(
            fs::read_to_string(system.rom_dir.join("custom-Favs.cfg")).unwrap(),
            "./Juego (Disc 1).cue\n./Otro.cue\n"
        );
        assert_eq!(
            fs::read_to_string(system.rom_dir.join("Juego.m3u")).unwrap(),
            "Juego (Disc 1).cue\r\nJuego (USA) (Disc 2).cue\r\n"
        );
    }

    #[test]
    fn a_cue_takes_its_tracks_along_and_generic_tracks_stay() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        fs::write(d.join("Juego (USA).cue"), "FILE \"Juego (USA) (Track 1).bin\" BINARY\r\n  TRACK 01 MODE1/2352\r\nFILE \"Juego (USA) (Track 2).bin\" BINARY\r\n").unwrap();
        fs::write(d.join("Juego (USA) (Track 1).bin"), "1").unwrap();
        fs::write(d.join("Juego (USA) (Track 2).bin"), "2").unwrap();
        let tracks = track_renames(&d.join("Juego (USA).cue"), "Juego (USA)", "Juego");
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].new_ref, "Juego (Track 1).bin");

        // Lo que hace rename_roms: ROM, pistas y hoja
        fs::rename(d.join("Juego (USA).cue"), d.join("Juego.cue")).unwrap();
        for t in &tracks {
            fs::rename(&t.from, &t.to).unwrap();
        }
        rewrite_sheet(&d.join("Juego.cue"), &tracks);
        assert_eq!(
            fs::read_to_string(d.join("Juego.cue")).unwrap(),
            "FILE \"Juego (Track 1).bin\" BINARY\r\n  TRACK 01 MODE1/2352\r\nFILE \"Juego (Track 2).bin\" BINARY\r\n",
            "only the names change, CRLF kept"
        );
        assert!(d.join("Juego (Track 2).bin").is_file());

        // Una pista que se llama igual que el juego
        fs::write(d.join("Uno (Europe).cue"), "FILE \"Uno (Europe).bin\" BINARY\n").unwrap();
        fs::write(d.join("Uno (Europe).bin"), "x").unwrap();
        assert_eq!(track_renames(&d.join("Uno (Europe).cue"), "Uno (Europe)", "Uno")[0].new_ref, "Uno.bin");

        // GDI con pistas genéricas: no se tocan
        fs::write(d.join("Crazy (Japan).gdi"), "2\n1 0 4 2352 track01.bin 0\n2 600 0 2352 track02.raw 0\n").unwrap();
        fs::write(d.join("track01.bin"), "").unwrap();
        fs::write(d.join("track02.raw"), "").unwrap();
        assert!(track_renames(&d.join("Crazy (Japan).gdi"), "Crazy (Japan)", "Crazy").is_empty());
    }

    #[test]
    fn a_track_clash_is_a_conflict() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        fs::write(d.join("Juego (USA).cue"), "FILE \"Juego (USA).bin\" BINARY\n").unwrap();
        fs::write(d.join("Juego (USA).bin"), "x").unwrap();
        fs::write(d.join("Juego.bin"), "otro").unwrap();
        assert_eq!(target_conflict(d, "./Juego (USA).cue", "Juego").as_deref(), Some("exists"));
        assert_eq!(target_conflict(d, "./Juego (USA).cue", "Juego Bueno"), None);
    }
}
