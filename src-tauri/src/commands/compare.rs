//! «Comparar con otra carpeta»: compara la biblioteca del perfil con otra carpeta que tiene la
//! misma estructura (otra copia, la SD de un dispositivo, un disco de copia), sistema a sistema:
//! lo que solo está aquí, lo que solo está allí y lo que está en los dos sitios pero no es igual.
//! Después copia los juegos elegidos en un sentido u otro, con su media y su entrada de gamelist.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::layout::{media_stem_for, strip_dot_slash, Layout, SystemRef};
use crate::media::{self, MediaIndex, MediaKind};
use crate::models::{Game, GameList};
use crate::rom_hash::{self, CrcCache};

use super::export_device::{dest_layout, rom_files};

/// Un lado de la comparación: lo que ocupan los ficheros del juego y la fecha del más nuevo
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SideInfo {
    pub size: u64,
    pub modified: u64,
    pub files: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompareGame {
    /// Ruta de la ROM tal como está en la gamelist (la del lado donde está)
    pub path: String,
    pub name: String,
    pub here: Option<SideInfo>,
    pub there: Option<SideInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemCompare {
    /// Nombre de la carpeta del sistema
    pub system: String,
    pub only_here: Vec<CompareGame>,
    pub only_there: Vec<CompareGame>,
    pub different: Vec<CompareGame>,
    /// Juegos iguales en los dos sitios (solo se cuentan)
    pub same: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompareReport {
    pub systems: Vec<SystemCompare>,
}

#[derive(Debug, Clone, Serialize)]
struct CompareProgress {
    current: usize,
    total: usize,
    bytes: u64,
    total_bytes: u64,
    file: String,
}

static CANCEL: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn cancel_compare() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Las dos raíces: (estructura y carpeta de ROMs de la biblioteca, las mismas de la otra carpeta)
fn roots(app: &tauri::AppHandle, dest: &str) -> Result<((Layout, PathBuf), (Layout, PathBuf)), String> {
    let settings = super::settings::get_settings(app.clone())?;
    let profile = settings.active_profile().ok_or("No active profile")?;
    let layout = Layout::for_profile(profile)?;
    let here_root = PathBuf::from(&profile.rom_root_path);
    let dest_dir = PathBuf::from(dest);
    if dest.trim().is_empty() || !dest_dir.is_dir() {
        return Err(format!("Folder not found: {}", dest));
    }
    let (there_layout, there_root) = dest_layout(&layout, &dest_dir);
    if there_root == here_root {
        return Err("The folder is the library itself".into());
    }
    Ok(((layout, here_root), (there_layout, there_root)))
}

/// Carpetas de sistema de una raíz (sin las ocultas ni las propias de ES-DE)
fn system_names(layout: &Layout, root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else { return Vec::new() };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !layout.is_own_folder(p))
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .filter(|n| !n.starts_with('.'))
        .collect()
}

/// Un juego de un lado: su entrada y sus ficheros (absolutos), ya comprobado que la ROM existe
struct Entry<'a> {
    game: &'a Game,
    files: Vec<PathBuf>,
}

/// Los juegos de una gamelist cuya ROM existe, por ruta sin «./»
fn entries<'a>(system: &SystemRef, list: &'a GameList) -> HashMap<String, Entry<'a>> {
    let mut out = HashMap::new();
    for game in &list.games {
        let rel = strip_dot_slash(&game.path).to_string();
        let rom = system.rom_dir.join(&rel);
        if rom.exists() {
            out.insert(rel, Entry { game, files: rom_files(&rom) });
        }
    }
    out
}

fn side_info(files: &[PathBuf]) -> SideInfo {
    let mut info = SideInfo { size: 0, modified: 0, files: files.len() };
    for file in files {
        let Ok(meta) = fs::metadata(file) else { continue };
        info.size += meta.len();
        let secs = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
        info.modified = info.modified.max(secs);
    }
    info
}

