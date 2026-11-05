//! «Exportar a dispositivo»: copia una selección de la biblioteca (ROMs con sus ficheros, media
//! y gamelist) a una carpeta (tarjeta SD, USB, disco, carpeta de red montada) con la misma
//! estructura que el perfil activo, copiando solo lo nuevo o cambiado. Y al revés: trae del
//! destino lo que cambió al jugar allí (favorito, oculto, partidas, última vez, tiempo).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::layout::{media_stem_for, strip_dot_slash, EsdePaths, Layout, SystemRef};
use crate::media::{self, MediaIndex};
use crate::models::{Game, GameList};

/// Lo que se elige en la ventana
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    /// Carpeta de destino
    pub dest: String,
    /// Carpetas de ROMs de los sistemas a exportar
    pub systems: Vec<String>,
    /// «all» | «favorites» | «visible» (todos menos los ocultos)
    pub games: String,
    /// Carpetas de los tipos de media que se copian (covers, screenshots, videos…)
    pub media: Vec<String>,
    /// Quitar del destino lo que ya no está en la selección (dentro de los sistemas elegidos)
    pub remove_extra: bool,
}

/// Estructura del destino: la misma que la del perfil. ES-DE: <destino>/ROMs y <destino>/ES-DE.
fn dest_layout(layout: &Layout, dest: &Path) -> (Layout, PathBuf) {
    match layout {
        Layout::Classic => (Layout::Classic, dest.to_path_buf()),
        Layout::Esde(_) => {
            let home = dest.join("ES-DE");
            let media_root = home.join("downloaded_media");
            (Layout::Esde(EsdePaths { home, media_root }), dest.join("ROMs"))
        }
    }
}

fn wanted(game: &Game, filter: &str) -> bool {
    match filter {
        "favorites" => game.favorite.as_deref() == Some("true"),
        "visible" => game.hidden.as_deref() != Some("true"),
        _ => true,
    }
}

/// Todos los ficheros de una carpeta (una ROM que es carpeta, p. ej. «Juego.ps3»)
fn files_in(dir: &Path) -> Vec<PathBuf> {
    media::list_files_recursive(dir)
}

/// ¿Hay que copiarlo? Si falta, si cambia el tamaño o si el original es más nuevo
fn needs_copy(src: &Path, dst: &Path) -> bool {
    let (Ok(s), Ok(d)) = (fs::metadata(src), fs::metadata(dst)) else { return true };
    if s.len() != d.len() {
        return true;
    }
    let secs = |m: &fs::Metadata| m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    // 2 s de margen: FAT32 (tarjetas SD) guarda la fecha con esa precisión
    secs(&s) > secs(&d) + 2
}

/// Lo que se hará con un sistema
struct SystemPlan {
    dst: SystemRef,
    /// Gamelist del destino, solo con los juegos exportados
    gamelist: GameList,
    /// (origen, destino) de cada fichero que hay que copiar
    copies: Vec<(PathBuf, PathBuf, u64)>,
    up_to_date: usize,
    /// Ficheros del destino que ya no están en la selección (si se pidió quitarlos)
    extra: Vec<PathBuf>,
}

