//! «Llevar a otro perfil…»: un sistema entero pasa del perfil activo a otro perfil guardado
//! (clásico o ES-DE, con su propia carpeta de ROMs): las ROMs con sus ficheros de disco, los
//! medios con el nombre y la carpeta de la estructura del destino, las entradas de la gamelist
//! con todos sus datos y las colecciones que los nombran. Se puede mover o copiar.
//!
//! Nunca sobrescribe: un juego que el destino ya tiene (misma entrada o su ROM ya está) se salta
//! entero. Lo que no es de ningún juego de la gamelist se queda donde está.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::layout::{strip_dot_slash, Layout, SystemRef};
use crate::media::{self, MediaIndex};
use crate::models::{Game, GameList};

use super::migrate::MigrationMode;
use super::progress::Progress;

#[derive(Debug, Clone, Default, Serialize)]
pub struct TransferResult {
    /// Juegos llevados
    pub transferred: usize,
    /// Juegos que no se han llevado, con el motivo
    pub skipped: Vec<String>,
    /// Medios de tipos que la estructura del destino no tiene (se quedan en el origen)
    pub media_unsupported: usize,
    /// Ficheros de la carpeta del sistema que no son de ningún juego de la gamelist (se quedan)
    pub files_left: usize,
    /// Carpeta del sistema en el destino
    pub destination: String,
}

/// Lo que se va a hacer con un juego: ficheros de la ROM y medios (origen → destino)
struct GamePlan {
    files: Vec<(PathBuf, PathBuf)>,
    media: Vec<(&'static str, PathBuf, PathBuf)>,
    unsupported: usize,
}

fn plan_game(
    src_layout: &Layout,
    dst_layout: &Layout,
    from: &SystemRef,
    to: &SystemRef,
    index: &MediaIndex,
    game: &Game,
) -> Result<GamePlan, String> {
    let rel = strip_dot_slash(&game.path);
    let rom = from.rom_dir.join(rel);
    if !rom.exists() {
        return Err("ROM not found".into());
    }
    let new_rom = to.rom_dir.join(rel);
    if new_rom.exists() {
        return Err("the ROM is already in the destination".into());
    }
    let rom_dir = rom.parent().unwrap_or(&from.rom_dir).to_path_buf();
    let new_dir = new_rom.parent().unwrap_or(&to.rom_dir).to_path_buf();

    // La ROM y lo que usa (.bin de un .cue, pistas de un .gdi, discos de un .m3u), en su sitio
    let mut files = vec![(rom.clone(), new_rom)];
    if rom.is_file() {
        for companion in super::move_games::companion_files(&rom) {
            let Ok(rel) = companion.strip_prefix(&rom_dir) else { continue };
            files.push((companion.clone(), new_dir.join(rel)));
        }
    }

    let mut media_moves = Vec::new();
    let mut unsupported = 0;
    for kind in media::MEDIA_KINDS {
        let Some(src) = super::media_kinds::current_file(src_layout, from, index, game, kind) else { continue };
        // ES-DE no tiene todos los tipos de la clásica: esos se quedan
        if !dst_layout.writes_media_tags() && !kind.in_esde {
            unsupported += 1;
            continue;
        }
        let ext = media::extension_lower(&src).unwrap_or_else(|| "png".into());
        media_moves.push((kind.field, src, dst_layout.media_file_path(to, kind.folder, &game.path, &ext)));
    }

    // Nada puede pisar algo que ya exista en el destino
    for dest in files.iter().map(|(_, d)| d).chain(media_moves.iter().map(|(_, _, d)| d)) {
        if dest.exists() {
            let shown = dest.strip_prefix(&to.rom_dir).unwrap_or(dest);
            return Err(format!("already exists in the destination: {}", shown.display()));
        }
    }
    Ok(GamePlan { files, media: media_moves, unsupported })
}

/// Copia una carpeta entera (ROMs que son carpetas). No pisa nada.
fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("Failed to create {}: {}", to.display(), e))?;
    for entry in fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        let dest = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            media::copy_file(&path, &dest)?;
        }
    }
    Ok(())
}