/// Ficheros del juego por ruta relativa a la carpeta del sistema, con su tamaño
fn by_rel(root: &Path, files: &[PathBuf]) -> BTreeMap<PathBuf, (PathBuf, u64)> {
    files
        .iter()
        .filter_map(|f| {
            let rel = f.strip_prefix(root).ok()?.to_path_buf();
            Some((rel, (f.clone(), fs::metadata(f).map(|m| m.len()).unwrap_or(0))))
        })
        .collect()
}

/// Huella de un fichero: la de la caché (dentro del zip/7z si es de un solo fichero) o, si no
/// tiene una útil (comprimido de varios ficheros), la del fichero entero
fn crc_of(cache: &mut CrcCache, file: &Path) -> Option<String> {
    cache.crc(file).or_else(|| rom_hash::file_crc(file).ok())
}

/// ¿Es distinto el juego en los dos sitios? Otros ficheros u otro tamaño; con `crc`, también si
/// algún fichero del mismo tamaño tiene otra huella.
fn differs(here_root: &Path, here: &[PathBuf], there_root: &Path, there: &[PathBuf], crc: Option<&mut CrcCache>) -> bool {
    let a = by_rel(here_root, here);
    let b = by_rel(there_root, there);
    if a.len() != b.len() || a.iter().any(|(rel, (_, size))| b.get(rel).map(|(_, s)| s) != Some(size)) {
        return true;
    }
    let Some(cache) = crc else { return false };
    a.iter().any(|(rel, (file, _))| {
        let other = &b[rel].0;
        crc_of(cache, file) != crc_of(cache, other)
    })
}

fn compare_game(path: &str, game: &Game, here: Option<SideInfo>, there: Option<SideInfo>) -> CompareGame {
    CompareGame { path: path.to_string(), name: game.name.clone().unwrap_or_else(|| path.to_string()), here, there }
}

/// Compara un sistema. `on_game` se llama antes de mirar cada juego que está en los dos sitios.
fn compare_system(
    here_layout: &Layout,
    here: &SystemRef,
    there_layout: &Layout,
    there: &SystemRef,
    mut crc: Option<&mut CrcCache>,
    on_game: &mut dyn FnMut(&str),
) -> Result<SystemCompare, String> {
    let here_list = super::gamelist::read_gamelist(here_layout, here)?;
    let there_list = super::gamelist::read_gamelist(there_layout, there)?;
    let mine = entries(here, &here_list);
    let theirs = entries(there, &there_list);

    let mut out = SystemCompare { system: here.name.clone(), only_here: Vec::new(), only_there: Vec::new(), different: Vec::new(), same: 0 };
    for (rel, entry) in &mine {
        let game_path = entry.game.path.as_str();
        match theirs.get(rel) {
            None => out.only_here.push(compare_game(game_path, entry.game, Some(side_info(&entry.files)), None)),
            Some(other) => {
                on_game(game_path);
                if differs(&here.rom_dir, &entry.files, &there.rom_dir, &other.files, crc.as_deref_mut()) {
                    out.different.push(compare_game(game_path, entry.game, Some(side_info(&entry.files)), Some(side_info(&other.files))));
                } else {
                    out.same += 1;
                }
            }
        }
    }
    for (rel, entry) in &theirs {
        if !mine.contains_key(rel) {
            out.only_there.push(compare_game(&entry.game.path, entry.game, None, Some(side_info(&entry.files))));
        }
    }
    for list in [&mut out.only_here, &mut out.only_there, &mut out.different] {
        list.sort_by_key(|g| g.name.to_lowercase());
    }
    Ok(out)
}

