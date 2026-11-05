//! «Mover a…»: lleva juegos a otro sistema o a una subcarpeta (del mismo u otro sistema) con
//! todo lo suyo: la ROM, los ficheros que usa (.bin de un .cue, pistas de un .gdi, discos de
//! un .m3u), su media, su entrada de la gamelist y sus colecciones. Nunca sobrescribe: si algo
//! ya existe en el destino, ese juego se salta entero.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::layout::{strip_dot_slash, Layout, SystemRef};
use crate::media::{self, MediaIndex};
use crate::models::{Game, GameList};

#[derive(Debug, Clone, Serialize)]
pub struct MoveResult {
    pub moved: usize,
    /// Juegos no movidos, con el motivo
    pub skipped: Vec<String>,
}

/// Subcarpetas de ROMs de un sistema (sin las de media, ocultas ni las propias de ES-DE),
/// en ruta relativa con «/»
#[tauri::command]
pub fn list_subfolders(system_path: String) -> Result<Vec<String>, String> {
    let system = SystemRef::from_rom_dir(&system_path);
    let skipped = super::new_roms::skipped_dirs();
    let mut out = Vec::new();
    collect_dirs(&system.rom_dir, &system.rom_dir, &skipped, &mut out);
    out.sort_by_key(|d| d.to_lowercase());
    Ok(out)
}

fn collect_dirs(root: &Path, dir: &Path, skipped: &HashSet<String>, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !path.is_dir() || name.starts_with('.') || skipped.contains(&name.to_lowercase()) {
            continue;
        }
        // Una carpeta que es un juego (ES-DE: «Juego.ps3», «Juego.m3u»…) no es una subcarpeta
        if Path::new(&name).extension().is_some() {
            continue;
        }
        if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
        collect_dirs(root, &path, skipped, out);
    }
}

/// Ficheros que usa un juego (rutas absolutas): los de un .cue, .gdi o .m3u y, de los .cue que
/// lista un .m3u, también sus .bin. No incluye la propia ROM.
pub fn companion_files(rom: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    add_companions(rom, &mut out, 0);
    out.retain(|p| p != rom);
    out
}

fn add_companions(file: &Path, out: &mut Vec<PathBuf>, depth: u8) {
    let Some(dir) = file.parent() else { return };
    let ext = media::extension_lower(file).unwrap_or_default();
    let Ok(content) = fs::read_to_string(file) else { return };
    let referenced: Vec<String> = match ext.as_str() {
        // FILE "Juego (Track 1).bin" BINARY
        "cue" => content
            .lines()
            .filter_map(|l| {
                let t = l.trim();
                let rest = t.strip_prefix("FILE ").or_else(|| t.strip_prefix("file "))?;
                Some(match rest.strip_prefix('"') {
                    Some(quoted) => quoted.split('"').next()?.to_string(),
                    None => rest.rsplit_once(' ').map_or(rest, |(name, _)| name).to_string(),
                })
            })
            .collect(),
        // 1 0 4 2352 "track01.bin" 0   (la primera línea es el número de pistas)
        "gdi" => content
            .lines()
            .skip(1)
            .filter_map(|l| {
                let t = l.trim();
                if let Some(start) = t.find('"') {
                    return t[start + 1..].split('"').next().map(String::from);
                }
                t.split_whitespace().nth(4).map(String::from)
            })
            .collect(),
        "m3u" => content.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(String::from).collect(),
        _ => Vec::new(),
    };
    for name in referenced {
        let path = dir.join(name.replace('\\', "/"));
        if path.is_file() && !out.contains(&path) {
            out.push(path.clone());
            // Un .m3u que lista .cue: también los .bin de cada .cue
            if depth == 0 {
                add_companions(&path, out, depth + 1);
            }
        }
    }
}

