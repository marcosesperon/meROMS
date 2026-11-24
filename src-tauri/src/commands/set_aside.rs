//! «Apartar»: sacar de la colección sin borrar nada. Las versiones que sobran (y los medios
//! sobrantes o dañados, si el usuario lo prefiere a la papelera) se mueven a la carpeta de
//! apartados del perfil (Ajustes → Mi colección), dentro de `<sistema>/versions/` y
//! `<sistema>/media/`. Una subcarpeta del propio sistema no vale: EmulationStation y ES-DE la
//! enseñarían como una carpeta de juegos.
//!
//! Todo se mueve con `media::move_file` (apuntado en el diario), así que «Deshacer» lo devuelve
//! a su sitio y vuelve a meter las entradas en la gamelist. Nunca se pisa nada: si un destino
//! ya existe, ese juego (o ese fichero) se salta entero.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::layout::{Layout, SystemRef};
use crate::media;
use crate::models::Game;

use super::progress::Progress;

/// Subcarpeta de los juegos apartados dentro de `<carpeta de apartados>/<sistema>/`
const VERSIONS_DIR: &str = "versions";
/// Subcarpeta de los medios apartados
const MEDIA_DIR: &str = "media";

#[derive(Debug, Clone, Default, Serialize)]
pub struct SetAsideResult {
    /// Juegos (o ficheros de medios) apartados
    pub moved: usize,
    /// Lo que no se pudo apartar, con el motivo
    pub skipped: Vec<String>,
    /// Carpeta donde ha ido todo
    pub folder: String,
}

/// Carpeta de apartados del perfil activo para este sistema (`<carpeta>/<sistema>`).
/// Sin carpeta elegida, error `no_set_aside_dir` (la ventana ofrece elegirla).
fn set_aside_root(app: &tauri::AppHandle, system: &SystemRef) -> Result<PathBuf, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let dir = settings
        .active_profile()
        .and_then(|p| p.set_aside_dir.clone())
        .filter(|d| !d.trim().is_empty())
        .ok_or("no_set_aside_dir")?;
    Ok(PathBuf::from(dir).join(&system.name))
}

/// Dónde va un fichero de `base` dentro de `dest` (misma ruta relativa; si no está dentro de
/// `base`, solo con su nombre)
fn destination(file: &Path, base: &Path, dest: &Path) -> PathBuf {
    match file.strip_prefix(base) {
        Ok(rel) => dest.join(rel),
        Err(_) => dest.join(file.file_name().unwrap_or_default()),
    }
}

/// Los ficheros de un juego y adónde van: ROM (con sus pistas, si es un .cue/.gdi/.m3u) y medios
fn game_moves(layout: &Layout, system: &SystemRef, root: &Path, rom: Option<&PathBuf>, media_files: &[PathBuf]) -> Vec<(PathBuf, PathBuf)> {
    let versions = root.join(VERSIONS_DIR);
    let mut moves = Vec::new();
    if let Some(rom) = rom {
        moves.push((rom.clone(), destination(rom, &system.rom_dir, &versions)));
        for companion in super::move_games::companion_files(rom).into_iter().filter(|c| c.exists()) {
            moves.push((companion.clone(), destination(&companion, &system.rom_dir, &versions)));
        }
    }
    let media_base = layout.media_type_dir(system, "");
    for file in media_files {
        moves.push((file.clone(), destination(file, &media_base, &versions.join(MEDIA_DIR))));
    }
    moves
}

/// Mueve los ficheros de un juego. Si alguno falla, devuelve a su sitio los que ya se movieron
/// (el juego se queda como estaba) y dice cuál falló.
fn move_all(moves: &[(PathBuf, PathBuf)]) -> Result<(), String> {
    if let Some((_, to)) = moves.iter().find(|(_, to)| to.exists()) {
        return Err(format!("{} (already exists)", to.display()));
    }
    let mut done: Vec<&(PathBuf, PathBuf)> = Vec::new();
    for pair in moves {
        if let Err(e) = media::move_file(&pair.0, &pair.1) {
            for (from, to) in done.into_iter().rev() {
                let _ = media::move_file(to, from);
            }
            return Err(format!("{} ({})", pair.0.display(), e));
        }
        done.push(pair);
    }
    Ok(())
}