/// Compara la biblioteca con `dest`: los sistemas de los dos lados. Emite `compare-progress`
/// (por sistema y, con `check_crc`, por juego). `cancel_compare` lo para.
#[tauri::command]
pub async fn compare_folders(app: tauri::AppHandle, dest: String, check_crc: bool) -> Result<CompareReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        CANCEL.store(false, Ordering::SeqCst);
        let ((layout, here_root), (there_layout, there_root)) = roots(&app, &dest)?;
        let mut names: Vec<String> = system_names(&layout, &here_root);
        for name in system_names(&there_layout, &there_root) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names.sort();

        let mut cache = if check_crc { Some(CrcCache::load(&rom_hash::cache_file(&app)?)) } else { None };
        let total = names.len();
        let mut systems = Vec::new();
        for (i, name) in names.iter().enumerate() {
            if CANCEL.load(Ordering::SeqCst) {
                break;
            }
            let here = SystemRef::from_rom_dir(here_root.join(name));
            let there = SystemRef::from_rom_dir(there_root.join(name));
            // Un aviso por sistema y, si se mira el contenido (tarda), otro por juego
            let mut on_game = |game: &str| {
                if game.is_empty() || check_crc {
                    let file = if game.is_empty() { name.clone() } else { format!("{} / {}", name, game) };
                    let _ = app.emit("compare-progress", CompareProgress { current: i + 1, total, bytes: 0, total_bytes: 0, file });
                }
            };
            on_game("");
            let report = compare_system(&layout, &here, &there_layout, &there, cache.as_mut(), &mut on_game)?;
            let empty = report.only_here.is_empty() && report.only_there.is_empty() && report.different.is_empty() && report.same == 0;
            if !empty {
                systems.push(report);
            }
        }
        if let Some(cache) = cache.as_mut() {
            cache.save()?;
        }
        Ok(CompareReport { systems })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- Aplicar ----------

/// Un juego que se copia: de la biblioteca a la otra carpeta («there») o al revés («here»)
#[derive(Debug, Clone, Deserialize)]
pub struct CompareAction {
    pub system: String,
    pub game_path: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CompareResult {
    pub to_there: usize,
    pub to_here: usize,
    /// Ficheros sustituidos (a la papelera en la biblioteca, borrados en la otra carpeta)
    pub replaced: usize,
    pub failed: Vec<String>,
    pub stopped: bool,
}

/// Un lado de la copia, con su gamelist cargada (la del destino se va cambiando)
struct Side {
    layout: Layout,
    system: SystemRef,
    list: GameList,
    index: MediaIndex,
    /// Es la biblioteca: lo que se sustituye va a la papelera
    is_library: bool,
}

impl Side {
    fn load(layout: &Layout, system: SystemRef, is_library: bool) -> Result<Self, String> {
        let list = super::gamelist::read_gamelist(layout, &system)?;
        let index = MediaIndex::build(layout, &system);
        Ok(Side { layout: layout.clone(), system, list, index, is_library })
    }

    fn find(&self, game_path: &str) -> Option<usize> {
        let rel = strip_dot_slash(game_path);
        self.list.games.iter().position(|g| strip_dot_slash(&g.path) == rel)
    }

    /// Tipos de media que tiene esta estructura
    fn kinds(&self) -> impl Iterator<Item = &'static MediaKind> + '_ {
        media::MEDIA_KINDS.iter().filter(|k| self.layout.writes_media_tags() || k.in_esde)
    }

