//! «Eliminar sistema»: quita un sistema entero de la biblioteca. Siempre se van las ROMs, la
//! gamelist (con sus copias), los medios y el resto de la carpeta del sistema. Además, si se
//! marca:
//! - las carpetas de emuladores (subcarpetas que no son de medios ni tienen ROMs de la gamelist,
//!   p. ej. `fbneo/`, `mame2003-plus/`);
//! - las partidas guardadas del sistema (`<partidas>/<sistema>`);
//! - lo de fuera: en ES-DE, sus líneas en las colecciones personalizadas, y lo que meROMS guarda
//!   del sistema (estados de juego, «no encontrados», DAT, vista del sistema, cachés).
//!
//! A la papelera o borrado definitivo, a elegir. No hay Deshacer (para eso está la papelera).
//! Cada ruta se comprueba antes de borrar: solo dentro de las carpetas de ese sistema.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::{strip_dot_slash, Layout, SystemRef};
use crate::media::MEDIA_KINDS;
use crate::models::{Game, Profile};

use super::progress::Progress;

/// Una carpeta de emulador (o cualquier otra que no es de medios ni tiene ROMs)
#[derive(Debug, Clone, Serialize)]
pub struct ExtraDir {
    pub name: String,
    pub bytes: u64,
}

/// Lo que se iría, para la ventana de confirmación
#[derive(Debug, Clone, Default, Serialize)]
pub struct DeletePlan {
    /// Nombre de la carpeta del sistema: hay que escribirlo para confirmar
    pub name: String,
    pub rom_dir: String,
    /// ES-DE: la carpeta de medios y la de la gamelist del sistema (en clásica están dentro de
    /// la carpeta de ROMs)
    pub media_dir: Option<String>,
    pub gamelist_dir: Option<String>,
    /// Juegos de la gamelist y ficheros de medios
    pub games: usize,
    pub media_files: usize,
    /// Lo que se va siempre (sin carpetas de emuladores ni partidas)
    pub bytes: u64,
    pub emulator_dirs: Vec<ExtraDir>,
    pub saves_dir: Option<String>,
    pub saves_files: usize,
    pub saves_bytes: u64,
    /// ES-DE: líneas de este sistema en las colecciones personalizadas
    pub collection_lines: usize,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct DeleteOptions {
    /// Borrar sin pasar por la papelera
    pub permanent: bool,
    pub emulators: bool,
    pub saves: bool,
    /// Colecciones de ES-DE y datos de meROMS de este sistema
    pub external: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DeleteResult {
    /// Ficheros o carpetas quitados (una carpeta entera cuenta como uno)
    pub removed: usize,
    pub failed: Vec<String>,
    /// Se canceló a medias (lo hecho, hecho está)
    pub stopped: bool,
}

#[tauri::command(async)]
pub fn plan_delete_system(app: tauri::AppHandle, system_path: String) -> Result<DeletePlan, String> {
    let (layout, system, profile) = context(&app, &system_path)?;
    let games = super::gamelist::read_gamelist(&layout, &system).map(|g| g.games).unwrap_or_default();
    Ok(plan(&layout, &system, &games, profile.as_ref()))
}

#[tauri::command(async)]
pub fn delete_system(app: tauri::AppHandle, system_path: String, options: DeleteOptions, task: Option<String>) -> Result<DeleteResult, String> {
    let (layout, system, profile) = context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    let games = super::gamelist::read_gamelist(&layout, &system).map(|g| g.games).unwrap_or_default();

    // Lo de fuera primero (necesita la gamelist y las rutas de los juegos, que van a desaparecer)
    if options.external {
        forget_external(&app, &layout, &system, &games, profile.as_ref());
    }
    let targets = targets(&layout, &system, &games, profile.as_ref(), options);
    // Las carpetas del sistema se quedan, vacías: solo se va lo de dentro
    let mut result = remove(&targets, options.permanent, &progress);
    if progress.cancelled() {
        result.stopped = true;
    }
    Ok(result)
}

/// Layout, sistema y perfil activo; el sistema tiene que ser una carpeta directa de la de ROMs
fn context(app: &tauri::AppHandle, system_path: &str) -> Result<(Layout, SystemRef, Option<Profile>), String> {
    let settings = super::settings::get_settings(app.clone())?;
    let profile = settings.active_profile().cloned();
    let (layout, system) = super::system_context(app, system_path)?;
    if let Some(p) = &profile {
        if system.rom_dir.parent() != Some(Path::new(&p.rom_root_path)) {
            return Err("The system is not a folder of the profile's ROM folder".into());
        }
    }
    Ok((layout, system, profile))
}

fn plan(layout: &Layout, system: &SystemRef, games: &[Game], profile: Option<&Profile>) -> DeletePlan {
    let extra = emulator_dirs(layout, system, games);
    let saves = profile.and_then(|p| saves_of(p, system));
    let esde = match layout {
        Layout::Esde(_) => Some((layout.media_type_dir(system, ""), layout.gamelist_path(system).parent().map(Path::to_path_buf))),
        Layout::Classic => None,
    };
    let mut bytes = dir_size_without(&system.rom_dir, &extra);
    let mut media_files = media_folders(layout, system).iter().map(|d| count_files(d)).sum();
    if let Some((media, gamelists)) = &esde {
        bytes += dir_size(media);
        media_files = count_files(media);
        bytes += gamelists.as_deref().map_or(0, dir_size);
    }
    DeletePlan {
        name: system.name.clone(),
        rom_dir: system.rom_dir.to_string_lossy().to_string(),
        media_dir: esde.as_ref().map(|(m, _)| m.to_string_lossy().to_string()),
        gamelist_dir: esde.as_ref().and_then(|(_, g)| g.as_ref()).map(|g| g.to_string_lossy().to_string()),
        games: games.len(),
        media_files,
        bytes,
        emulator_dirs: extra.iter().map(|d| ExtraDir { name: name_of(d), bytes: dir_size(d) }).collect(),
        saves_files: saves.as_deref().map_or(0, count_files),
        saves_bytes: saves.as_deref().map_or(0, dir_size),
        saves_dir: saves.map(|s| s.to_string_lossy().to_string()),
        collection_lines: esde_collection_files(layout, system).iter().map(|(_, lines)| *lines).sum(),
    }
}

/// Carpetas de medios dentro de la carpeta del sistema (clásica, o restos clásicos en ES-DE)
fn media_folders(layout: &Layout, system: &SystemRef) -> Vec<PathBuf> {
    let base = match layout {
        Layout::Classic => system.rom_dir.clone(),
        Layout::Esde(_) => return Vec::new(),
    };
    media_folder_names().iter().map(|n| base.join(n)).filter(|d| d.is_dir()).collect()
}

/// Nombres de carpetas de medios: los tipos, los antiguos y los que mira Limpiar medios
fn media_folder_names() -> HashSet<String> {
    let mut names: HashSet<String> = MEDIA_KINDS.iter().map(|k| k.folder.to_lowercase()).collect();
    names.extend(super::cleanup::MEDIA_DIRS.iter().map(|d| d.split('/').next().unwrap_or(d).to_lowercase()));
    for (folders, _, _) in super::images::legacy_targets() {
        names.extend(folders.iter().map(|f| f.to_lowercase()));
    }
    names
}

/// Subcarpetas de la carpeta del sistema que no son de medios, ni ocultas, ni propias de ES-DE,
/// ni tienen ninguna ROM de la gamelist (ni son ellas una ROM-carpeta): las de los emuladores
fn emulator_dirs(layout: &Layout, system: &SystemRef, games: &[Game]) -> Vec<PathBuf> {
    let media = media_folder_names();
    let with_roms: HashSet<String> = games
        .iter()
        .filter_map(|g| strip_dot_slash(&g.path).split('/').next().map(str::to_lowercase))
        .collect();
    let Ok(entries) = fs::read_dir(&system.rom_dir) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|p| {
            let name = name_of(p).to_lowercase();
            !name.starts_with('.') && !media.contains(&name) && !with_roms.contains(&name) && !layout.is_own_folder(p)
        })
        .collect();
    dirs.sort();
    dirs
}

/// Partidas del sistema: `<carpeta de partidas>/<sistema>`, si existe
fn saves_of(profile: &Profile, system: &SystemRef) -> Option<PathBuf> {
    super::saves::saves_dir(profile).map(|d| d.join(&system.name)).filter(|d| d.is_dir())
}

/// Lo que se borra, ya comprobado: nada fuera de las carpetas de este sistema. Las carpetas en
/// sí se quedan (vacías): se borra lo que tienen dentro, no ellas
fn targets(layout: &Layout, system: &SystemRef, games: &[Game], profile: Option<&Profile>, options: DeleteOptions) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // ES-DE: lo de dentro de su carpeta de medios y de la de su gamelist
    if let Layout::Esde(_) = layout {
        out.extend(contents_of(&layout.media_type_dir(system, ""), &[]));
        if let Some(dir) = layout.gamelist_path(system).parent() {
            out.extend(contents_of(dir, &[]));
        }
    }
    // La carpeta del sistema: todo lo de dentro, o todo menos las carpetas de emuladores
    let keep: Vec<PathBuf> = if options.emulators { Vec::new() } else { emulator_dirs(layout, system, games) };
    out.extend(contents_of(&system.rom_dir, &keep));
    if options.saves {
        if let Some(dir) = profile.and_then(|p| saves_of(p, system)) {
            out.extend(contents_of(&dir, &[]));
        }
    }
    let roots = allowed_roots(layout, system, profile);
    out.into_iter().filter(|p| p.exists() && is_allowed(p, &roots, layout, profile)).collect()
}