fn plan_system(layout: &Layout, dst_layout: &Layout, src: &SystemRef, dst: SystemRef, opts: &ExportOptions) -> Result<SystemPlan, String> {
    let list = super::gamelist::read_gamelist(layout, src)?;
    let index = MediaIndex::build(layout, src);
    let kinds: Vec<&media::MediaKind> = media::MEDIA_KINDS.iter().filter(|k| opts.media.iter().any(|m| m == k.folder)).collect();

    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut games: Vec<Game> = Vec::new();
    for game in list.games.iter().filter(|g| wanted(g, &opts.games)) {
        let rom = src.rom_dir.join(strip_dot_slash(&game.path));
        if !rom.exists() {
            continue; // sin ROM no se exporta
        }
        let is_dir = rom.is_dir();
        let rom_files: Vec<PathBuf> = if is_dir {
            files_in(&rom)
        } else {
            std::iter::once(rom.clone()).chain(super::move_games::companion_files(&rom)).collect()
        };
        for file in rom_files {
            if let Ok(rel) = file.strip_prefix(&src.rom_dir) {
                pairs.push((file.clone(), dst.rom_dir.join(rel)));
            }
        }

        let mut out = game.clone();
        if layout.writes_media_tags() {
            media::clear_media_fields(&mut out); // solo quedan los tipos copiados
        }
        for kind in &kinds {
            let Some(file) = super::media_kinds::current_file(layout, src, &index, game, kind) else { continue };
            let ext = media::extension_lower(&file).unwrap_or_else(|| "png".into());
            let target = dst_layout.media_file_for_stem(&dst, kind.folder, &media_stem_for(&game.path, is_dir), &ext);
            if dst_layout.writes_media_tags() {
                media::set_field(&mut out, kind.field, Some(media::media_ref(dst_layout, &dst, &target)));
            }
            pairs.push((file, target));
        }
        games.push(out);
    }

    // Lo que hay que copiar y lo que ya está al día
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut copies = Vec::new();
    let mut up_to_date = 0;
    for (from, to) in pairs {
        if !seen.insert(to.clone()) {
            continue;
        }
        if needs_copy(&from, &to) {
            let size = fs::metadata(&from).map(|m| m.len()).unwrap_or(0);
            copies.push((from, to, size));
        } else {
            up_to_date += 1;
        }
    }

    // Lo que sobra en el destino: solo dentro de las carpetas de este sistema
    let mut extra = Vec::new();
    if opts.remove_extra {
        let gamelist = dst_layout.gamelist_path(&dst);
        let mut roots = vec![dst.rom_dir.clone()];
        let media_dir = dst_layout.media_type_dir(&dst, "");
        if !media_dir.starts_with(&dst.rom_dir) {
            roots.push(media_dir);
        }
        for root in roots {
            for file in media::list_files_recursive(&root) {
                let hidden = file.strip_prefix(&root).ok().is_some_and(|rel| rel.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')));
                if file == gamelist || hidden || seen.contains(&file) {
                    continue;
                }
                extra.push(file);
            }
        }
    }

    Ok(SystemPlan { dst, gamelist: GameList { games, folders: list.folders, provider: list.provider }, copies, up_to_date, extra })
}

fn plan_all(app: &tauri::AppHandle, opts: &ExportOptions) -> Result<(Layout, Vec<SystemPlan>), String> {
    let dest = PathBuf::from(&opts.dest);
    if opts.dest.trim().is_empty() || !dest.is_dir() {
        return Err(format!("Destination folder not found: {}", opts.dest));
    }
    let layout = super::active_layout(app)?;
    let (dst_layout, rom_root) = dest_layout(&layout, &dest);
    let mut plans = Vec::new();
    for system_path in &opts.systems {
        let src = SystemRef::from_rom_dir(system_path);
        // El destino no puede ser la propia biblioteca
        if rom_root.join(&src.name) == src.rom_dir {
            return Err("The destination is the library itself".into());
        }
        let dst = SystemRef::from_rom_dir(rom_root.join(&src.name));
        plans.push(plan_system(&layout, &dst_layout, &src, dst, opts)?);
    }
    Ok((dst_layout, plans))
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportPreview {
    pub copy_files: usize,
    pub copy_bytes: u64,
    pub up_to_date: usize,
    pub remove_files: usize,
    /// Algunos de los que se quitarían (para enseñarlos)
    pub remove_sample: Vec<String>,
    pub games: usize,
}

/// Vista previa: qué se copiaría, cuánto ocupa, qué ya está y qué se quitaría
#[tauri::command]
pub async fn plan_export(app: tauri::AppHandle, options: ExportOptions) -> Result<ExportPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, plans) = plan_all(&app, &options)?;
        let extra: Vec<&PathBuf> = plans.iter().flat_map(|p| p.extra.iter()).collect();
        Ok(ExportPreview {
            copy_files: plans.iter().map(|p| p.copies.len()).sum(),
            copy_bytes: plans.iter().flat_map(|p| p.copies.iter()).map(|(_, _, s)| s).sum(),
            up_to_date: plans.iter().map(|p| p.up_to_date).sum(),
            remove_files: extra.len(),
            remove_sample: extra.iter().take(30).map(|p| p.to_string_lossy().to_string()).collect(),
            games: plans.iter().map(|p| p.gamelist.games.len()).sum(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

static CANCEL: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn cancel_export() {
    CANCEL.store(true, Ordering::SeqCst);
}

#[derive(Debug, Clone, Serialize)]
struct ExportProgress {
    current: usize,
    total: usize,
    bytes: u64,
    total_bytes: u64,
    file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub copied: usize,
    pub removed: usize,
    pub failed: Vec<String>,
    pub stopped: bool,
}

/// En la gamelist del destino se respetan las estadísticas que ya tenía (jugar allí las sube):
/// así exportar otra vez sin haber traído los cambios no las pierde
fn keep_device_stats(out: &mut GameList, existing: &GameList) {
    let by_path: HashMap<&str, &Game> = existing.games.iter().map(|g| (g.path.as_str(), g)).collect();
    for game in &mut out.games {
        let Some(old) = by_path.get(game.path.as_str()) else { continue };
        if num(&old.playcount) > num(&game.playcount) {
            game.playcount = old.playcount.clone();
        }
        if num(&old.gametime) > num(&game.gametime) {
            game.gametime = old.gametime.clone();
        }
        if old.lastplayed.as_deref().unwrap_or("") > game.lastplayed.as_deref().unwrap_or("") {
            game.lastplayed = old.lastplayed.clone();
        }
    }
}

fn num(v: &Option<String>) -> u64 {
    v.as_deref().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

/// Exporta: copia, escribe las gamelists del destino y (si se pidió) quita lo que sobra.
/// Emite `export-progress`; `cancel_export` lo para entre fichero y fichero.
#[tauri::command]
pub async fn run_export(app: tauri::AppHandle, options: ExportOptions) -> Result<ExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        CANCEL.store(false, Ordering::SeqCst);
        let (dst_layout, plans) = plan_all(&app, &options)?;
        let total: usize = plans.iter().map(|p| p.copies.len()).sum();
        let total_bytes: u64 = plans.iter().flat_map(|p| p.copies.iter()).map(|(_, _, s)| s).sum();
        let mut result = ExportResult { copied: 0, removed: 0, failed: Vec::new(), stopped: false };
        let mut bytes = 0u64;
        let mut i = 0;

        'systems: for plan in &plans {
            for (from, to, size) in &plan.copies {
                if CANCEL.load(Ordering::SeqCst) {
                    result.stopped = true;
                    break 'systems;
                }
                i += 1;
                let _ = app.emit(
                    "export-progress",
                    ExportProgress { current: i, total, bytes, total_bytes, file: to.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default() },
                );
                let copied = to.parent().map_or(Ok(()), fs::create_dir_all).and_then(|_| fs::copy(from, to).map(|_| ()));
                match copied {
                    Ok(()) => result.copied += 1,
                    Err(e) => result.failed.push(format!("{} ({})", to.display(), e)),
                }
                bytes += size;
            }
        }

        // Gamelists del destino (también si se paró: lo copiado ya cuenta)
        for plan in &plans {
            let mut list = plan.gamelist.clone();
            let existing = dst_layout.gamelist_path(&plan.dst);
            if let Ok(xml) = fs::read_to_string(&existing) {
                if let Ok(old) = crate::parser::parse_gamelist(&xml) {
                    keep_device_stats(&mut list, &old);
                }
            }
            if result.stopped {
                // Solo los juegos cuya ROM ya está en el destino
                list.games.retain(|g| plan.dst.rom_dir.join(strip_dot_slash(&g.path)).exists());
            }
            if let Err(e) = super::gamelist::write_gamelist_no_backup(&dst_layout, &plan.dst, &list) {
                result.failed.push(format!("{} ({})", existing.display(), e));
            }
        }

        if !result.stopped {
            for file in plans.iter().flat_map(|p| p.extra.iter()) {
                match fs::remove_file(file) {
                    Ok(()) => result.removed += 1,
                    Err(e) => result.failed.push(format!("{} ({})", file.display(), e)),
                }
            }
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- Traer cambios del destino ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceChange {
    /// Carpeta de ROMs del sistema en la biblioteca
    pub system_path: String,
    pub system: String,
    pub game_path: String,
    pub name: String,
    /// favorite | hidden | playcount | lastplayed | gametime
    pub field: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// Cambios del destino respecto a la biblioteca: favorito/oculto si son distintos; partidas,
/// tiempo y última vez si allí son mayores
fn diff(system_path: &str, system: &str, local: &[Game], device: &[Game]) -> Vec<DeviceChange> {
    let by_path: HashMap<&str, &Game> = device.iter().map(|g| (g.path.as_str(), g)).collect();
    let flag = |v: &Option<String>| v.as_deref() == Some("true");
    let mut out = Vec::new();
    for game in local {
        let Some(d) = by_path.get(game.path.as_str()) else { continue };
        let mut push = |field: &str, from: &Option<String>, to: &Option<String>| {
            out.push(DeviceChange {
                system_path: system_path.to_string(),
                system: system.to_string(),
                game_path: game.path.clone(),
                name: game.name.clone().unwrap_or_else(|| game.path.clone()),
                field: field.to_string(),
                from: from.clone(),
                to: to.clone(),
            });
        };
        if flag(&game.favorite) != flag(&d.favorite) {
            push("favorite", &game.favorite, &d.favorite);
        }
        if flag(&game.hidden) != flag(&d.hidden) {
            push("hidden", &game.hidden, &d.hidden);
        }
        if num(&d.playcount) > num(&game.playcount) {
            push("playcount", &game.playcount, &d.playcount);
        }
        if num(&d.gametime) > num(&game.gametime) {
            push("gametime", &game.gametime, &d.gametime);
        }
        if d.lastplayed.as_deref().unwrap_or("") > game.lastplayed.as_deref().unwrap_or("") {
            push("lastplayed", &game.lastplayed, &d.lastplayed);
        }
    }
    out
}

/// Lo que cambió en el destino (en los sistemas de la biblioteca que también están allí)
#[tauri::command]
pub fn pull_changes(app: tauri::AppHandle, dest: String, systems: Vec<String>) -> Result<Vec<DeviceChange>, String> {
    let layout = super::active_layout(&app)?;
    let (dst_layout, rom_root) = dest_layout(&layout, Path::new(&dest));
    let mut out = Vec::new();
    for system_path in systems {
        let src = SystemRef::from_rom_dir(&system_path);
        let dst = SystemRef::from_rom_dir(rom_root.join(&src.name));
        let Ok(xml) = fs::read_to_string(dst_layout.gamelist_path(&dst)) else { continue };
        let device = crate::parser::parse_gamelist(&xml)?;
        let local = super::gamelist::read_gamelist(&layout, &src)?;
        out.extend(diff(&system_path, &src.name, &local.games, &device.games));
    }
    Ok(out)
}

/// Escribe en la biblioteca los cambios elegidos (una escritura por sistema, con copia)
#[tauri::command]
pub fn apply_pull(app: tauri::AppHandle, changes: Vec<DeviceChange>) -> Result<usize, String> {
    let layout = super::active_layout(&app)?;
    let mut by_system: HashMap<String, Vec<DeviceChange>> = HashMap::new();
    for c in changes {
        by_system.entry(c.system_path.clone()).or_default().push(c);
    }
    let mut applied = 0;
    for (system_path, list) in by_system {
        let system = SystemRef::from_rom_dir(&system_path);
        let mut gamelist = super::gamelist::read_gamelist(&layout, &system)?;
        for c in &list {
            let Some(game) = gamelist.games.iter_mut().find(|g| g.path == c.game_path) else { continue };
            let slot = match c.field.as_str() {
                "favorite" => &mut game.favorite,
                "hidden" => &mut game.hidden,
                "playcount" => &mut game.playcount,
                "gametime" => &mut game.gametime,
                "lastplayed" => &mut game.lastplayed,
                _ => continue,
            };
            *slot = c.to.clone();
            applied += 1;
        }
        super::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn opts(dest: &Path, games: &str, media: &[&str], remove: bool) -> ExportOptions {
        ExportOptions {
            dest: dest.to_string_lossy().to_string(),
            systems: Vec::new(),
            games: games.into(),
            media: media.iter().map(|m| m.to_string()).collect(),
            remove_extra: remove,
        }
    }

    fn write_list(layout: &Layout, system: &SystemRef, games: Vec<Game>) {
        super::super::gamelist::write_gamelist_no_backup(layout, system, &GameList { games, ..Default::default() }).unwrap();
    }

    #[test]
    fn classic_copies_roms_chosen_media_and_a_gamelist_without_broken_links() {
        let tmp = tempfile::tempdir().unwrap();
        let src = SystemRef::from_rom_dir(tmp.path().join("lib/snes"));
        let dest = tmp.path().join("sd");
        fs::create_dir_all(&dest).unwrap();
        touch(&src.rom_dir.join("Zelda.sfc"), "rom");
        touch(&src.rom_dir.join("Mario.sfc"), "rom2");
        touch(&src.rom_dir.join("covers/Zelda.png"), "img");
        touch(&src.rom_dir.join("videos/Zelda.mp4"), "vid");
        write_list(&Layout::Classic, &src, vec![
            Game { path: "./Zelda.sfc".into(), favorite: Some("true".into()), image: Some("./covers/Zelda.png".into()), video: Some("./videos/Zelda.mp4".into()), ..Default::default() },
            Game { path: "./Mario.sfc".into(), ..Default::default() },
        ]);
        let (dl, root) = dest_layout(&Layout::Classic, &dest);
        let dst = SystemRef::from_rom_dir(root.join("snes"));
        let plan = plan_system(&Layout::Classic, &dl, &src, dst.clone(), &opts(&dest, "favorites", &["covers"], false)).unwrap();

        let targets: Vec<PathBuf> = plan.copies.iter().map(|(_, t, _)| t.clone()).collect();
        assert_eq!(targets, vec![dest.join("snes/Zelda.sfc"), dest.join("snes/covers/Zelda.png")], "only favourites, no videos");
        assert_eq!(plan.gamelist.games.len(), 1);
        assert_eq!(plan.gamelist.games[0].image.as_deref(), Some("./covers/Zelda.png"));
        assert_eq!(plan.gamelist.games[0].video, None, "the video wasn't copied: no link to it");
    }

    #[test]
    fn esde_goes_to_roms_and_es_de_folders_and_up_to_date_files_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Esde(EsdePaths { home: tmp.path().join("ES-DE"), media_root: tmp.path().join("ES-DE/downloaded_media") });
        let src = SystemRef::from_rom_dir(tmp.path().join("ROMs/gb"));
        touch(&src.rom_dir.join("Tetris.gb"), "rom");
        touch(&tmp.path().join("ES-DE/downloaded_media/gb/covers/Tetris.png"), "img");
        write_list(&layout, &src, vec![Game { path: "./Tetris.gb".into(), ..Default::default() }]);
        let dest = tmp.path().join("sd");
        let (dl, root) = dest_layout(&layout, &dest);
        let dst = SystemRef::from_rom_dir(root.join("gb"));

        let plan = plan_system(&layout, &dl, &src, dst.clone(), &opts(&dest, "all", &["covers"], false)).unwrap();
        let targets: Vec<PathBuf> = plan.copies.iter().map(|(_, t, _)| t.clone()).collect();
        assert_eq!(targets, vec![dest.join("ROMs/gb/Tetris.gb"), dest.join("ES-DE/downloaded_media/gb/covers/Tetris.png")]);
        assert_eq!(dl.gamelist_path(&dst), dest.join("ES-DE/gamelists/gb/gamelist.xml"));

        // Ya copiado: la segunda vez no hay nada que copiar
        for (from, to, _) in &plan.copies {
            touch(to, &fs::read_to_string(from).unwrap());
        }
        let again = plan_system(&layout, &dl, &src, dst, &opts(&dest, "all", &["covers"], false)).unwrap();
        assert!(again.copies.is_empty());
        assert_eq!(again.up_to_date, 2);
    }

    #[test]
    fn leftovers_are_only_looked_for_inside_the_chosen_systems() {
        let tmp = tempfile::tempdir().unwrap();
        let src = SystemRef::from_rom_dir(tmp.path().join("lib/nes"));
        touch(&src.rom_dir.join("A.nes"), "a");
        write_list(&Layout::Classic, &src, vec![Game { path: "./A.nes".into(), ..Default::default() }]);
        let dest = tmp.path().join("sd");
        touch(&dest.join("nes/Viejo.nes"), "x");
        touch(&dest.join("nes/gamelist.xml"), "<gameList/>");
        touch(&dest.join("nes/.backups/g.xml"), "x");
        touch(&dest.join("snes/Otro.sfc"), "x"); // otro sistema: no se toca
        let (dl, root) = dest_layout(&Layout::Classic, &dest);
        let plan = plan_system(&Layout::Classic, &dl, &src, SystemRef::from_rom_dir(root.join("nes")), &opts(&dest, "all", &[], true)).unwrap();
        assert_eq!(plan.extra, vec![dest.join("nes/Viejo.nes")]);
    }

    #[test]
    fn device_changes_and_kept_stats() {
        let local = vec![Game { path: "./a".into(), playcount: Some("1".into()), lastplayed: Some("20240101T000000".into()), ..Default::default() }];
        let device = vec![Game {
            path: "./a".into(),
            favorite: Some("true".into()),
            playcount: Some("5".into()),
            lastplayed: Some("20250101T000000".into()),
            ..Default::default()
        }];
        let changes = diff("/lib/nes", "nes", &local, &device);
        let fields: Vec<&str> = changes.iter().map(|c| c.field.as_str()).collect();
        assert_eq!(fields, vec!["favorite", "playcount", "lastplayed"]);

        // Exportar otra vez no baja las estadísticas del dispositivo
        let mut out = GameList { games: local.clone(), ..Default::default() };
        keep_device_stats(&mut out, &GameList { games: device, ..Default::default() });
        assert_eq!(out.games[0].playcount.as_deref(), Some("5"));
        assert_eq!(out.games[0].lastplayed.as_deref(), Some("20250101T000000"));
    }
}