/// Pasa un fichero (o una carpeta-ROM) al destino, moviendo o copiando
fn transfer_file(from: &Path, to: &Path, mode: MigrationMode) -> Result<(), String> {
    if from.is_dir() {
        if mode == MigrationMode::Move && to.parent().map(fs::create_dir_all).transpose().is_ok() && fs::rename(from, to).is_ok() {
            return Ok(());
        }
        copy_dir(from, to)?;
        if mode == MigrationMode::Move {
            fs::remove_dir_all(from).map_err(|e| format!("Failed to remove {}: {}", from.display(), e))?;
        }
        return Ok(());
    }
    match mode {
        MigrationMode::Copy => media::copy_file(from, to),
        MigrationMode::Move => media::move_file(from, to),
    }
}

/// Deshace lo pasado de un juego que ha fallado a medias
fn undo_files(done: Vec<(PathBuf, PathBuf)>, mode: MigrationMode) {
    for (src, dest) in done.into_iter().rev() {
        match mode {
            MigrationMode::Move => {
                let _ = transfer_file(&dest, &src, MigrationMode::Move);
            }
            MigrationMode::Copy if dest.is_dir() => {
                let _ = fs::remove_dir_all(&dest);
            }
            MigrationMode::Copy => {
                let _ = fs::remove_file(&dest);
            }
        }
    }
}