    fn media_of(&self, game: &Game, kind: &MediaKind) -> Option<PathBuf> {
        super::media_kinds::current_file(&self.layout, &self.system, &self.index, game, kind)
    }
}

/// Lo que se hará con un juego
struct GamePlan {
    /// (origen, destino, tamaño)
    copies: Vec<(PathBuf, PathBuf, u64)>,
    /// Ficheros del destino que se quitan antes de copiar (los que se pisan y los que sobran del
    /// juego viejo: otra extensión de media, un .bin que ya no está…)
    replaced: Vec<PathBuf>,
    /// La entrada que queda en la gamelist del destino
    entry: Game,
}

fn plan_game(src: &Side, dst: &Side, game: &Game) -> GamePlan {
    let rom = src.system.rom_dir.join(strip_dot_slash(&game.path));
    let is_dir = rom.is_dir();
    let mut copies = Vec::new();
    for file in rom_files(&rom) {
        if let Ok(rel) = file.strip_prefix(&src.system.rom_dir) {
            copies.push((file.clone(), dst.system.rom_dir.join(rel)));
        }
    }

    let mut entry = game.clone();
    media::clear_media_fields(&mut entry);
    let stem = media_stem_for(&game.path, is_dir);
    for kind in dst.kinds() {
        let Some(file) = src.media_of(game, kind) else { continue };
        let ext = media::extension_lower(&file).unwrap_or_else(|| "png".into());
        let target = dst.layout.media_file_for_stem(&dst.system, kind.folder, &stem, &ext);
        if dst.layout.writes_media_tags() {
            media::set_field(&mut entry, kind.field, Some(media::media_ref(&dst.layout, &dst.system, &target)));
        }
        copies.push((file, target));
    }

    // Lo que ya había en el destino para este juego y no se va a pisar con lo mismo
    let targets: HashSet<&PathBuf> = copies.iter().map(|(_, t)| t).collect();
    let mut replaced: Vec<PathBuf> = copies.iter().map(|(_, t)| t.clone()).filter(|t| t.is_file()).collect();
    if let Some(old) = dst.find(&game.path).map(|i| &dst.list.games[i]) {
        let old_rom = dst.system.rom_dir.join(strip_dot_slash(&old.path));
        let mut old_files = rom_files(&old_rom);
        old_files.extend(dst.kinds().filter_map(|k| dst.media_of(old, k)));
        for file in old_files {
            if file.is_file() && !targets.contains(&file) && !replaced.contains(&file) {
                replaced.push(file);
            }
        }
    }

    let copies = copies.into_iter().map(|(from, to)| {
        let size = fs::metadata(&from).map(|m| m.len()).unwrap_or(0);
        (from, to, size)
    });
    GamePlan { copies: copies.collect(), replaced, entry }
}

/// Quita lo sustituido: a la papelera en la biblioteca; borrado en la otra carpeta
fn remove_replaced(dst: &Side, files: &[PathBuf]) -> Result<(), String> {
    if dst.is_library {
        if let Some((file, e)) = media::trash_files(files).into_iter().next() {
            return Err(format!("{} ({})", file.display(), e));
        }
        return Ok(());
    }
    for file in files {
        fs::remove_file(file).map_err(|e| format!("{} ({})", file.display(), e))?;
    }
    Ok(())
}

/// Copia los ficheros de un juego. Si uno falla, quita los que ya se copiaron de ese juego.
fn copy_game(plan: &GamePlan, on_file: &mut dyn FnMut(&Path, u64)) -> Result<(), String> {
    let mut done: Vec<&PathBuf> = Vec::new();
    for (from, to, size) in &plan.copies {
        on_file(to, *size);
        let copied = to.parent().map_or(Ok(()), fs::create_dir_all).and_then(|_| fs::copy(from, to).map(|_| ()));
        if let Err(e) = copied {
            for file in done {
                let _ = fs::remove_file(file);
            }
            return Err(format!("{} ({})", to.display(), e));
        }
        done.push(to);
    }
    Ok(())
}

/// Pone la entrada en la gamelist del destino (sustituye la que tuviera la misma ruta)
fn put_entry(dst: &mut Side, entry: Game) {
    match dst.find(&entry.path) {
        Some(i) => dst.list.games[i] = entry,
        None => dst.list.games.push(entry),
    }
}

fn write_side(side: &Side) -> Result<(), String> {
    if side.is_library {
        super::gamelist::write_gamelist(&side.layout, &side.system, &side.list)
    } else {
        super::gamelist::write_gamelist_no_backup(&side.layout, &side.system, &side.list)
    }
}

/// Copia los juegos elegidos. Emite `compare-progress` fichero a fichero; `cancel_compare` lo
/// para entre juego y juego. Cada gamelist que cambia se escribe una vez (la de la biblioteca,
/// con copia de seguridad).
#[tauri::command]
pub async fn apply_compare(app: tauri::AppHandle, dest: String, actions: Vec<CompareAction>) -> Result<CompareResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        CANCEL.store(false, Ordering::SeqCst);
        let ((layout, here_root), (there_layout, there_root)) = roots(&app, &dest)?;