/// Lo que hay dentro de una carpeta (sin entrar más), menos lo que se pide conservar
fn contents_of(dir: &Path, keep: &[PathBuf]) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut inside: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| !keep.contains(p)).collect();
    inside.sort();
    inside
}

/// Dentro de qué carpetas se puede borrar: la del sistema, su carpeta de medios y la de su
/// gamelist en ES-DE, y sus partidas
fn allowed_roots(layout: &Layout, system: &SystemRef, profile: Option<&Profile>) -> Vec<PathBuf> {
    let mut roots = vec![system.rom_dir.clone()];
    if let Layout::Esde(_) = layout {
        roots.push(layout.media_type_dir(system, ""));
        roots.extend(layout.gamelist_path(system).parent().map(Path::to_path_buf));
    }
    roots.extend(profile.and_then(|p| super::saves::saves_dir(p)).map(|d| d.join(&system.name)));
    roots
}

/// La ruta está dentro de alguna raíz permitida (o es ella) y no es ninguna de las carpetas
/// generales (la de ROMs, la de ES-DE, la de medios, la de partidas)
fn is_allowed(path: &Path, roots: &[PathBuf], layout: &Layout, profile: Option<&Profile>) -> bool {
    let mut forbidden: Vec<PathBuf> = Vec::new();
    if let Some(p) = profile {
        forbidden.push(PathBuf::from(&p.rom_root_path));
        forbidden.extend(super::saves::saves_dir(p));
    }
    if let Layout::Esde(esde) = layout {
        forbidden.push(esde.home.clone());
        forbidden.push(esde.media_root.clone());
        forbidden.push(esde.home.join("gamelists"));
    }
    let sys_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    !sys_name.is_empty() && roots.iter().any(|r| path.starts_with(r)) && !forbidden.iter().any(|f| f == path) && !layout.is_own_folder(path)
}