/// El trabajo de verdad (sin la app, para poder probarlo): pasa los juegos de `src_list` al
/// destino y deja las dos gamelists cambiadas (sin escribir). Devuelve los pares (ruta en el
/// origen, ruta en el destino) de los juegos llevados, que son las mismas rutas relativas.
#[allow(clippy::too_many_arguments)]
fn transfer_games(
    src_layout: &Layout,
    dst_layout: &Layout,
    from: &SystemRef,
    to: &SystemRef,
    src_list: &mut GameList,
    dst_list: &mut GameList,
    mode: MigrationMode,
    progress: &Progress,
    result: &mut TransferResult,
) -> Vec<(String, String)> {
    progress.phase("media");
    let index = MediaIndex::build(src_layout, from);
    let in_destination: HashSet<String> = dst_list.games.iter().map(|g| strip_dot_slash(&g.path).to_string()).collect();
    let games: Vec<Game> = src_list.games.clone();
    let mut done_paths: Vec<(String, String)> = Vec::new();

    for (i, game) in games.iter().enumerate() {
        // Cancelado: se para entre un juego y otro; lo llevado se guarda en las dos gamelists
        if progress.cancelled() {
            break;
        }
        progress.step("moving", i + 1, games.len(), &game.path);
        let name = game.name.clone().unwrap_or_else(|| game.path.clone());
        if in_destination.contains(strip_dot_slash(&game.path)) {
            result.skipped.push(format!("{} (already in the destination)", name));
            continue;
        }
        let plan = match plan_game(src_layout, dst_layout, from, to, &index, game) {
            Ok(p) => p,
            Err(e) => {
                result.skipped.push(format!("{} ({})", name, e));
                continue;
            }
        };

        // Ficheros de la ROM; si uno falla, se deshace lo pasado de ese juego
        let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut failed = None;
        for (src, dest) in &plan.files {
            match transfer_file(src, dest, mode) {
                Ok(()) => done.push((src.clone(), dest.clone())),
                Err(e) => {
                    failed = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = failed {
            undo_files(done, mode);
            result.skipped.push(format!("{} ({})", name, e));
            continue;
        }

        // Medios: al mover, en clásica un fichero que usa también otro juego se copia
        let mut entry = game.clone();
        for field in media::media_fields() {
            media::set_field(&mut entry, field, None);
        }
        for (field, src, dest) in &plan.media {
            let shared = mode == MigrationMode::Move
                && src_layout.writes_media_tags()
                && src_list.games.iter().any(|g| {
                    g.path != game.path && media::media_fields().any(|f| media::get_field(g, f).is_some_and(|v| media::resolve_media_path(from, v) == *src))
                });
            let how = if shared { MigrationMode::Copy } else { mode };
            if transfer_file(src, dest, how).is_ok() && dst_layout.writes_media_tags() {
                media::set_field(&mut entry, field, Some(media::media_ref(dst_layout, to, dest)));
            }
        }
        result.media_unsupported += plan.unsupported;

        if mode == MigrationMode::Move {
            src_list.games.retain(|g| g.path != game.path);
        }
        dst_list.games.push(entry);
        done_paths.push((game.path.clone(), game.path.clone()));
        result.transferred += 1;
    }
    done_paths
}

/// Colecciones: los juegos llevados entran en la misma colección del destino (se crea si no
/// existe) y, al mover, salen de la del origen. Clásica: custom-X.cfg del sistema; ES-DE:
/// las comunes de `<ES-DE>/collections`.
fn transfer_collections(src_layout: &Layout, dst_layout: &Layout, from: &SystemRef, to: &SystemRef, done: &[(String, String)], mode: MigrationMode) {
    if done.is_empty() {
        return;
    }
    let src_dir = src_layout.collections_dir(from);
    let dst_dir = dst_layout.collections_dir(to);
    let Ok(entries) = fs::read_dir(&src_dir) else { return };
    let cfgs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().map(|n| n.to_string_lossy().to_string()).is_some_and(|n| n.starts_with("custom-") && n.ends_with(".cfg")))
        .collect();
    for cfg in cfgs {
        let Ok(content) = fs::read_to_string(&cfg) else { continue };
        let lines: HashSet<&str> = content.lines().map(str::trim).collect();
        let listed: Vec<&(String, String)> = done.iter().filter(|(old, _)| lines.contains(src_layout.collection_entry(from, old).as_str())).collect();
        if listed.is_empty() {
            continue;
        }
        if mode == MigrationMode::Move {
            let gone: HashSet<String> = listed.iter().map(|(old, _)| src_layout.collection_entry(from, old)).collect();
            super::rename::remove_lines(&cfg, |line| gone.contains(line.trim()));
        }
        let target = dst_dir.join(cfg.file_name().unwrap());
        let existing = fs::read_to_string(&target).unwrap_or_default();
        let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
        let mut out = existing.clone();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(newline);
        }
        for (_, new) in listed {
            let line = dst_layout.collection_entry(to, new);
            if !existing.lines().any(|l| l.trim() == line) {
                out.push_str(&line);
                out.push_str(newline);
            }
        }
        if let Some(dir) = target.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(&target, out);
    }
}

/// Lleva el sistema `system_path` del perfil activo al perfil `target_profile`.
#[tauri::command(async)]
pub fn transfer_system(
    app: tauri::AppHandle,
    system_path: String,
    target_profile: String,
    mode: MigrationMode,
    task: Option<String>,
) -> Result<TransferResult, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let (src_layout, from) = super::system_context(&app, &system_path)?;
    let target = settings
        .profiles
        .iter()
        .find(|p| p.id == target_profile)
        .ok_or_else(|| format!("Profile not found: {}", target_profile))?;
    if settings.active_profile().is_some_and(|p| p.id == target.id) {
        return Err("The destination is the active profile".into());
    }
    if target.rom_root_path.trim().is_empty() || !Path::new(&target.rom_root_path).is_dir() {
        return Err(format!("The ROM folder of '{}' doesn't exist", target.name));
    }
    let dst_layout = Layout::for_profile(target)?;
    let to = SystemRef::from_rom_dir(Path::new(&target.rom_root_path).join(&from.name));
    if to.rom_dir == from.rom_dir {
        return Err("Both profiles use the same ROM folder: use \"Migrate structure\" instead".into());
    }
    let progress = Progress::new(&app, task);
    transfer(&src_layout, &dst_layout, &from, &to, mode, &progress)
}