        // Por sistema y sentido: una carga y una escritura de cada gamelist
        let mut groups: BTreeMap<(String, bool), Vec<String>> = BTreeMap::new();
        for a in actions {
            groups.entry((a.system, a.to == "here")).or_default().push(a.game_path);
        }

        let mut work = Vec::new();
        for ((system, to_here), paths) in groups {
            let here = Side::load(&layout, SystemRef::from_rom_dir(here_root.join(&system)), true)?;
            let there = Side::load(&there_layout, SystemRef::from_rom_dir(there_root.join(&system)), false)?;
            let (src, dst) = if to_here { (there, here) } else { (here, there) };
            let plans: Vec<(String, GamePlan)> = paths
                .iter()
                .filter_map(|p| src.find(p).map(|i| (p.clone(), plan_game(&src, &dst, &src.list.games[i]))))
                .collect();
            work.push((to_here, dst, plans));
        }

        let total: usize = work.iter().flat_map(|(_, _, plans)| plans.iter()).map(|(_, p)| p.copies.len()).sum();
        let total_bytes: u64 = work.iter().flat_map(|(_, _, plans)| plans.iter()).flat_map(|(_, p)| p.copies.iter()).map(|(_, _, s)| s).sum();
        let (mut current, mut bytes) = (0usize, 0u64);
        let mut result = CompareResult::default();

        'groups: for (to_here, dst, plans) in &mut work {
            let mut changed = false;
            for (game_path, plan) in plans.iter() {
                if CANCEL.load(Ordering::SeqCst) {
                    result.stopped = true;
                    if changed {
                        write_side(dst).unwrap_or_else(|e| result.failed.push(e));
                    }
                    break 'groups;
                }
                if let Err(e) = remove_replaced(dst, &plan.replaced) {
                    result.failed.push(format!("{}: {}", game_path, e));
                    continue;
                }
                result.replaced += plan.replaced.len();
                let mut on_file = |to: &Path, size: u64| {
                    current += 1;
                    let file = to.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                    let _ = app.emit("compare-progress", CompareProgress { current, total, bytes, total_bytes, file });
                    bytes += size;
                };
                match copy_game(plan, &mut on_file) {
                    Ok(()) => {
                        put_entry(dst, plan.entry.clone());
                        changed = true;
                        if *to_here {
                            result.to_here += 1;
                        } else {
                            result.to_there += 1;
                        }
                    }
                    Err(e) => result.failed.push(format!("{}: {}", game_path, e)),
                }
            }
            if changed {
                write_side(dst).unwrap_or_else(|e| result.failed.push(e));
            }
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::EsdePaths;