/// Lo que hay que mover de un juego: (origen, destino) de cada fichero y la ruta nueva
struct MovePlan {
    files: Vec<(PathBuf, PathBuf)>,
    /// (campo, origen, destino) de la media
    media: Vec<(&'static str, PathBuf, PathBuf)>,
    new_path: String,
}

/// Ruta nueva de la ROM: «./<subcarpeta>/<nombre del fichero>»
fn new_rom_path(rom_path: &str, subfolder: &str) -> String {
    let file_name = Path::new(strip_dot_slash(rom_path)).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let sub = subfolder.trim().trim_matches('/');
    if sub.is_empty() {
        format!("./{}", file_name)
    } else {
        format!("./{}/{}", sub, file_name)
    }
}

#[allow(clippy::too_many_arguments)]
fn plan(
    layout: &Layout,
    from: &SystemRef,
    to: &SystemRef,
    index: &MediaIndex,
    game: &Game,
    subfolder: &str,
) -> Result<MovePlan, String> {
    let rom = from.rom_dir.join(strip_dot_slash(&game.path));
    if !rom.exists() {
        return Err("ROM not found".into());
    }
    let new_path = new_rom_path(&game.path, subfolder);
    let new_rom = to.rom_dir.join(strip_dot_slash(&new_path));
    let rom_dir = rom.parent().unwrap_or(&from.rom_dir).to_path_buf();
    let new_dir = new_rom.parent().unwrap_or(&to.rom_dir).to_path_buf();

    let mut files = vec![(rom.clone(), new_rom)];
    if rom.is_file() {
        for companion in companion_files(&rom) {
            // Fuera de la carpeta de la ROM (rutas raras con «..»): se deja donde está
            let Ok(rel) = companion.strip_prefix(&rom_dir) else { continue };
            files.push((companion.clone(), new_dir.join(rel)));
        }
    }

    let is_dir = rom.is_dir();
    let mut media_moves = Vec::new();
    for kind in media::MEDIA_KINDS {
        let Some(src) = super::media_kinds::current_file(layout, from, index, game, kind) else { continue };
        let ext = media::extension_lower(&src).unwrap_or_else(|| "png".into());
        let stem = crate::layout::media_stem_for(&new_path, is_dir);
        let dest = layout.media_file_for_stem(to, kind.folder, &stem, &ext);
        media_moves.push((kind.field, src, dest));
    }

    // Nada puede pisar algo que ya exista
    for (src, dest) in files.iter().map(|(a, b)| (a, b)).chain(media_moves.iter().map(|(_, a, b)| (a, b))) {
        if dest.exists() && src != dest {
            let shown = dest.strip_prefix(&to.rom_dir).unwrap_or(dest);
            return Err(format!("already exists in the destination: {}", shown.display()));
        }
    }
    Ok(MovePlan { files, media: media_moves, new_path })
}

/// El trabajo de verdad (sin la app, para poder probarlo): mueve los juegos `game_paths` de
/// `from` a `to`/`subfolder` y devuelve las dos gamelists ya cambiadas (sin escribir).
fn move_between(
    layout: &Layout,
    from: &SystemRef,
    to: &SystemRef,
    src_list: &mut GameList,
    dst_list: &mut GameList,
    game_paths: &[String],
    subfolder: &str,
) -> (Vec<(String, String)>, Vec<String>) {
    let index = MediaIndex::build(layout, from);
    let same_system = from.rom_dir == to.rom_dir;
    let mut moved: Vec<(String, String)> = Vec::new();
    let mut skipped = Vec::new();

    for path in game_paths {
        let Some(pos) = src_list.games.iter().position(|g| &g.path == path) else { continue };
        let game = src_list.games[pos].clone();
        let name = game.name.clone().unwrap_or_else(|| game.path.clone());
        let plan = match plan(layout, from, to, &index, &game, subfolder) {
            Ok(p) => p,
            Err(e) => {
                skipped.push(format!("{} ({})", name, e));
                continue;
            }
        };
        if same_system && plan.new_path == game.path {
            continue; // ya está ahí
        }

        // Ficheros de la ROM; si uno falla, se deshace lo movido de ese juego
        let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut failed = None;
        for (src, dest) in &plan.files {
            match media::move_file(src, dest) {
                Ok(()) => done.push((src.clone(), dest.clone())),
                Err(e) => {
                    failed = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = failed {
            for (src, dest) in done.into_iter().rev() {
                let _ = media::move_file(&dest, &src);
            }
            skipped.push(format!("{} ({})", name, e));
            continue;
        }

        // Media: en clásica, un fichero que usa también otro juego se copia (no se le quita)
        let mut updated = game.clone();
        for (field, src, dest) in &plan.media {
            let shared = layout.writes_media_tags()
                && src_list.games.iter().enumerate().any(|(i, g)| {
                    i != pos && media::media_fields().any(|f| media::get_field(g, f).is_some_and(|v| media::resolve_media_path(from, v) == *src))
                });
            let ok = if shared {
                dest.parent().map(fs::create_dir_all).is_some() && fs::copy(src, dest).is_ok()
            } else {
                media::move_file(src, dest).is_ok()
            };
            if ok && layout.writes_media_tags() {
                media::set_field(&mut updated, field, Some(media::media_ref(layout, to, dest)));
            }
        }
        updated.path = plan.new_path.clone();

        src_list.games.remove(pos);
        if same_system {
            src_list.games.push(updated);
        } else {
            dst_list.games.push(updated);
        }
        moved.push((game.path.clone(), plan.new_path));
    }
    (moved, skipped)
}

/// Colecciones: el juego pasa a su ruta nueva.
/// ES-DE (colecciones comunes): se reescribe la línea. Clásica (cada sistema tiene las suyas):
/// sale del custom-X.cfg del origen y entra en el custom-X.cfg del destino.
fn update_collections(layout: &Layout, from: &SystemRef, to: &SystemRef, moved: &[(String, String)]) {
    if moved.is_empty() {
        return;
    }
    let cfgs = |dir: &Path| -> Vec<PathBuf> {
        fs::read_dir(dir)
            .map(|e| {
                e.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.file_name().map(|n| n.to_string_lossy().to_string()).is_some_and(|n| n.starts_with("custom-") && n.ends_with(".cfg")))
                    .collect()
            })
            .unwrap_or_default()
    };
    let src_dir = layout.collections_dir(from);
    let dst_dir = layout.collections_dir(to);
    if src_dir == dst_dir {
        let renames: std::collections::HashMap<String, String> =
            moved.iter().map(|(old, new)| (layout.collection_entry(from, old), layout.collection_entry(to, new))).collect();
        for cfg in cfgs(&src_dir) {
            super::rename::rewrite_lines(&cfg, |line| renames.get(line.trim()).cloned());
        }
        return;
    }
    for cfg in cfgs(&src_dir) {
        let Ok(content) = fs::read_to_string(&cfg) else { continue };
        let lines: HashSet<&str> = content.lines().map(str::trim).collect();
        let leaving: Vec<&(String, String)> = moved.iter().filter(|(old, _)| lines.contains(layout.collection_entry(from, old).as_str())).collect();
        if leaving.is_empty() {
            continue;
        }
        let gone: HashSet<String> = leaving.iter().map(|(old, _)| layout.collection_entry(from, old)).collect();
        super::rename::remove_lines(&cfg, |line| gone.contains(line.trim()));
        // Misma colección en el sistema de destino (se crea si no existe)
        let target = dst_dir.join(cfg.file_name().unwrap());
        let existing = fs::read_to_string(&target).unwrap_or_default();
        let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
        let mut out = existing.clone();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(newline);
        }
        for (_, new) in leaving {
            let entry = layout.collection_entry(to, new);
            if !existing.lines().any(|l| l.trim() == entry) {
                out.push_str(&entry);
                out.push_str(newline);
            }
        }
        let _ = fs::write(&target, out);
    }
}

/// Mueve juegos a otro sistema o subcarpeta. Escribe las dos gamelists (con copia de seguridad).
#[tauri::command]
pub fn move_games(
    app: tauri::AppHandle,
    system_path: String,
    game_paths: Vec<String>,
    dest_system_path: String,
    dest_subfolder: Option<String>,
) -> Result<MoveResult, String> {
    let (layout, from) = super::system_context(&app, &system_path)?;
    let to = SystemRef::from_rom_dir(&dest_system_path);
    if !to.rom_dir.is_dir() {
        return Err(format!("Destination system not found: {}", dest_system_path));
    }
    let sub = dest_subfolder.unwrap_or_default();
    if sub.split(['/', '\\']).any(|part| part == "..") {
        return Err("Invalid subfolder".into());
    }
    let same = from.rom_dir == to.rom_dir;
    let mut src_list = super::gamelist::read_gamelist(&layout, &from)?;
    let mut dst_list = if same { GameList::default() } else { super::gamelist::read_gamelist(&layout, &to)? };

    let (moved, skipped) = move_between(&layout, &from, &to, &mut src_list, &mut dst_list, &game_paths, &sub);
    if !moved.is_empty() {
        super::gamelist::write_gamelist(&layout, &from, &src_list)?;
        if !same {
            super::gamelist::write_gamelist(&layout, &to, &dst_list)?;
        }
        update_collections(&layout, &from, &to, &moved);
    }
    Ok(MoveResult { moved: moved.len(), skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::EsdePaths;

    fn touch(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn list(paths: &[&str]) -> GameList {
        GameList {
            games: paths.iter().map(|p| Game { path: p.to_string(), name: Some(format!("Juego {}", p)), ..Default::default() }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn classic_moves_rom_media_and_entry_to_another_system() {
        let tmp = tempfile::tempdir().unwrap();
        let from = SystemRef::from_rom_dir(tmp.path().join("gbc"));
        let to = SystemRef::from_rom_dir(tmp.path().join("gb"));
        fs::create_dir_all(&to.rom_dir).unwrap();
        touch(&from.rom_dir.join("Tetris.gb"), "rom");
        touch(&from.rom_dir.join("covers/Tetris.png"), "img");
        let mut src = list(&["./Tetris.gb", "./Otro.gbc"]);
        src.games[0].image = Some("./covers/Tetris.png".into());
        let mut dst = GameList::default();

        let (moved, skipped) = move_between(&Layout::Classic, &from, &to, &mut src, &mut dst, &["./Tetris.gb".into()], "");
        assert_eq!(moved.len(), 1, "{:?}", skipped);
        assert!(to.rom_dir.join("Tetris.gb").is_file());
        assert!(to.rom_dir.join("covers/Tetris.png").is_file());
        assert!(!from.rom_dir.join("Tetris.gb").exists());
        assert_eq!(src.games.len(), 1);
        assert_eq!(dst.games[0].path, "./Tetris.gb");
        assert_eq!(dst.games[0].image.as_deref(), Some("./covers/Tetris.png"));
        assert_eq!(dst.games[0].name.as_deref(), Some("Juego ./Tetris.gb"), "data travels with it");
    }

    #[test]
    fn esde_moves_into_a_new_subfolder_with_its_media() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Esde(EsdePaths { home: tmp.path().join("ES-DE"), media_root: tmp.path().join("media") });
        let sys = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        touch(&sys.rom_dir.join("Zelda.sfc"), "rom");
        touch(&tmp.path().join("media/snes/covers/Zelda.png"), "img");
        let mut src = list(&["./Zelda.sfc"]);
        let mut dst = GameList::default();

        let (moved, _) = move_between(&layout, &sys, &sys, &mut src, &mut dst, &["./Zelda.sfc".into()], "RPG/");
        assert_eq!(moved, vec![("./Zelda.sfc".to_string(), "./RPG/Zelda.sfc".to_string())]);
        assert!(sys.rom_dir.join("RPG/Zelda.sfc").is_file());
        // ES-DE busca la media repitiendo la subcarpeta
        assert!(tmp.path().join("media/snes/covers/RPG/Zelda.png").is_file());
        assert_eq!(src.games[0].path, "./RPG/Zelda.sfc", "same system: stays in its gamelist");
        assert_eq!(list_subfolders(sys.rom_dir.to_string_lossy().to_string()).unwrap(), vec!["RPG".to_string()]);
    }

    #[test]
    fn a_cue_travels_with_its_bins_and_a_clash_leaves_the_game_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let from = SystemRef::from_rom_dir(tmp.path().join("psx"));
        let to = SystemRef::from_rom_dir(tmp.path().join("saturn"));
        touch(&from.rom_dir.join("Juego.cue"), "FILE \"Juego (Track 1).bin\" BINARY\nFILE \"Juego (Track 2).bin\" BINARY\n");
        touch(&from.rom_dir.join("Juego (Track 1).bin"), "1");
        touch(&from.rom_dir.join("Juego (Track 2).bin"), "2");
        touch(&from.rom_dir.join("Otro.cue"), "FILE \"Otro.bin\" BINARY\n");
        touch(&from.rom_dir.join("Otro.bin"), "x");
        touch(&to.rom_dir.join("Otro.bin"), "ya estaba");
        let mut src = list(&["./Juego.cue", "./Otro.cue"]);
        let mut dst = GameList::default();

        let (moved, skipped) = move_between(&Layout::Classic, &from, &to, &mut src, &mut dst, &["./Juego.cue".into(), "./Otro.cue".into()], "");
        assert_eq!(moved.len(), 1);
        assert!(to.rom_dir.join("Juego (Track 2).bin").is_file());
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].contains("Otro.bin"));
        // El que choca no se ha tocado
        assert!(from.rom_dir.join("Otro.cue").is_file() && from.rom_dir.join("Otro.bin").is_file());
        assert_eq!(fs::read_to_string(to.rom_dir.join("Otro.bin")).unwrap(), "ya estaba");
    }

    #[test]
    fn collections_follow_the_game() {
        let tmp = tempfile::tempdir().unwrap();
        // Clásica: sale del cfg del origen y entra en el del destino
        let from = SystemRef::from_rom_dir(tmp.path().join("gbc"));
        let to = SystemRef::from_rom_dir(tmp.path().join("gb"));
        touch(&from.rom_dir.join("custom-Favoritos.cfg"), "./Tetris.gb\n./Otro.gbc\n");
        fs::create_dir_all(&to.rom_dir).unwrap();
        update_collections(&Layout::Classic, &from, &to, &[("./Tetris.gb".into(), "./Puzzle/Tetris.gb".into())]);
        assert_eq!(fs::read_to_string(from.rom_dir.join("custom-Favoritos.cfg")).unwrap(), "./Otro.gbc\n");
        assert_eq!(fs::read_to_string(to.rom_dir.join("custom-Favoritos.cfg")).unwrap(), "./Puzzle/Tetris.gb\n");

        // ES-DE: colecciones comunes, se reescribe la línea
        let layout = Layout::Esde(EsdePaths { home: tmp.path().join("ES-DE"), media_root: tmp.path().join("media") });
        touch(&tmp.path().join("ES-DE/collections/custom-Favoritos.cfg"), "%ROMPATH%/gbc/Tetris.gb\n");
        update_collections(&layout, &from, &to, &[("./Tetris.gb".into(), "./Tetris.gb".into())]);
        assert_eq!(fs::read_to_string(tmp.path().join("ES-DE/collections/custom-Favoritos.cfg")).unwrap(), "%ROMPATH%/gb/Tetris.gb\n");
    }

    #[test]
    fn reads_the_files_of_cue_gdi_and_m3u() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        touch(&d.join("g.gdi"), "2\n1 0 4 2352 track01.bin 0\n2 600 0 2352 \"track 02.raw\" 0\n");
        touch(&d.join("track01.bin"), "");
        touch(&d.join("track 02.raw"), "");
        assert_eq!(companion_files(&d.join("g.gdi")).len(), 2);
        touch(&d.join("Discos/D1.cue"), "FILE \"D1.bin\" BINARY\n");
        touch(&d.join("Discos/D1.bin"), "");
        touch(&d.join("Juego.m3u"), "# lista\nDiscos/D1.cue\n");
        let files = companion_files(&d.join("Juego.m3u"));
        assert!(files.contains(&d.join("Discos/D1.cue")) && files.contains(&d.join("Discos/D1.bin")));
    }
}