/// Todo el proceso, sin la app (para los tests)
fn transfer(src_layout: &Layout, dst_layout: &Layout, from: &SystemRef, to: &SystemRef, mode: MigrationMode, progress: &Progress) -> Result<TransferResult, String> {
    let mut src_list = super::gamelist::read_gamelist_with(src_layout, from, progress)?;
    let mut dst_list = super::gamelist::read_gamelist(dst_layout, to)?;
    let dst_existed = dst_layout.gamelist_path(to).exists();
    let mut result = TransferResult { destination: to.rom_dir.to_string_lossy().to_string(), ..Default::default() };

    fs::create_dir_all(&to.rom_dir).map_err(|e| format!("Failed to create {}: {}", to.rom_dir.display(), e))?;
    let done = transfer_games(src_layout, dst_layout, from, to, &mut src_list, &mut dst_list, mode, progress, &mut result);

    if !done.is_empty() {
        progress.phase("saving");
        // write_gamelist() hace la copia de seguridad si ya había gamelist
        if dst_existed {
            super::gamelist::write_gamelist(dst_layout, to, &dst_list)?;
        } else {
            super::gamelist::write_gamelist_no_backup(dst_layout, to, &dst_list)?;
        }
        if mode == MigrationMode::Move {
            super::gamelist::write_gamelist(src_layout, from, &src_list)?;
        }
        transfer_collections(src_layout, dst_layout, from, to, &done, mode);
        // El estado de cada juego (guardado por la ruta absoluta de la ROM) va con él
        if mode == MigrationMode::Move {
            super::game_status::remap(&from.rom_dir, &to.rom_dir, &done);
        }
    }

    // Lo que se queda en la carpeta del sistema y no es de ningún juego de la gamelist
    let kept: HashSet<PathBuf> = src_list.games.iter().map(|g| from.rom_dir.join(strip_dot_slash(&g.path))).collect();
    result.files_left = super::new_roms::rom_files(from).iter().filter(|f| !kept.contains(*f)).count();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::EsdePaths;

    fn touch(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn gamelist(paths: &[&str]) -> GameList {
        GameList {
            games: paths.iter().map(|p| Game { path: p.to_string(), name: Some(format!("Juego {}", p)), ..Default::default() }).collect(),
            ..Default::default()
        }
    }

    fn esde(root: &Path) -> Layout {
        Layout::Esde(EsdePaths { home: root.join("ES-DE"), media_root: root.join("ES-DE/downloaded_media") })
    }

    #[test]
    fn classic_to_esde_moves_roms_discs_media_and_gamelist() {
        let tmp = tempfile::tempdir().unwrap();
        let from = SystemRef::from_rom_dir(tmp.path().join("roms/psx"));
        let to = SystemRef::from_rom_dir(tmp.path().join("roms-es-de/psx"));
        let dst = esde(&tmp.path().join("roms-es-de"));
        touch(&from.rom_dir.join("Juego.cue"), "FILE \"Juego.bin\" BINARY\n  TRACK 01 MODE2/2352\n");
        touch(&from.rom_dir.join("Juego.bin"), "bin");
        touch(&from.rom_dir.join("images/Juego-image.png"), "img");
        touch(&from.rom_dir.join("Suelta.cue"), "x"); // no está en la gamelist: se queda
        let mut list = gamelist(&["./Juego.cue"]);
        list.games[0].image = Some("./images/Juego-image.png".into());
        super::super::gamelist::write_gamelist_no_backup(&Layout::Classic, &from, &list).unwrap();

        let r = transfer(&Layout::Classic, &dst, &from, &to, MigrationMode::Move, &Progress::none()).unwrap();
        assert_eq!(r.transferred, 1, "{:?}", r.skipped);
        assert!(to.rom_dir.join("Juego.cue").is_file() && to.rom_dir.join("Juego.bin").is_file());
        assert!(!from.rom_dir.join("Juego.cue").exists() && !from.rom_dir.join("Juego.bin").exists());
        assert!(dst.media_file_path(&to, "covers", "./Juego.cue", "png").is_file());
        let moved = super::super::gamelist::read_gamelist(&dst, &to).unwrap();
        assert_eq!(moved.games.len(), 1);
        assert_eq!(moved.games[0].name.as_deref(), Some("Juego ./Juego.cue"));
        assert!(super::super::gamelist::read_gamelist(&Layout::Classic, &from).unwrap().games.is_empty());
        assert_eq!(r.files_left, 1);
    }

    #[test]
    fn esde_to_classic_copy_writes_media_paths_and_keeps_the_source() {
        let tmp = tempfile::tempdir().unwrap();
        let src = esde(&tmp.path().join("roms-es-de"));
        let from = SystemRef::from_rom_dir(tmp.path().join("roms-es-de/snes"));
        let to = SystemRef::from_rom_dir(tmp.path().join("roms/snes"));
        touch(&from.rom_dir.join("Zelda.sfc"), "rom");
        touch(&src.media_file_path(&from, "covers", "./Zelda.sfc", "jpg"), "img");
        super::super::gamelist::write_gamelist_no_backup(&src, &from, &gamelist(&["./Zelda.sfc"])).unwrap();

        let r = transfer(&src, &Layout::Classic, &from, &to, MigrationMode::Copy, &Progress::none()).unwrap();
        assert_eq!(r.transferred, 1, "{:?}", r.skipped);
        assert!(from.rom_dir.join("Zelda.sfc").is_file(), "copying keeps the source");
        assert!(to.rom_dir.join("Zelda.sfc").is_file());
        let copied = super::super::gamelist::read_gamelist(&Layout::Classic, &to).unwrap();
        let image = copied.games[0].image.clone().unwrap();
        assert!(media::resolve_media_path(&to, &image).is_file(), "media path written: {}", image);
        assert_eq!(super::super::gamelist::read_gamelist(&src, &from).unwrap().games.len(), 1);
    }

    #[test]
    fn games_the_destination_has_are_skipped_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let from = SystemRef::from_rom_dir(tmp.path().join("a/gb"));
        let to = SystemRef::from_rom_dir(tmp.path().join("b/gb"));
        touch(&from.rom_dir.join("Tetris.gb"), "nuevo");
        touch(&from.rom_dir.join("Otro.gb"), "rom");
        touch(&to.rom_dir.join("Tetris.gb"), "el del destino");
        super::super::gamelist::write_gamelist_no_backup(&Layout::Classic, &from, &gamelist(&["./Tetris.gb", "./Otro.gb"])).unwrap();
        super::super::gamelist::write_gamelist_no_backup(&Layout::Classic, &to, &gamelist(&["./Tetris.gb"])).unwrap();

        let r = transfer(&Layout::Classic, &Layout::Classic, &from, &to, MigrationMode::Move, &Progress::none()).unwrap();
        assert_eq!(r.transferred, 1);
        assert_eq!(r.skipped.len(), 1);
        assert_eq!(fs::read_to_string(to.rom_dir.join("Tetris.gb")).unwrap(), "el del destino");
        assert!(from.rom_dir.join("Tetris.gb").is_file(), "the skipped one stays");
        assert_eq!(super::super::gamelist::read_gamelist(&Layout::Classic, &to).unwrap().games.len(), 2);
    }

    #[test]
    fn cancelled_before_starting_moves_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let from = SystemRef::from_rom_dir(tmp.path().join("a/gb"));
        let to = SystemRef::from_rom_dir(tmp.path().join("b/gb"));
        touch(&from.rom_dir.join("Tetris.gb"), "rom");
        super::super::gamelist::write_gamelist_no_backup(&Layout::Classic, &from, &gamelist(&["./Tetris.gb"])).unwrap();
        let progress = Progress::for_task("t-transfer-cancel");
        super::super::progress::cancel_task("t-transfer-cancel".into());

        let r = transfer(&Layout::Classic, &Layout::Classic, &from, &to, MigrationMode::Move, &progress).unwrap();
        assert_eq!(r.transferred, 0);
        assert!(from.rom_dir.join("Tetris.gb").is_file());
        assert!(!Layout::Classic.gamelist_path(&to).exists());
    }
}