/// Aparta juegos: su ROM y sus medios van a `<apartados>/<sistema>/versions/` (misma ruta
/// relativa; los medios bajo `media/`) y su entrada sale de la gamelist (y de colecciones y
/// .m3u). En clásica, un medio que también usa un juego que se queda no se mueve.
#[tauri::command(async)]
pub fn set_aside_games(app: tauri::AppHandle, system_path: String, game_paths: Vec<String>, task: Option<String>) -> Result<SetAsideResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let root = set_aside_root(&app, &system)?;
    let progress = Progress::new(&app, task);
    let (result, op) = set_aside_games_in(&layout, &system, &root, &game_paths, &progress)?;
    super::undo::record(op);
    Ok(result)
}

/// Lo hace y devuelve, aparte, la operación para Deshacer (el comando la apunta)
fn set_aside_games_in(layout: &Layout, system: &SystemRef, root: &Path, game_paths: &[String], progress: &Progress) -> Result<(SetAsideResult, super::undo::UndoOp), String> {
    let mut gamelist = super::gamelist::read_gamelist(layout, system)?;
    let plan = super::duplicates::files_to_remove(layout, system, &gamelist.games, game_paths);
    let mut result = SetAsideResult { folder: root.join(VERSIONS_DIR).to_string_lossy().to_string(), ..Default::default() };

    media::journal_begin();
    let mut moved: HashSet<String> = HashSet::new();
    for (i, (game_path, rom, media_files)) in plan.iter().enumerate() {
        if progress.cancelled() {
            break;
        }
        progress.step("moving", i + 1, plan.len(), game_path);
        match move_all(&game_moves(layout, system, root, rom.as_ref(), media_files)) {
            Ok(()) => {
                moved.insert(game_path.clone());
            }
            Err(e) => result.skipped.push(e),
        }
    }
    let files = media::journal_take();

    if moved.is_empty() {
        return Ok((result, Default::default()));
    }
    progress.phase("saving");
    let sys = system.rom_dir.to_string_lossy().to_string();
    let removed: Vec<(String, Game)> = gamelist.games.iter().filter(|g| moved.contains(&g.path)).map(|g| (sys.clone(), g.clone())).collect();
    gamelist.games.retain(|g| !moved.contains(&g.path));
    super::gamelist::write_gamelist(layout, system, &gamelist)?;
    let paths: Vec<String> = moved.iter().cloned().collect();
    super::rename::remove_references(layout, system, &paths);
    result.moved = moved.len();
    Ok((result, super::undo::UndoOp { kind: "set_aside".into(), system: sys, files, removed, ..Default::default() }))
}

/// Aparta ficheros de medios (sobrantes o dañados) a `<apartados>/<sistema>/media/`, con su
/// ruta dentro de la carpeta de medios. Solo ficheros del sistema; en clásica se vacían los
/// campos de la gamelist que apuntaban a ellos (Deshacer los vuelve a poner).
#[tauri::command(async)]
pub fn set_aside_media(app: tauri::AppHandle, system_path: String, paths: Vec<String>, task: Option<String>) -> Result<SetAsideResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let root = set_aside_root(&app, &system)?;
    let progress = Progress::new(&app, task);
    let (result, op) = set_aside_media_in(&layout, &system, &root, &paths, &progress)?;
    super::undo::record(op);
    Ok(result)
}

