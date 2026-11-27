//! «Importar ROMs»: trae a un sistema las ROMs de otra carpeta, con su estructura que sea:
//! - **clásica**: gamelist y medios dentro de la carpeta (también con nombres antiguos);
//! - **ES-DE**: gamelist y medios en una carpeta de ES-DE (junto al origen, por encima, dentro,
//!   o la que se elija a mano);
//! - **ROMs sueltas**: sin gamelist (el título sale del nombre con las reglas de limpieza).
//!
//! Primero una vista previa juego a juego (`plan_import`): nuevo, la misma ROM que ya hay (por
//! nombre o por contenido), otra versión de un juego que ya hay (lo de Versiones y duplicados) o
//! el nombre ocupado por otra ROM. Después `import_roms` copia o mueve los elegidos: ROM y
//! acompañantes (conservando subcarpetas), medios (todos, los obligatorios o ninguno) con el
//! nombrado unificado del perfil, y la entrada de la gamelist. Nunca se sobrescribe nada; si
//! algo falla en un juego, solo ese juego se deshace. La gamelist se escribe una vez (con
//! copia) y la operación entera se puede deshacer.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::{esde_settings, media_stem_for, strip_dot_slash, EsdePaths, Layout, SystemRef};
use crate::media::{self, MediaIndex, MediaKind, MEDIA_KINDS};
use crate::models::Game;
use crate::rename_rules::CompiledRules;

use super::migrate::MigrationMode;
use super::progress::Progress;