    fn touch(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn write_list(layout: &Layout, system: &SystemRef, games: Vec<Game>) {
        super::super::gamelist::write_gamelist_no_backup(layout, system, &GameList { games, ..Default::default() }).unwrap();
    }

    fn game(path: &str) -> Game {
        Game { path: path.into(), name: Some(path.trim_start_matches("./").into()), ..Default::default() }
    }

    fn paths(list: &[CompareGame]) -> Vec<&str> {
        list.iter().map(|g| g.path.as_str()).collect()
    }

    #[test]
    fn games_are_matched_by_path_and_sorted_into_the_three_lists() {
        let tmp = tempfile::tempdir().unwrap();
        let here = SystemRef::from_rom_dir(tmp.path().join("lib/snes"));
        let there = SystemRef::from_rom_dir(tmp.path().join("sd/snes"));
        touch(&here.rom_dir.join("Zelda.sfc"), "rom");
        touch(&here.rom_dir.join("Mario.sfc"), "mario");
        touch(&here.rom_dir.join("Solo.sfc"), "x");
        touch(&there.rom_dir.join("Zelda.sfc"), "rom");
        touch(&there.rom_dir.join("Mario.sfc"), "mario v2");
        touch(&there.rom_dir.join("Alli.sfc"), "y");
        write_list(&Layout::Classic, &here, vec![game("./Zelda.sfc"), game("./Mario.sfc"), game("./Solo.sfc"), game("./SinRom.sfc")]);
        // Sin «./» en la otra carpeta: es el mismo juego
        write_list(&Layout::Classic, &there, vec![game("Zelda.sfc"), game("./Mario.sfc"), game("./Alli.sfc")]);

        let r = compare_system(&Layout::Classic, &here, &Layout::Classic, &there, None, &mut |_| {}).unwrap();
        assert_eq!(paths(&r.only_here), vec!["./Solo.sfc"], "a game without its ROM doesn't count");
        assert_eq!(paths(&r.only_there), vec!["./Alli.sfc"]);
        assert_eq!(paths(&r.different), vec!["./Mario.sfc"]);
        assert_eq!(r.same, 1);
        assert_eq!(r.different[0].there.as_ref().unwrap().size, 8);
    }

    #[test]
    fn a_cue_counts_with_its_bins_and_crc_finds_same_size_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let here = SystemRef::from_rom_dir(tmp.path().join("lib/psx"));
        let there = SystemRef::from_rom_dir(tmp.path().join("sd/psx"));
        for (sys, bin) in [(&here, "aaaa"), (&there, "bbbb")] {
            touch(&sys.rom_dir.join("Juego.cue"), "FILE \"Juego (Track 1).bin\" BINARY\n");
            touch(&sys.rom_dir.join("Juego (Track 1).bin"), bin);
            write_list(&Layout::Classic, sys, vec![game("./Juego.cue")]);
        }
        let r = compare_system(&Layout::Classic, &here, &Layout::Classic, &there, None, &mut |_| {}).unwrap();
        assert_eq!(r.same, 1, "same sizes: equal without the content check");
        assert_eq!(r.different.len(), 0);

        let mut cache = CrcCache::default();
        let r = compare_system(&Layout::Classic, &here, &Layout::Classic, &there, Some(&mut cache), &mut |_| {}).unwrap();
        assert_eq!(paths(&r.different), vec!["./Juego.cue"]);
        assert_eq!(r.different[0].here.as_ref().unwrap().files, 2);

        // Falta un .bin en un lado: distinto aunque no se mire el contenido
        fs::remove_file(there.rom_dir.join("Juego (Track 1).bin")).unwrap();
        let r = compare_system(&Layout::Classic, &here, &Layout::Classic, &there, None, &mut |_| {}).unwrap();
        assert_eq!(r.different.len(), 1);
    }

    #[test]
    fn copying_there_brings_files_media_and_the_gamelist_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let here_sys = SystemRef::from_rom_dir(tmp.path().join("lib/nes"));
        let there_sys = SystemRef::from_rom_dir(tmp.path().join("sd/nes"));
        touch(&here_sys.rom_dir.join("A.nes"), "a");
        touch(&here_sys.rom_dir.join("covers/A.png"), "img");
        let mut a = game("./A.nes");
        a.image = Some("./covers/A.png".into());
        a.favorite = Some("true".into());
        write_list(&Layout::Classic, &here_sys, vec![a]);
        write_list(&Layout::Classic, &there_sys, vec![game("./B.nes")]);
        touch(&there_sys.rom_dir.join("B.nes"), "b");