fn set_aside_media_in(layout: &Layout, system: &SystemRef, root: &Path, paths: &[String], progress: &Progress) -> Result<(SetAsideResult, super::undo::UndoOp), String> {
    let (allowed, skipped) = super::cleanup::allowed_media_paths(layout, system, paths);
    let dest = root.join(MEDIA_DIR);
    let mut result = SetAsideResult { folder: dest.to_string_lossy().to_string(), skipped, ..Default::default() };
    // Las rutas ya vienen resueltas: las bases también, para que `strip_prefix` encaje
    let canon = |p: PathBuf| p.canonicalize().unwrap_or(p);
    let media_base = canon(layout.media_type_dir(system, ""));
    let rom_dir = canon(system.rom_dir.clone());

    media::journal_begin();
    let mut moved: HashSet<PathBuf> = HashSet::new();
    for (i, file) in allowed.iter().enumerate() {
        if progress.cancelled() {
            break;
        }
        progress.step("moving", i + 1, allowed.len(), &file.to_string_lossy());
        // En ES-DE, los restos de la estructura clásica están en la carpeta de ROMs
        let base = if file.starts_with(&media_base) { &media_base } else { &rom_dir };
        let to = destination(file, base, &dest);
        match media::move_file(file, &to) {
            Ok(()) => {
                moved.insert(file.clone());
            }
            Err(e) => result.skipped.push(format!("{} ({})", file.display(), e)),
        }
    }
    let files = media::journal_take();
    result.moved = moved.len();
    if moved.is_empty() {
        return Ok((result, Default::default()));
    }

    let sys = system.rom_dir.to_string_lossy().to_string();
    let mut games = Vec::new();
    if layout.writes_media_tags() {
        progress.phase("saving");
        let before = super::gamelist::read_gamelist(layout, system)?;
        let mut after = before.clone();
        if super::cleanup::clear_fields_pointing_to(system, &mut after, &moved) {
            super::gamelist::write_gamelist(layout, system, &after)?;
            games = super::undo::changes_same_paths(&sys, &before, &after);
        }
    }
    Ok((result, super::undo::UndoOp { kind: "set_aside".into(), system: sys, files, games, ..Default::default() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GameList;
    use std::fs;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn a_set_aside_game_leaves_the_gamelist_with_its_files_and_undo_brings_it_back() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Classic;
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/nes"));
        let root = tmp.path().join("apartados/nes");
        touch(&system.rom_dir.join("Zelda (Europe).nes"));
        touch(&system.rom_dir.join("covers/Zelda (Europe).png"));
        touch(&system.rom_dir.join("Zelda (USA).nes"));
        let list = GameList {
            games: vec![
                Game { path: "./Zelda (Europe).nes".into(), image: Some("./covers/Zelda (Europe).png".into()), ..Default::default() },
                Game { path: "./Zelda (USA).nes".into(), ..Default::default() },
            ],
            ..Default::default()
        };
        super::super::gamelist::write_gamelist_no_backup(&layout, &system, &list).unwrap();

        let (r, op) = set_aside_games_in(&layout, &system, &root, &["./Zelda (Europe).nes".into()], &Progress::none()).unwrap();
        assert_eq!(r.moved, 1, "{:?}", r.skipped);
        assert!(root.join("versions/Zelda (Europe).nes").is_file());
        assert!(root.join("versions/media/covers/Zelda (Europe).png").is_file());
        assert!(!system.rom_dir.join("Zelda (Europe).nes").exists());
        let after = super::super::gamelist::read_gamelist(&layout, &system).unwrap();
        assert_eq!(after.games.len(), 1);

        let u = super::super::undo::undo_op_for_tests(&layout, &op).unwrap();
        assert_eq!(u.restored, 2, "{:?}", u.skipped);
        assert!(system.rom_dir.join("Zelda (Europe).nes").is_file());
        assert!(system.rom_dir.join("covers/Zelda (Europe).png").is_file());
        let back = super::super::gamelist::read_gamelist(&layout, &system).unwrap();
        assert_eq!(back.games.len(), 2, "the entry is back");
    }

    #[test]
    fn a_game_whose_destination_is_taken_is_skipped_whole() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Classic;
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/nes"));
        let root = tmp.path().join("apartados/nes");
        touch(&system.rom_dir.join("Juego.nes"));
        touch(&root.join("versions/Juego.nes"));
        let list = GameList { games: vec![Game { path: "./Juego.nes".into(), ..Default::default() }], ..Default::default() };
        super::super::gamelist::write_gamelist_no_backup(&layout, &system, &list).unwrap();
        let (r, _) = set_aside_games_in(&layout, &system, &root, &["./Juego.nes".into()], &Progress::none()).unwrap();
        assert_eq!(r.moved, 0);
        assert_eq!(r.skipped.len(), 1);
        assert!(system.rom_dir.join("Juego.nes").is_file(), "nothing moved");
        assert_eq!(super::super::gamelist::read_gamelist(&layout, &system).unwrap().games.len(), 1);
    }

    #[test]
    fn set_aside_media_clears_the_classic_field() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Classic;
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/nes"));
        let root = tmp.path().join("apartados/nes");
        touch(&system.rom_dir.join("Juego.nes"));
        let cover = system.rom_dir.join("covers/Juego.png");
        touch(&cover);
        let list = GameList { games: vec![Game { path: "./Juego.nes".into(), image: Some("./covers/Juego.png".into()), ..Default::default() }], ..Default::default() };
        super::super::gamelist::write_gamelist_no_backup(&layout, &system, &list).unwrap();
        let (r, _) = set_aside_media_in(&layout, &system, &root, &[cover.to_string_lossy().to_string()], &Progress::none()).unwrap();
        assert_eq!(r.moved, 1, "{:?}", r.skipped);
        assert!(root.join("media/covers/Juego.png").is_file());
        assert!(super::super::gamelist::read_gamelist(&layout, &system).unwrap().games[0].image.is_none());
    }
}