/// Cómo está el origen
#[derive(Debug, Clone, Serialize)]
pub struct ImportSource {
    /// classic | esde | loose
    pub kind: String,
    pub folder: String,
    /// ES-DE: su carpeta de datos (encontrada o elegida)
    pub esde_home: Option<String>,
    /// La gamelist que se lee (si la hay)
    pub gamelist: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportGame {
    /// Ruta en el origen («./sub/Juego.zip»): es también la que tendrá en el sistema
    pub path: String,
    pub name: String,
    /// new | same_rom | other_version | taken
    pub status: String,
    /// El juego tuyo con el que coincide (misma ROM u otra versión)
    pub other: Option<String>,
    /// Misma ROM: la ruta de tu juego en la gamelist (el que se completa)
    pub mine: Option<String>,
    /// Misma ROM: cuántos datos vacíos de tu juego rellenaría
    pub fills: usize,
    /// Campos de los medios que trae (misma ROM: solo los que a tu juego le faltan)
    pub media: Vec<String>,
    /// Tamaño de la ROM (con acompañantes)
    pub bytes: u64,
    /// false = la extensión no es de este sistema (sale desmarcada)
    pub known_extension: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportPlan {
    pub source: ImportSource,
    pub games: Vec<ImportGame>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportOptions {
    pub mode: MigrationMode,
    /// all | required | none
    pub media: String,
    /// Con `required`: los campos de medios obligatorios de «Mi colección»
    #[serde(default)]
    pub required: Vec<String>,
}

/// Un juego que ya tienes (misma ROM) y se completa con lo del origen en vez de importarse
#[derive(Debug, Clone, Deserialize)]
pub struct CompleteRequest {
    /// Ruta en el origen
    pub path: String,
    /// Ruta de tu juego en la gamelist
    pub mine: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportResult {
    /// Rutas de los juegos importados (para «Ponerlo todo en orden» con ellos)
    pub imported: Vec<String>,
    /// Rutas de tus juegos que se completaron (datos o medios)
    pub completed: Vec<String>,
    /// Juegos que no se pudieron importar, con el motivo
    pub skipped: Vec<String>,
    pub media: usize,
    pub stopped: bool,
}

/// El origen montado: su layout, su «sistema» y qué tipo es
struct Source {
    layout: Layout,
    system: SystemRef,
    info: ImportSource,
}

#[tauri::command(async)]
pub fn plan_import(app: tauri::AppHandle, system_path: String, folder: String, esde_home: Option<String>, task: Option<String>) -> Result<ImportPlan, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    let source = detect(Path::new(&folder), esde_home.as_deref().map(Path::new))?;
    check_not_inside(&source, &system)?;
    let rules = CompiledRules::new(&settings.rename_rules_or_default());
    let src_games = source_games(&source, &rules, &system);
    progress.check()?;
    let dst_games = super::gamelist::read_gamelist(&layout, &system).map(|g| g.games).unwrap_or_default();
    let mut crc = crate::rom_hash::cache_file(&app).map(|f| crate::rom_hash::CrcCache::load(&f)).ok();
    let dat = |combined: &[Game]| super::dat::version_groups(&app, &system, combined).unwrap_or_default();
    let games = classify(&source, &layout, &system, &src_games, &dst_games, crc.as_mut(), &dat, &progress)?;
    if let Some(c) = crc.as_mut() {
        let _ = c.save();
    }
    Ok(ImportPlan { source: source.info, games })
}

#[tauri::command(async)]
pub fn import_roms(
    app: tauri::AppHandle,
    system_path: String,
    folder: String,
    esde_home: Option<String>,
    paths: Vec<String>,
    complete: Vec<CompleteRequest>,
    options: ImportOptions,
    task: Option<String>,
) -> Result<ImportResult, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    let source = detect(Path::new(&folder), esde_home.as_deref().map(Path::new))?;
    check_not_inside(&source, &system)?;
    let rules = CompiledRules::new(&settings.rename_rules_or_default());
    let wanted: HashSet<&str> = paths.iter().map(String::as_str).collect();
    let all = source_games(&source, &rules, &system);
    let games: Vec<Game> = all.iter().filter(|(g, _)| wanted.contains(g.path.as_str())).map(|(g, _)| g.clone()).collect();
    // Los que ya tienes: el juego del origen con la ruta de tu juego
    let by_path: HashMap<&str, &Game> = all.iter().map(|(g, _)| (strip_dot_slash(&g.path), g)).collect();
    let to_complete: Vec<(Game, String)> =
        complete.iter().filter_map(|c| by_path.get(strip_dot_slash(&c.path)).map(|g| ((*g).clone(), c.mine.clone()))).collect();

    let mut gamelist = super::gamelist::read_gamelist(&layout, &system).unwrap_or_default();
    media::journal_begin();
    let (completed, merged) = complete_games(&source, &layout, &system, &mut gamelist.games, &to_complete, &options, &progress);
    let mut result = if progress.cancelled() {
        ImportResult { stopped: true, ..Default::default() }
    } else {
        import(&source, &layout, &system, &mut gamelist.games, &games, &options, &progress)
    };
    result.media += completed.media;
    result.completed = completed.completed;
    result.skipped.extend(completed.skipped);
    result.stopped |= completed.stopped;
    let files = media::journal_take();
    if !result.imported.is_empty() || !result.completed.is_empty() {
        // write_gamelist() hace la copia de seguridad antes de escribir
        super::gamelist::write_gamelist(&layout, &system, &gamelist)?;
        let sys = system.rom_dir.to_string_lossy().to_string();
        super::undo::record(super::undo::UndoOp {
            kind: "import".into(),
            system: sys.clone(),
            files,
            added: result.imported.iter().map(|p| (sys.clone(), p.clone())).collect(),
            // Los completados: Deshacer pone la entrada como estaba
            merged: merged.into_iter().map(|(after, before)| (sys.clone(), after, vec![before])).collect(),
            ..Default::default()
        });
    }
    Ok(result)
}

/// Qué estructura tiene la carpeta elegida
fn detect(folder: &Path, esde_override: Option<&Path>) -> Result<Source, String> {
    if !folder.is_dir() {
        return Err(format!("Folder not found: {}", folder.display()));
    }
    let system = SystemRef::from_rom_dir(folder);
    let info = |kind: &str, home: Option<&Path>, gamelist: Option<PathBuf>| ImportSource {
        kind: kind.into(),
        folder: folder.to_string_lossy().to_string(),
        esde_home: home.map(|h| h.to_string_lossy().to_string()),
        gamelist: gamelist.map(|g| g.to_string_lossy().to_string()),
    };
    // Una carpeta de ES-DE elegida a mano manda
    if let Some(home) = esde_override {
        let layout = esde_layout(home)?;
        let gamelist = Some(layout.gamelist_path(&system)).filter(|g| g.is_file());
        let home = esde_settings::data_folder(home);
        return Ok(Source { layout, system, info: info("esde", Some(&home), gamelist) });
    }
    if Layout::Classic.gamelist_path(&system).is_file() {
        let gamelist = Layout::Classic.gamelist_path(&system);
        return Ok(Source { layout: Layout::Classic, system, info: info("classic", None, Some(gamelist)) });
    }
    for home in esde_candidates(folder) {
        let Ok(layout) = esde_layout(&home) else { continue };
        let has_data = layout.gamelist_path(&system).is_file() || layout.media_type_dir(&system, "").is_dir();
        if has_data {
            let gamelist = Some(layout.gamelist_path(&system)).filter(|g| g.is_file());
            let home = esde_settings::data_folder(&home);
            return Ok(Source { layout, system, info: info("esde", Some(&home), gamelist) });
        }
    }
    Ok(Source { layout: Layout::Classic, system, info: info("loose", None, None) })
}

/// Dónde puede estar la carpeta de ES-DE de un origen: junto a él, por encima, dentro
fn esde_candidates(folder: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dir = folder.parent();
    // <raíz>/snes con <raíz>/ES-DE, o <raíz>/ROMs/snes con <raíz>/ES-DE
    for _ in 0..2 {
        let Some(d) = dir else { break };
        out.push(d.join("ES-DE"));
        out.push(d.to_path_buf());
        dir = d.parent();
    }
    out.push(folder.join("ES-DE"));
    out.retain(|d| d.join("gamelists").is_dir() || d.join("downloaded_media").is_dir() || d.join("ES-DE").is_dir() || d.join("settings").is_dir());
    out
}

fn esde_layout(home: &Path) -> Result<Layout, String> {
    let paths = esde_settings::detect_esde_paths(home)?;
    Ok(Layout::Esde(EsdePaths { home: PathBuf::from(&paths.esde_home), media_root: PathBuf::from(&paths.media_dir) }))
}

/// No se importa una carpeta en sí misma (ni el sistema dentro de su propia carpeta)
fn check_not_inside(source: &Source, system: &SystemRef) -> Result<(), String> {
    let canon = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let (src, dst) = (canon(&source.system.rom_dir), canon(&system.rom_dir));
    if src.starts_with(&dst) || dst.starts_with(&src) {
        return Err("The source folder is the system folder (or inside it)".into());
    }
    Ok(())
}

/// Juegos del origen: los de su gamelist cuya ROM existe y, además, las ROMs que no están en
/// ella (con el título del nombre de fichero). El bool: la extensión es del sistema de destino
/// (`dest`), no del nombre de la carpeta de origen, que puede ser otro.
fn source_games(source: &Source, rules: &CompiledRules, dest: &SystemRef) -> Vec<(Game, bool)> {
    let exts = crate::models::system::extensions_for_system(&dest.name);
    let known = |path: &str| -> bool {
        let rom = source.system.rom_dir.join(strip_dot_slash(path));
        // Una ROM-carpeta vale siempre
        rom.is_dir() || media::extension_lower(&rom).is_some_and(|e| exts.contains(&format!(".{e}").as_str()))
    };
    let mut games: Vec<Game> = Vec::new();
    if source.info.gamelist.is_some() {
        if let Ok(list) = super::gamelist::read_gamelist(&source.layout, &source.system) {
            let mut seen = HashSet::new();
            for g in list.games {
                if seen.insert(g.path.clone()) && source.system.rom_dir.join(strip_dot_slash(&g.path)).exists() {
                    games.push(g);
                }
            }
        }
    }
    for rom in super::new_roms::find_new_roms(&source.system, &games, rules) {
        games.push(Game { path: rom.path, name: Some(rom.proposed_name), ..Default::default() });
    }
    games.into_iter().map(|g| {
        let k = known(&g.path);
        (g, k)
    }).collect()
}

/// Tipos de medio que tiene el destino
fn dest_kinds(layout: &Layout) -> impl Iterator<Item = &'static MediaKind> + '_ {
    MEDIA_KINDS.iter().filter(|k| layout.writes_media_tags() || k.in_esde)
}

#[allow(clippy::too_many_arguments)]
fn classify(
    source: &Source,
    layout: &Layout,
    system: &SystemRef,
    src_games: &[(Game, bool)],
    dst_games: &[Game],
    mut crc: Option<&mut crate::rom_hash::CrcCache>,
    dat: &dyn Fn(&[Game]) -> HashMap<String, String>,
    progress: &Progress,
) -> Result<Vec<ImportGame>, String> {
    let src_index = MediaIndex::build(&source.layout, &source.system);
    // ROMs del destino por tamaño (el CRC solo se mira si coincide el tamaño)
    let mut by_size: HashMap<u64, Vec<(PathBuf, usize)>> = HashMap::new();
    for (i, g) in dst_games.iter().enumerate() {
        let rom = system.rom_dir.join(strip_dot_slash(&g.path));
        if let Some(meta) = fs::metadata(&rom).ok().filter(|m| m.is_file()) {
            by_size.entry(meta.len()).or_default().push((rom, i));
        }
    }
    let dst_index = MediaIndex::build(layout, system);
    // Otras versiones: el agrupado de Versiones y duplicados sobre los tuyos + los del origen.
    // Los del origen llevan otra ruta para no confundirse con los tuyos
    let mut combined: Vec<Game> = dst_games.to_vec();
    combined.extend(src_games.iter().map(|(g, _)| Game { path: format!("./\u{0}import/{}", strip_dot_slash(&g.path)), ..g.clone() }));
    let groups = super::duplicates::group_games(&combined, &dat(&combined));
    let mut version_of: HashMap<usize, String> = HashMap::new();
    for group in groups {
        let mine = group.members.iter().find(|&&i| i < dst_games.len());
        if let Some(&mine) = mine {
            for &i in group.members.iter().filter(|&&i| i >= dst_games.len()) {
                version_of.insert(i - dst_games.len(), dst_games[mine].name.clone().unwrap_or_else(|| dst_games[mine].path.clone()));
            }
        }
    }

    let mut out = Vec::with_capacity(src_games.len());
    for (i, (game, known)) in src_games.iter().enumerate() {
        progress.check()?;
        progress.step("games", i + 1, src_games.len(), &game.path);
        let rom = source.system.rom_dir.join(strip_dot_slash(&game.path));
        let files = super::export_device::rom_files(&rom);
        let bytes: u64 = files.iter().filter_map(|f| fs::metadata(f).ok()).map(|m| m.len()).sum();
        let media: Vec<String> = dest_kinds(layout)
            .filter(|k| super::media_kinds::current_file(&source.layout, &source.system, &src_index, game, k).is_some())
            .map(|k| k.field.to_string())
            .collect();
        let target = system.rom_dir.join(strip_dot_slash(&game.path));
        let same_content = |c: &mut Option<&mut crate::rom_hash::CrcCache>, other: &Path| -> bool {
            let (Ok(a), Ok(b)) = (fs::metadata(&rom), fs::metadata(other)) else { return false };
            if !a.is_file() || a.len() != b.len() {
                return false;
            }
            match c.as_deref_mut() {
                Some(cache) => cache.crc(&rom).is_some() && cache.crc(&rom) == cache.crc(other),
                None => false,
            }
        };
        // (estado, juego tuyo con el que coincide: índice en dst_games)
        let (status, mine): (&str, Option<usize>) = if target.exists() {
            // Mismo nombre: la misma ROM si es igual (CRC), o si es tu entrada y mide lo mismo
            let mine = dst_games.iter().position(|g| strip_dot_slash(&g.path) == strip_dot_slash(&game.path));
            let same_size = fs::metadata(&rom).map(|m| m.len()).ok() == fs::metadata(&target).map(|m| m.len()).ok();
            if same_content(&mut crc, &target) || (mine.is_some() && same_size) {
                ("same_rom", mine)
            } else {
                ("taken", None)
            }
        } else {
            let size = fs::metadata(&rom).map(|m| m.len()).unwrap_or(0);
            let twin = by_size.get(&size).and_then(|list| list.iter().find(|(p, _)| same_content(&mut crc, p)).map(|(_, i)| *i));
            match twin {
                Some(t) => ("same_rom", Some(t)),
                None if version_of.contains_key(&i) => ("other_version", None),
                None => ("new", None),
            }
        };
        let other = match mine {
            Some(m) => Some(dst_games[m].name.clone().unwrap_or_else(|| dst_games[m].path.clone())),
            None => version_of.get(&i).cloned().filter(|_| status == "other_version"),
        };
        // Misma ROM: lo que aportaría a tu juego (datos vacíos y medios que le faltan)
        let (fills, media) = match mine {
            Some(m) => {
                let my_game = &dst_games[m];
                let lacking: Vec<String> = media
                    .into_iter()
                    .filter(|f| MEDIA_KINDS.iter().find(|k| k.field == f).is_some_and(|k| super::media_kinds::current_file(layout, system, &dst_index, my_game, k).is_none()))
                    .collect();
                (data_fills(system, my_game, game), lacking)
            }
            None => (0, media),
        };
        out.push(ImportGame {
            path: game.path.clone(),
            name: game.name.clone().unwrap_or_else(|| game.path.clone()),
            status: status.to_string(),
            other,
            mine: mine.map(|m| dst_games[m].path.clone()),
            fills,
            media,
            bytes,
            known_extension: *known,
        });
    }
    Ok(out)
}

/// Cuántos datos vacíos de `mine` rellenaría el juego del origen (sin los medios: van aparte)
fn data_fills(system: &SystemRef, mine: &Game, theirs: &Game) -> usize {
    let mut filled = mine.clone();
    let mut source = theirs.clone();
    media::clear_media_fields(&mut source);
    super::repeated_entries::fill_empty(system, &mut filled, &source, true);
    let (Ok(serde_json::Value::Object(a)), Ok(serde_json::Value::Object(b))) = (serde_json::to_value(mine), serde_json::to_value(&filled)) else { return 0 };
    b.iter().filter(|(k, v)| a.get(*k) != Some(*v)).count()
}

/// Lo que importa cada juego (ROM aparte): los tipos de medio elegidos
fn chosen_kinds<'a>(layout: &'a Layout, options: &'a ImportOptions) -> Vec<&'static MediaKind> {
    dest_kinds(layout)
        .filter(|k| match options.media.as_str() {
            "all" => true,
            "required" => options.required.iter().any(|f| f == k.field),
            _ => false,
        })
        .collect()
}

/// Completa tus juegos que ya están (misma ROM): rellena sus datos vacíos y les trae los medios
/// que les faltan, con el nombre de tu ROM. La ROM no se toca. Devuelve, para Deshacer, cada
/// entrada completada con cómo era antes.
fn complete_games(
    source: &Source,
    layout: &Layout,
    system: &SystemRef,
    dst_games: &mut [Game],
    games: &[(Game, String)],
    options: &ImportOptions,
    progress: &Progress,
) -> (ImportResult, Vec<(Game, Game)>) {
    let mut result = ImportResult::default();
    let mut merged = Vec::new();
    let src_index = MediaIndex::build(&source.layout, &source.system);
    let dst_index = MediaIndex::build(layout, system);
    let kinds = chosen_kinds(layout, options);
    for (i, (game, mine_path)) in games.iter().enumerate() {
        if progress.cancelled() {
            result.stopped = true;
            break;
        }
        progress.step("completing", i + 1, games.len(), &game.path);
        let Some(pos) = dst_games.iter().position(|g| strip_dot_slash(&g.path) == strip_dot_slash(mine_path)) else {
            result.skipped.push(format!("{} (not in the gamelist)", strip_dot_slash(mine_path)));
            continue;
        };
        let before = dst_games[pos].clone();
        let mut after = before.clone();
        // Datos: solo los vacíos (ids y hashes también: es la misma ROM)
        let mut theirs = game.clone();
        media::clear_media_fields(&mut theirs);
        super::repeated_entries::fill_empty(system, &mut after, &theirs, true);
        // Medios que tu juego no tiene, con el nombre de tu ROM (nunca se pisa nada)
        let my_rom = system.rom_dir.join(strip_dot_slash(&before.path));
        let stem = media_stem_for(&before.path, my_rom.is_dir());
        let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
        for kind in &kinds {
            if super::media_kinds::current_file(layout, system, &dst_index, &before, kind).is_some() {
                continue;
            }
            let Some(file) = super::media_kinds::current_file(&source.layout, &source.system, &src_index, game, kind) else { continue };
            let ext = media::extension_lower(&file).unwrap_or_else(|| "png".into());
            let target = layout.media_file_for_stem(system, kind.folder, &stem, &ext);
            if target.exists() {
                continue;
            }
            if layout.writes_media_tags() {
                media::set_field(&mut after, kind.field, Some(media::media_ref(layout, system, &target)));
            }
            moves.push((file, target));
        }
        if let Err(e) = transfer(&moves, options.mode) {
            result.skipped.push(format!("{} ({})", strip_dot_slash(&game.path), e));
            continue;
        }
        let changed = !moves.is_empty() || serde_json::to_value(&after).ok() != serde_json::to_value(&before).ok();
        if changed {
            result.media += moves.len();
            result.completed.push(after.path.clone());
            dst_games[pos] = after.clone();
            merged.push((after, before));
        }
    }
    (result, merged)
}

/// Importa los juegos: ficheros, medios y entrada. Va añadiendo a `dst_games`.
fn import(source: &Source, layout: &Layout, system: &SystemRef, dst_games: &mut Vec<Game>, games: &[Game], options: &ImportOptions, progress: &Progress) -> ImportResult {
    let mut result = ImportResult::default();
    let src_index = MediaIndex::build(&source.layout, &source.system);
    let kinds = chosen_kinds(layout, options);

    for (i, game) in games.iter().enumerate() {
        if progress.cancelled() {
            result.stopped = true;
            break;
        }
        progress.step("importing", i + 1, games.len(), &game.path);
        let rom = source.system.rom_dir.join(strip_dot_slash(&game.path));
        // ROM y acompañantes, con sus subcarpetas
        let mut moves: Vec<(PathBuf, PathBuf)> = super::export_device::rom_files(&rom)
            .into_iter()
            .filter_map(|f| f.strip_prefix(&source.system.rom_dir).ok().map(|rel| (f.clone(), system.rom_dir.join(rel))))
            .collect();
        if moves.is_empty() || moves.iter().any(|(_, to)| to.exists()) || dst_games.iter().any(|g| strip_dot_slash(&g.path) == strip_dot_slash(&game.path)) {
            result.skipped.push(format!("{} (exists)", strip_dot_slash(&game.path)));
            continue;
        }
        // Medios con el nombrado unificado del destino (si ya hay uno, no se pisa)
        let mut entry = game.clone();
        media::clear_media_fields(&mut entry);
        let stem = media_stem_for(&game.path, rom.is_dir());
        let mut media_count = 0;
        for kind in &kinds {
            let Some(file) = super::media_kinds::current_file(&source.layout, &source.system, &src_index, game, kind) else { continue };
            let ext = media::extension_lower(&file).unwrap_or_else(|| "png".into());
            let target = layout.media_file_for_stem(system, kind.folder, &stem, &ext);
            if target.exists() || moves.iter().any(|(_, t)| *t == target) {
                continue;
            }
            if layout.writes_media_tags() {
                media::set_field(&mut entry, kind.field, Some(media::media_ref(layout, system, &target)));
            }
            moves.push((file, target));
            media_count += 1;
        }
        match transfer(&moves, options.mode) {
            Ok(()) => {
                result.media += media_count;
                result.imported.push(entry.path.clone());
                dst_games.push(entry);
            }
            Err(e) => result.skipped.push(format!("{} ({})", strip_dot_slash(&game.path), e)),
        }
    }
    result
}

/// Copia o mueve los ficheros de un juego; si uno falla, deshace los de ese juego
fn transfer(files: &[(PathBuf, PathBuf)], mode: MigrationMode) -> Result<(), String> {
    let mut done: Vec<(&PathBuf, &PathBuf)> = Vec::new();
    for (from, to) in files {
        let r = match mode {
            MigrationMode::Copy => media::copy_file(from, to).map(|_| media::journal_copy(to)),
            MigrationMode::Move => media::move_file(from, to),
        };
        if let Err(e) = r {
            for (f, t) in done.into_iter().rev() {
                match mode {
                    MigrationMode::Copy => {
                        let _ = fs::remove_file(t);
                    }
                    MigrationMode::Move => {
                        let _ = media::move_file(t, f);
                    }
                }
            }
            return Err(e);
        }
        done.push((from, to));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path, content: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn opts(mode: MigrationMode, media: &str) -> ImportOptions {
        ImportOptions { mode, media: media.into(), required: vec!["image".into()] }
    }

    fn rules() -> CompiledRules {
        CompiledRules::new(&[])
    }

    /// Origen clásico con gamelist, medios (uno con nombre antiguo) y una ROM fuera de la gamelist
    fn classic_source(root: &Path) -> PathBuf {
        let src = root.join("from/snes");
        touch(&src.join("Mario (USA).sfc"), b"mario");
        touch(&src.join("Zelda (USA).sfc"), b"zelda");
        touch(&src.join("covers/Mario (USA).png"), b"png");
        touch(&src.join("images/Mario (USA)-thumb.png"), b"shot");
        touch(&src.join("videos/Mario (USA).mp4"), b"mp4");
        touch(
            &src.join("gamelist.xml"),
            br#"<gameList><game><path>./Mario (USA).sfc</path><name>Super Mario</name><favorite>true</favorite><image>./covers/Mario (USA).png</image><thumbnail>./images/Mario (USA)-thumb.png</thumbnail><video>./videos/Mario (USA).mp4</video></game></gameList>"#,
        );
        src
    }

    #[test]
    fn detects_classic_es_de_and_loose_sources() {
        let tmp = tempfile::tempdir().unwrap();
        let classic = classic_source(tmp.path());
        assert_eq!(detect(&classic, None).unwrap().info.kind, "classic");

        let esde_roms = tmp.path().join("card/ROMs/snes");
        touch(&esde_roms.join("Mario.sfc"), b"m");
        touch(&tmp.path().join("card/ES-DE/gamelists/snes/gamelist.xml"), b"<gameList/>");
        let s = detect(&esde_roms, None).unwrap();
        assert_eq!(s.info.kind, "esde");
        assert!(s.info.esde_home.unwrap().ends_with("ES-DE"));

        let loose = tmp.path().join("loose/snes");
        touch(&loose.join("Mario.sfc"), b"m");
        assert_eq!(detect(&loose, None).unwrap().info.kind, "loose");
    }

    #[test]
    fn classifies_new_same_rom_other_version_and_taken() {
        let tmp = tempfile::tempdir().unwrap();
        let src = classic_source(tmp.path());
        touch(&src.join("Sonic (USA).sfc"), b"sonic-usa");
        touch(&src.join("Kirby.sfc"), b"kirby-from-source");
        let dst = SystemRef::from_rom_dir(tmp.path().join("lib/snes"));
        touch(&dst.rom_dir.join("Zelda copy.sfc"), b"zelda"); // same content, other name
        touch(&dst.rom_dir.join("Sonic (Europe).sfc"), b"sonic-eur"); // another version
        touch(&dst.rom_dir.join("Kirby.sfc"), b"another kirby"); // same name, other content
        let dst_games = vec![
            Game { path: "./Zelda copy.sfc".into(), name: Some("Zelda".into()), ..Default::default() },
            Game { path: "./Sonic (Europe).sfc".into(), name: Some("Sonic".into()), ..Default::default() },
            Game { path: "./Kirby.sfc".into(), name: Some("Kirby Other".into()), ..Default::default() },
        ];
        let source = detect(&src, None).unwrap();
        let games = source_games(&source, &rules(), &dst);
        let mut cache = crate::rom_hash::CrcCache::load(&tmp.path().join("crc.json"));
        let list = classify(&source, &Layout::Classic, &dst, &games, &dst_games, Some(&mut cache), &|_| HashMap::new(), &Progress::none()).unwrap();
        let status: HashMap<&str, &str> = list.iter().map(|g| (strip_dot_slash(&g.path), g.status.as_str())).collect();
        assert_eq!(status["Mario (USA).sfc"], "new");
        assert_eq!(status["Zelda (USA).sfc"], "same_rom");
        assert_eq!(status["Sonic (USA).sfc"], "other_version");
        assert_eq!(status["Kirby.sfc"], "taken");
        let mario = list.iter().find(|g| g.path == "./Mario (USA).sfc").unwrap();
        assert_eq!(mario.media, vec!["image", "thumbnail", "video"]);
    }

    #[test]
    fn imports_classic_into_es_de_with_required_media_only() {
        let tmp = tempfile::tempdir().unwrap();
        let src = classic_source(tmp.path());
        let home = tmp.path().join("ES-DE");
        let layout = Layout::Esde(EsdePaths { home: home.clone(), media_root: home.join("downloaded_media") });
        let dst = SystemRef::from_rom_dir(tmp.path().join("lib/snes"));
        fs::create_dir_all(&dst.rom_dir).unwrap();
        let source = detect(&src, None).unwrap();
        let games: Vec<Game> = source_games(&source, &rules(), &dst).into_iter().map(|(g, _)| g).collect();
        let mut dst_games = Vec::new();
        let r = import(&source, &layout, &dst, &mut dst_games, &games, &opts(MigrationMode::Copy, "required"), &Progress::none());
        assert_eq!(r.imported.len(), 2);
        assert_eq!(r.media, 1);
        assert!(dst.rom_dir.join("Mario (USA).sfc").is_file() && src.join("Mario (USA).sfc").is_file(), "copied, source kept");
        assert!(home.join("downloaded_media/snes/covers/Mario (USA).png").is_file());
        assert!(!home.join("downloaded_media/snes/screenshots").exists(), "only the required media");
        let mario = dst_games.iter().find(|g| g.path == "./Mario (USA).sfc").unwrap();
        assert_eq!(mario.name.as_deref(), Some("Super Mario"));
        assert_eq!(mario.favorite.as_deref(), Some("true"));
        assert!(mario.image.is_none(), "ES-DE: no media paths");
    }

    #[test]
    fn imports_es_de_into_classic_with_all_media_moving_and_never_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let roms = tmp.path().join("card/ROMs/snes");
        touch(&roms.join("Mario.sfc"), b"m");
        touch(&roms.join("Taken.sfc"), b"t1");
        touch(&tmp.path().join("card/ES-DE/gamelists/snes/gamelist.xml"), br#"<gameList><game><path>./Mario.sfc</path><name>Mario</name></game></gameList>"#);
        touch(&tmp.path().join("card/ES-DE/downloaded_media/snes/covers/Mario.png"), b"png");
        touch(&tmp.path().join("card/ES-DE/downloaded_media/snes/screenshots/Mario.png"), b"shot");
        let dst = SystemRef::from_rom_dir(tmp.path().join("lib/snes"));
        touch(&dst.rom_dir.join("Taken.sfc"), b"t2");

        let source = detect(&roms, None).unwrap();
        let games: Vec<Game> = source_games(&source, &rules(), &dst).into_iter().map(|(g, _)| g).collect();
        let mut dst_games = Vec::new();
        let r = import(&source, &Layout::Classic, &dst, &mut dst_games, &games, &opts(MigrationMode::Move, "all"), &Progress::none());
        assert_eq!(r.imported, vec!["./Mario.sfc"]);
        assert_eq!(r.skipped.len(), 1, "Taken.sfc exists: skipped");
        assert_eq!(fs::read(dst.rom_dir.join("Taken.sfc")).unwrap(), b"t2", "never overwritten");
        assert!(!roms.join("Mario.sfc").exists(), "moved");
        let mario = &dst_games[0];
        assert_eq!(mario.image.as_deref(), Some("./covers/Mario.png"));
        assert_eq!(mario.thumbnail.as_deref(), Some("./screenshots/Mario.png"));
        assert!(dst.rom_dir.join("screenshots/Mario.png").is_file());
    }

    #[test]
    fn loose_roms_get_their_title_from_the_file_name() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("loose/snes");
        touch(&src.join("Contra III (USA).sfc"), b"c");
        let source = detect(&src, None).unwrap();
        let dst = SystemRef::from_rom_dir(tmp.path().join("lib/snes"));
        let games = source_games(&source, &rules(), &dst);
        assert_eq!(games[0].0.name.as_deref(), Some("Contra III (USA)"));
    }

    #[test]
    fn extensions_are_checked_against_the_destination_also_for_gamelist_entries() {
        let tmp = tempfile::tempdir().unwrap();
        // Carpeta de origen con otro nombre: manda el sistema de destino
        let src = tmp.path().join("from/whatever");
        touch(&src.join("Mario.sfc"), b"m");
        touch(&src.join("Notes.txt"), b"t");
        touch(&src.join("Zelda.nes"), b"z");
        touch(&src.join("gamelist.xml"), br#"<gameList><game><path>./Zelda.nes</path><name>Zelda</name></game></gameList>"#);
        let dst = SystemRef::from_rom_dir(tmp.path().join("lib/snes"));
        let source = detect(&src, None).unwrap();
        let known: HashMap<String, bool> = source_games(&source, &rules(), &dst).into_iter().map(|(g, k)| (strip_dot_slash(&g.path).to_string(), k)).collect();
        assert!(known["Mario.sfc"]);
        assert!(!known["Zelda.nes"], "in the source gamelist, but not a snes extension");
        assert!(!known.get("Notes.txt").copied().unwrap_or(false));
    }

    #[test]
    fn same_rom_with_another_name_completes_your_game_and_is_not_duplicated() {
        let tmp = tempfile::tempdir().unwrap();
        let src = classic_source(tmp.path());
        let dst = SystemRef::from_rom_dir(tmp.path().join("lib/snes"));
        touch(&dst.rom_dir.join("Mario copy.sfc"), b"mario"); // the same ROM, another name
        touch(&dst.rom_dir.join("screenshots/Mario copy.png"), b"mine"); // it has a screenshot already
        let mut dst_games = vec![Game {
            path: "./Mario copy.sfc".into(),
            name: Some("Mario mine".into()),
            thumbnail: Some("./screenshots/Mario copy.png".into()),
            ..Default::default()
        }];
        let source = detect(&src, None).unwrap();
        let games = source_games(&source, &rules(), &dst);
        let mut cache = crate::rom_hash::CrcCache::load(&tmp.path().join("crc.json"));
        let list = classify(&source, &Layout::Classic, &dst, &games, &dst_games, Some(&mut cache), &|_| HashMap::new(), &Progress::none()).unwrap();
        let mario = list.iter().find(|g| g.path == "./Mario (USA).sfc").unwrap();
        assert_eq!(mario.status, "same_rom");
        assert_eq!(mario.mine.as_deref(), Some("./Mario copy.sfc"));
        assert_eq!(mario.fills, 1, "favorite (the name is not empty)");
        assert_eq!(mario.media, vec!["image", "video"], "not the screenshot it already has");

        let mario_src = games.iter().find(|(g, _)| g.path == "./Mario (USA).sfc").unwrap().0.clone();
        let (r, merged) = complete_games(&source, &Layout::Classic, &dst, &mut dst_games, &[(mario_src, "./Mario copy.sfc".into())], &opts(MigrationMode::Copy, "all"), &Progress::none());
        assert_eq!(r.completed, vec!["./Mario copy.sfc"]);
        assert_eq!(r.media, 2);
        assert!(!dst.rom_dir.join("Mario (USA).sfc").exists(), "the ROM is not copied");
        assert_eq!(dst_games.len(), 1);
        let g = &dst_games[0];
        assert_eq!((g.name.as_deref(), g.favorite.as_deref()), (Some("Mario mine"), Some("true")));
        assert_eq!(g.image.as_deref(), Some("./covers/Mario copy.png"));
        assert_eq!(fs::read(dst.rom_dir.join("screenshots/Mario copy.png")).unwrap(), b"mine", "never overwritten");
        assert!(dst.rom_dir.join("videos/Mario copy.mp4").is_file());
        assert_eq!(merged.len(), 1);
        assert!(merged[0].1.favorite.is_none(), "the entry as it was, for Undo");
    }

    #[test]
    fn same_rom_with_nothing_to_add_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("loose/snes");
        touch(&src.join("Contra.sfc"), b"c");
        let dst = SystemRef::from_rom_dir(tmp.path().join("lib/snes"));
        touch(&dst.rom_dir.join("Contra.sfc"), b"c");
        let mut dst_games = vec![Game { path: "./Contra.sfc".into(), name: Some("Contra".into()), ..Default::default() }];
        let source = detect(&src, None).unwrap();
        let games = source_games(&source, &rules(), &dst);
        let list = classify(&source, &Layout::Classic, &dst, &games, &dst_games, None, &|_| HashMap::new(), &Progress::none()).unwrap();
        assert_eq!((list[0].status.as_str(), list[0].fills, list[0].media.len()), ("same_rom", 0, 0));
        let (r, merged) = complete_games(&source, &Layout::Classic, &dst, &mut dst_games, &[(games[0].0.clone(), "./Contra.sfc".into())], &opts(MigrationMode::Copy, "all"), &Progress::none());
        assert!(r.completed.is_empty() && merged.is_empty());
    }
}