        let src = Side::load(&Layout::Classic, here_sys.clone(), true).unwrap();
        let mut dst = Side::load(&Layout::Classic, there_sys.clone(), false).unwrap();
        let plan = plan_game(&src, &dst, &src.list.games[0]);
        assert!(plan.replaced.is_empty());
        copy_game(&plan, &mut |_, _| {}).unwrap();
        put_entry(&mut dst, plan.entry);
        write_side(&dst).unwrap();

        assert_eq!(fs::read_to_string(there_sys.rom_dir.join("A.nes")).unwrap(), "a");
        assert_eq!(fs::read_to_string(there_sys.rom_dir.join("covers/A.png")).unwrap(), "img");
        let list = super::super::gamelist::read_gamelist(&Layout::Classic, &there_sys).unwrap();
        assert_eq!(list.games.len(), 2);
        let copied = list.games.iter().find(|g| g.path == "./A.nes").unwrap();
        assert_eq!(copied.image.as_deref(), Some("./covers/A.png"));
        assert_eq!(copied.favorite.as_deref(), Some("true"));
    }

    #[test]
    fn replacing_in_the_library_removes_old_files_of_the_game_esde() {
        let tmp = tempfile::tempdir().unwrap();
        let esde = |root: &str| Layout::Esde(EsdePaths { home: tmp.path().join(root).join("ES-DE"), media_root: tmp.path().join(root).join("ES-DE/downloaded_media") });
        let (here_layout, there_layout) = (esde("lib"), esde("sd"));
        let here_sys = SystemRef::from_rom_dir(tmp.path().join("lib/ROMs/gb"));
        let there_sys = SystemRef::from_rom_dir(tmp.path().join("sd/ROMs/gb"));
        touch(&here_sys.rom_dir.join("T.gb"), "old");
        touch(&tmp.path().join("lib/ES-DE/downloaded_media/gb/covers/T.jpg"), "old cover");
        touch(&there_sys.rom_dir.join("T.gb"), "new rom");
        touch(&tmp.path().join("sd/ES-DE/downloaded_media/gb/covers/T.png"), "new cover");
        let mut t = game("./T.gb");
        t.playcount = Some("7".into());
        write_list(&here_layout, &here_sys, vec![game("./T.gb")]);
        write_list(&there_layout, &there_sys, vec![t]);

        let src = Side::load(&there_layout, there_sys, false).unwrap();
        let dst = Side::load(&here_layout, here_sys.clone(), true).unwrap();
        let plan = plan_game(&src, &dst, &src.list.games[0]);
        let mut replaced = plan.replaced.clone();
        replaced.sort();
        let mut expected = vec![here_sys.rom_dir.join("T.gb"), tmp.path().join("lib/ES-DE/downloaded_media/gb/covers/T.jpg")];
        expected.sort();
        assert_eq!(replaced, expected, "the old ROM and the cover with the other extension");
        assert_eq!(plan.entry.playcount.as_deref(), Some("7"));
        assert_eq!(plan.entry.image, None, "ES-DE: no media paths in the entry");
        let targets: Vec<&PathBuf> = plan.copies.iter().map(|(_, t, _)| t).collect();
        assert!(targets.contains(&&tmp.path().join("lib/ES-DE/downloaded_media/gb/covers/T.png")));
    }

    #[test]
    fn a_failed_copy_leaves_nothing_of_that_game() {
        let tmp = tempfile::tempdir().unwrap();
        let ok = tmp.path().join("src/a.bin");
        touch(&ok, "a");
        let plan = GamePlan {
            copies: vec![(ok, tmp.path().join("dst/a.bin"), 1), (tmp.path().join("src/missing.bin"), tmp.path().join("dst/b.bin"), 1)],
            replaced: Vec::new(),
            entry: Game::default(),
        };
        assert!(copy_game(&plan, &mut |_, _| {}).is_err());
        assert!(!tmp.path().join("dst/a.bin").exists());
    }
}