/// A la papelera (por tandas, con progreso y Cancelar) o borrado definitivo
fn remove(targets: &[PathBuf], permanent: bool, progress: &Progress) -> DeleteResult {
    let mut result = DeleteResult::default();
    if permanent {
        for (i, path) in targets.iter().enumerate() {
            if progress.cancelled() {
                result.stopped = true;
                break;
            }
            progress.step("deleting", i + 1, targets.len(), &path.to_string_lossy());
            let done = if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
            match done {
                Ok(()) => result.removed += 1,
                Err(e) => result.failed.push(format!("{} ({})", path.display(), e)),
            }
        }
    } else {
        let (done, errors) = crate::media::trash_files_in_batches(targets, progress);
        result.removed = done - errors.len();
        result.failed = errors.into_iter().map(|(p, e)| format!("{} ({})", p.display(), e)).collect();
        result.stopped = done < targets.len();
    }
    result
}

/// Colecciones de ES-DE con líneas de este sistema, y cuántas
fn esde_collection_files(layout: &Layout, system: &SystemRef) -> Vec<(PathBuf, usize)> {
    let Layout::Esde(_) = layout else { return Vec::new() };
    let dir = layout.collections_dir(system);
    let Ok(entries) = fs::read_dir(&dir) else { return Vec::new() };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| name_of(p).starts_with("custom-") && p.extension().is_some_and(|x| x == "cfg"))
        .filter_map(|p| {
            let text = fs::read_to_string(&p).ok()?;
            let n = text.lines().filter(|l| is_line_of(l, system)).count();
            (n > 0).then_some((p, n))
        })
        .collect()
}

/// Una línea de colección de ES-DE que es de este sistema (`%ROMPATH%/<sistema>/…`)
fn is_line_of(line: &str, system: &SystemRef) -> bool {
    let line = line.trim().replace('\\', "/");
    line.starts_with(&format!("%ROMPATH%/{}/", system.name))
}

/// Quita las líneas del sistema de las colecciones de ES-DE (manteniendo los finales de línea)
fn clean_collections(layout: &Layout, system: &SystemRef) {
    for (path, _) in esde_collection_files(layout, system) {
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let kept: String = text.split_inclusive('\n').filter(|l| !is_line_of(l, system)).collect();
        let _ = fs::write(&path, kept);
    }
}

/// Colecciones de ES-DE y lo que meROMS guarda de este sistema
fn forget_external(app: &tauri::AppHandle, layout: &Layout, system: &SystemRef, games: &[Game], profile: Option<&Profile>) {
    clean_collections(layout, system);
    let paths: Vec<String> = games.iter().map(|g| g.path.clone()).collect();
    super::game_status::forget(&system.rom_dir, &paths);
    super::scrape_misses::forget(&system.rom_dir, &paths);
    let system_path = system.rom_dir.to_string_lossy().to_string();
    let _ = super::dat::clear_system_dat(app.clone(), system_path.clone());
    super::retroachievements::forget_system(app, &system_path);
    super::system_status::forget_system_status(app.clone(), system_path);
    // La vista del sistema en el perfil
    if let (Some(profile), Ok(mut settings)) = (profile, super::settings::get_settings(app.clone())) {
        let key = crate::models::profile::system_key(&system.name);
        if let Some(p) = settings.profiles.iter_mut().find(|p| p.id == profile.id) {
            if p.system_views.as_mut().is_some_and(|v| v.remove(&key).is_some()) {
                let _ = super::settings::save_settings(app.clone(), settings);
            }
        }
    }
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Ficheros dentro de una carpeta (sin seguir enlaces)
fn count_files(dir: &Path) -> usize {
    walk(dir).len()
}

fn dir_size(dir: &Path) -> u64 {
    walk(dir).iter().map(|(_, size)| size).sum()
}

/// Tamaño de una carpeta sin las subcarpetas `skip`
fn dir_size_without(dir: &Path, skip: &[PathBuf]) -> u64 {
    walk(dir).iter().filter(|(p, _)| !skip.iter().any(|s| p.starts_with(s))).map(|(_, size)| size).sum()
}

fn walk(dir: &Path) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                out.push((entry.path(), entry.metadata().map(|m| m.len()).unwrap_or(0)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::EsdePaths;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    fn game(path: &str) -> Game {
        Game { path: path.into(), ..Default::default() }
    }

    /// Clásica: ROMs sueltas y en subcarpeta, medios, copias y dos carpetas de emuladores
    fn classic(root: &Path) -> (Layout, SystemRef, Vec<Game>) {
        let rom_dir = root.join("ROMs/cps1");
        touch(&rom_dir.join("Game A.zip"));
        touch(&rom_dir.join("hacks/Game B.zip"));
        touch(&rom_dir.join("gamelist.xml"));
        touch(&rom_dir.join(".backups/gamelist_1.xml"));
        touch(&rom_dir.join("covers/Game A.png"));
        touch(&rom_dir.join("images/Game A-image.png"));
        touch(&rom_dir.join("fbneo/a.nv"));
        touch(&rom_dir.join("mame2003-plus/cfg/a.cfg"));
        (Layout::Classic, SystemRef::from_rom_dir(&rom_dir), vec![game("./Game A.zip"), game("./hacks/Game B.zip")])
    }

    #[test]
    fn emulator_folders_are_the_ones_without_roms_or_media() {
        let tmp = tempfile::tempdir().unwrap();
        let (layout, system, games) = classic(tmp.path());
        let names: Vec<String> = emulator_dirs(&layout, &system, &games).iter().map(|d| name_of(d)).collect();
        assert_eq!(names, ["fbneo", "mame2003-plus"]);
        let plan = plan(&layout, &system, &games, None);
        assert_eq!((plan.games, plan.media_files), (2, 2));
    }

    #[test]
    fn without_emulators_everything_else_goes_and_they_stay() {
        let tmp = tempfile::tempdir().unwrap();
        let (layout, system, games) = classic(tmp.path());
        let opts = DeleteOptions { permanent: true, ..Default::default() };
        let list = targets(&layout, &system, &games, None, opts);
        assert!(!list.iter().any(|p| p.ends_with("fbneo") || p.ends_with("mame2003-plus")));
        let r = remove(&list, true, &Progress::none());
        assert!(r.failed.is_empty());
        let left: Vec<String> = fs::read_dir(&system.rom_dir).unwrap().flatten().map(|e| name_of(&e.path())).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        assert_eq!(left, ["fbneo", "mame2003-plus"]);
    }

    #[test]
    fn with_emulators_the_folder_is_emptied_but_stays() {
        let tmp = tempfile::tempdir().unwrap();
        let (layout, system, games) = classic(tmp.path());
        let opts = DeleteOptions { permanent: true, emulators: true, ..Default::default() };
        let list = targets(&layout, &system, &games, None, opts);
        assert!(!list.contains(&system.rom_dir), "the system folder itself is never a target");
        remove(&list, true, &Progress::none());
        assert!(system.rom_dir.is_dir() && fs::read_dir(&system.rom_dir).unwrap().next().is_none(), "the system folder stays, empty");
        assert!(tmp.path().join("ROMs").is_dir(), "the ROM folder itself stays");
    }

    #[test]
    fn es_de_takes_its_media_and_gamelist_and_only_its_collection_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("ES-DE");
        let layout = Layout::Esde(EsdePaths { home: home.clone(), media_root: home.join("downloaded_media") });
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        touch(&system.rom_dir.join("Mario.sfc"));
        touch(&home.join("downloaded_media/snes/covers/Mario.png"));
        touch(&home.join("downloaded_media/nes/covers/Zelda.png"));
        touch(&home.join("gamelists/snes/gamelist.xml"));
        touch(&home.join("gamelists/nes/gamelist.xml"));
        fs::create_dir_all(home.join("collections")).unwrap();
        fs::write(home.join("collections/custom-Fav.cfg"), "%ROMPATH%/snes/Mario.sfc\r\n%ROMPATH%/nes/Zelda.nes\r\n").unwrap();

        let games = vec![game("./Mario.sfc")];
        let plan = plan(&layout, &system, &games, None);
        assert_eq!((plan.media_files, plan.collection_lines), (1, 1));
        let list = targets(&layout, &system, &games, None, DeleteOptions { permanent: true, ..Default::default() });
        remove(&list, true, &Progress::none());
        clean_collections(&layout, &system);
        let empty = |d: &Path| d.is_dir() && fs::read_dir(d).unwrap().next().is_none();
        assert!(empty(&home.join("downloaded_media/snes")) && empty(&home.join("gamelists/snes")) && empty(&system.rom_dir));
        assert!(home.join("downloaded_media/nes/covers/Zelda.png").exists() && home.join("gamelists/nes/gamelist.xml").exists());
        assert_eq!(fs::read_to_string(home.join("collections/custom-Fav.cfg")).unwrap(), "%ROMPATH%/nes/Zelda.nes\r\n");
    }

    #[test]
    fn nothing_outside_the_system_is_allowed() {
        let tmp = tempfile::tempdir().unwrap();
        let (layout, system, _) = classic(tmp.path());
        let roots = allowed_roots(&layout, &system, None);
        assert!(is_allowed(&system.rom_dir.join("covers"), &roots, &layout, None));
        assert!(!is_allowed(&tmp.path().join("ROMs"), &roots, &layout, None));
        assert!(!is_allowed(&tmp.path().join("ROMs/nes"), &roots, &layout, None));
    }
}
