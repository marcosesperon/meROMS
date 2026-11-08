//! Migrador entre estructuras (clásica ↔ ES-DE).
//!
//! Qué hace por cada sistema:
//! - Medios: copia o mueve cada carpeta de tipo (covers/, screenshots/…) tal cual a su
//!   sitio en la otra estructura. Como el nombrado es el mismo, no hay que renombrar nada.
//! - Gamelist: la escribe donde la quiere el destino, quitando (→ ES-DE) o regenerando
//!   (→ clásica) las rutas de medios. Los campos que meROMS no conoce se conservan.
//! - Colecciones: convierte las rutas `./juego.zip` ↔ `%ROMPATH%/<sistema>/juego.zip`.
//!
//! Las ROMs no se tocan nunca. Si un fichero ya existe en el destino se salta y se informa.
//! La vista previa y la ejecución usan exactamente el mismo código (`dry_run`), para que
//! lo que se enseña sea lo que luego pasa.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::layout::{Layout, SystemRef};
use crate::media::{self, MediaIndex, MEDIA_KINDS};
use crate::models::{AppSettings, Game, GameList, Profile};
use crate::parser;

/// Como mucho se devuelven estos ficheros saltados por sistema (el resto solo se cuenta).
const MAX_LISTED_SKIPS: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationMode {
    Copy,
    Move,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SystemReport {
    pub name: String,
    pub has_gamelist: bool,
    /// En el destino ya había gamelist (se hace copia antes de reemplazarla)
    pub dest_gamelist_exists: bool,
    pub games: usize,
    pub media_transferred: usize,
    pub media_skipped_count: usize,
    /// Primeros ficheros saltados (ya existían en el destino o fallaron)
    pub media_skipped: Vec<String>,
    /// Medios de tipos que la estructura destino no tiene (p. ej. wheels/ → ES-DE)
    pub media_unsupported: usize,
    /// Clásica → ES-DE: medios con nombre antiguo que pasan ya con el nombrado unificado
    /// (incluidos en `media_transferred`)
    pub media_renamed: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MigrationReport {
    pub dry_run: bool,
    pub systems: Vec<SystemReport>,
    /// Ficheros de colección escritos (o que se escribirían)
    pub collections: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Serialize)]
struct MigrationProgress {
    current: usize,
    total: usize,
    system: String,
    /// Fichero de medios en curso (vacío al empezar cada sistema)
    file: String,
}

/// `source` puede ser un perfil guardado o uno «detectado» que no está en los ajustes
/// (p. ej. la estructura clásica de la misma carpeta). Con `systems` solo se migran esos.
#[tauri::command]
pub async fn plan_migration(
    app: tauri::AppHandle,
    source: Profile,
    target: Profile,
    mode: MigrationMode,
    systems: Option<Vec<String>>,
) -> Result<MigrationReport, String> {
    let (src, dst, rom_root) = prepare(&source, &target)?;
    // Los ajustes hacen falta para reconocer medios con nombre antiguo configurado
    let settings = super::settings::get_settings(app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let scope = Scope { rom_root: &rom_root, only: systems.as_deref(), settings: &settings };
        migrate(&src, &dst, &scope, mode, true, &mut |_, _, _, _| {})
    })
        .await
        .map_err(|e| format!("Migration task failed: {}", e))?
}

#[tauri::command]
pub async fn run_migration(
    app: tauri::AppHandle,
    source: Profile,
    target: Profile,
    mode: MigrationMode,
    systems: Option<Vec<String>>,
) -> Result<MigrationReport, String> {
    let (src, dst, rom_root) = prepare(&source, &target)?;
    let settings = super::settings::get_settings(app.clone())?;
    let emitter = app.clone();
    // En otro hilo: copiar miles de medios no debe congelar la ventana
    tauri::async_runtime::spawn_blocking(move || {
        let scope = Scope { rom_root: &rom_root, only: systems.as_deref(), settings: &settings };
        // Un aviso por sistema y, dentro, como mucho uno cada 50 ms con el fichero en curso
        let mut last: Option<std::time::Instant> = None;
        migrate(&src, &dst, &scope, mode, false, &mut |current, total, system, file| {
            let now = std::time::Instant::now();
            if !file.is_empty() && last.is_some_and(|l| now.duration_since(l) < std::time::Duration::from_millis(50)) {
                return;
            }
            last = Some(now);
            let _ = emitter.emit(
                "migration-progress",
                MigrationProgress { current, total, system: system.to_string(), file: file.to_string() },
            );
        })
    })
    .await
    .map_err(|e| format!("Migration task failed: {}", e))?
}

/// Comprueba los perfiles y monta los dos layouts. El destino usa siempre la misma
/// carpeta de ROMs que el origen: las ROMs no se mueven.
fn prepare(source: &Profile, target: &Profile) -> Result<(Layout, Layout, PathBuf), String> {
    if source.kind == target.kind {
        return Err("Source and destination use the same structure".into());
    }
    let mut target = target.clone();
    target.rom_root_path = source.rom_root_path.clone();

    let rom_root = PathBuf::from(&source.rom_root_path);
    if !rom_root.is_dir() {
        return Err(format!("ROM folder not found: {}", source.rom_root_path));
    }
    Ok((Layout::for_profile(source)?, Layout::for_profile(&target)?, rom_root))
}

/// Qué se migra: la carpeta de ROMs, opcionalmente solo unos sistemas, y los ajustes
/// (para reconocer los medios con nombre antiguo).
struct Scope<'a> {
    rom_root: &'a Path,
    only: Option<&'a [String]>,
    settings: &'a AppSettings,
}

/// El migrador en sí. Con `dry_run` solo calcula el informe sin tocar el disco.
fn migrate(
    src: &Layout,
    dst: &Layout,
    scope: &Scope,
    mode: MigrationMode,
    dry_run: bool,
    on_progress: &mut dyn FnMut(usize, usize, &str, &str),
) -> Result<MigrationReport, String> {
    let rom_root = scope.rom_root;
    let mut report = MigrationReport { dry_run, ..Default::default() };
    report.warnings.extend(esde_rom_dir_warning(dst, rom_root));

    let mut systems = list_system_dirs(rom_root, &[src, dst])?;
    if let Some(only) = scope.only {
        systems.retain(|dir| only.contains(&SystemRef::from_rom_dir(dir).name));
    }
    let total = systems.len();
    for (i, dir) in systems.iter().enumerate() {
        let system = SystemRef::from_rom_dir(dir);
        on_progress(i + 1, total, &system.name, "");
        // Cada fichero de medios que se pasa, para enseñarlo en la ventana
        let mut on_file = |file: &str| on_progress(i + 1, total, &system.name, file);

        let mut sr = SystemReport { name: system.name.clone(), ..Default::default() };
        // Primero se lee la gamelist: dice qué fichero es el bueno de cada juego
        let source_gamelist = read_source_gamelist(src, &system, &mut sr)?;
        let games: &[Game] = source_gamelist.as_ref().map(|(g, _)| g.games.as_slice()).unwrap_or(&[]);
        let handled = if src.writes_media_tags() {
            transfer_normalized_media(src, dst, &system, scope.settings, games, mode, dry_run, &mut sr, &mut on_file)
        } else {
            HashSet::new()
        };
        transfer_media(src, dst, &system, &handled, mode, dry_run, &mut sr, &mut on_file);
        if let Some((gamelist, xml)) = source_gamelist {
            write_destination_gamelist(src, dst, &system, gamelist, &xml, mode, dry_run, &mut sr)?;
        }
        if matches!(src, Layout::Classic) {
            migrate_classic_collections(dst, &system, mode, dry_run, &mut report)?;
        }

        // Sistemas vacíos (ni gamelist ni medios) no se enseñan
        if sr.has_gamelist || sr.media_transferred + sr.media_skipped_count + sr.media_unsupported > 0 {
            report.systems.push(sr);
        }
    }

    if matches!(src, Layout::Esde(_)) {
        migrate_esde_collections(src, dst, rom_root, &systems, mode, dry_run, &mut report)?;
    }
    Ok(report)
}

/// Carpetas de sistema dentro de la carpeta de ROMs (ni las ocultas ni las propias de
/// ES-DE de ninguno de los `layouts`, por si ES-DE vive dentro de la carpeta de ROMs).
pub(super) fn list_system_dirs(rom_root: &Path, layouts: &[&Layout]) -> Result<Vec<PathBuf>, String> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(rom_root)
        .map_err(|e| format!("Failed to read ROM folder: {}", e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .filter(|p| !layouts.iter().any(|l| l.is_own_folder(p)))
        .collect();
    dirs.sort();
    Ok(dirs)
}

/// Lee la gamelist de origen tal cual está en disco (XML incluido, para conservar los
/// campos desconocidos).
fn read_source_gamelist(src: &Layout, system: &SystemRef, sr: &mut SystemReport) -> Result<Option<(GameList, String)>, String> {
    let path = src.gamelist_path(system);
    if !path.exists() {
        return Ok(None);
    }
    let xml = fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    let gamelist = parser::parse_gamelist(&xml)?;
    sr.has_gamelist = true;
    sr.games = gamelist.games.len();
    Ok(Some((gamelist, xml)))
}

/// Clásica → ES-DE: los medios que no siguen el nombrado unificado pasan directamente con
/// él. Qué fichero es el bueno lo decide el normalizador (`normalize::plan`), así la regla
/// vive en un solo sitio. Devuelve los ficheros de origen ya tratados, para que el recorrido
/// de carpetas de después no los copie otra vez con su nombre viejo.
#[allow(clippy::too_many_arguments)]
fn transfer_normalized_media(
    src: &Layout,
    dst: &Layout,
    system: &SystemRef,
    settings: &AppSettings,
    games: &[Game],
    mode: MigrationMode,
    dry_run: bool,
    sr: &mut SystemReport,
    on_file: &mut dyn FnMut(&str),
) -> HashSet<PathBuf> {
    use super::normalize::NormalizeStatus;

    let src_base = src.media_type_dir(system, "");
    let dst_base = dst.media_type_dir(system, "");
    let items = super::normalize::plan(src, system, settings, games);
    let move_targets: HashSet<&str> = items
        .iter()
        .filter(|i| i.status == NormalizeStatus::Move)
        .map(|i| i.to.as_str())
        .collect();
    let mut handled = HashSet::new();

    for item in &items {
        let from = PathBuf::from(&item.from);
        let to = PathBuf::from(&item.to);
        match item.status {
            // Ya está con su nombre: lo pasa el recorrido de carpetas
            NormalizeStatus::Link => continue,
            NormalizeStatus::Conflict if move_targets.contains(item.to.as_str()) => {
                // Otro fichero ya va a ese destino: este se queda fuera
                handled.insert(from);
                skip(sr, format!("{} (-> {})", item.from_relative, item.to_relative));
                continue;
            }
            NormalizeStatus::Conflict => {
                // En el origen ya hay un unificado, pero la gamelist apunta a otro: gana el de la gamelist
                handled.insert(to.clone());
                skip(sr, format!("{} (-> {})", item.to_relative, item.from_relative));
            }
            NormalizeStatus::Move => {}
        }
        handled.insert(from.clone());

        let Ok(rel) = to.strip_prefix(&src_base) else { continue };
        let supported = rel
            .components()
            .next()
            .and_then(|c| media::kind_for_folder(&c.as_os_str().to_string_lossy()))
            .is_some_and(|kind| kind.in_esde || dst.writes_media_tags());
        if !supported {
            sr.media_unsupported += 1;
            continue;
        }
        let dest = dst_base.join(rel);
        if dest.exists() {
            skip(sr, item.to_relative.clone());
            continue;
        }
        if !dry_run {
            on_file(&item.to_relative);
            let done = match mode {
                MigrationMode::Copy => media::copy_file(&from, &dest),
                MigrationMode::Move => media::move_file(&from, &dest),
            };
            if let Err(e) = done {
                skip(sr, format!("{} ({})", item.from_relative, e));
                continue;
            }
            if mode == MigrationMode::Move {
                if let Some(dir) = from.parent() {
                    media::remove_empty_dirs_up_to(dir, &src_base);
                }
            }
        }
        sr.media_transferred += 1;
        sr.media_renamed += 1;
    }
    handled
}

/// Copia o mueve las carpetas de medios de un sistema a la estructura destino.
/// `handled`: ficheros que ya ha pasado (o descartado) `transfer_normalized_media`.
#[allow(clippy::too_many_arguments)]
fn transfer_media(
    src: &Layout,
    dst: &Layout,
    system: &SystemRef,
    handled: &HashSet<PathBuf>,
    mode: MigrationMode,
    dry_run: bool,
    sr: &mut SystemReport,
    on_file: &mut dyn FnMut(&str),
) {
    let src_base = src.media_type_dir(system, "");
    for kind in MEDIA_KINDS {
        let src_dir = src.media_type_dir(system, kind.folder);
        let files: Vec<PathBuf> = media::list_files_recursive(&src_dir)
            .into_iter()
            .filter(|f| !handled.contains(f))
            .collect();
        if files.is_empty() {
            continue;
        }
        if matches!(dst, Layout::Esde(_)) && !kind.in_esde {
            sr.media_unsupported += files.len();
            continue;
        }
        let dst_dir = dst.media_type_dir(system, kind.folder);

        for file in files {
            let Ok(rel) = file.strip_prefix(&src_dir) else { continue };
            let to = dst_dir.join(rel);
            let shown = Path::new(kind.folder).join(rel).to_string_lossy().to_string();

            if to.exists() {
                skip(sr, shown);
                continue;
            }
            if !dry_run {
                on_file(&shown);
                let done = match mode {
                    MigrationMode::Copy => media::copy_file(&file, &to),
                    MigrationMode::Move => media::move_file(&file, &to),
                };
                if let Err(e) = done {
                    skip(sr, format!("{} ({})", shown, e));
                    continue;
                }
                if mode == MigrationMode::Move {
                    if let Some(dir) = file.parent() {
                        media::remove_empty_dirs_up_to(dir, &src_base);
                    }
                }
            }
            sr.media_transferred += 1;
        }
    }
}

fn skip(sr: &mut SystemReport, what: String) {
    sr.media_skipped_count += 1;
    if sr.media_skipped.len() < MAX_LISTED_SKIPS {
        sr.media_skipped.push(what);
    }
}

/// Escribe la gamelist en el destino:
/// - → ES-DE: sin rutas de medios (write_gamelist ya las quita).
/// - → clásica: con las rutas regeneradas a partir de los medios que hay ahora en el destino.
/// En modo «mover», la de origen se guarda en sus copias y se quita.
#[allow(clippy::too_many_arguments)]
fn write_destination_gamelist(
    src: &Layout,
    dst: &Layout,
    system: &SystemRef,
    mut gamelist: GameList,
    source_xml: &str,
    mode: MigrationMode,
    dry_run: bool,
    sr: &mut SystemReport,
) -> Result<(), String> {
    sr.dest_gamelist_exists = dst.gamelist_path(system).exists();
    if dry_run {
        return Ok(());
    }

    gamelist.games.iter_mut().for_each(media::clear_media_fields);
    if dst.writes_media_tags() {
        let index = MediaIndex::build(dst, system);
        for game in &mut gamelist.games {
            for kind in MEDIA_KINDS {
                if let Some(file) = index.find(system, kind, &game.path) {
                    media::set_field(game, kind.field, Some(media::media_ref(dst, system, file)));
                }
            }
        }
    }

    // Los campos desconocidos salen del XML de origen (el destino puede ni existir)
    super::gamelist::write_gamelist_with_original(dst, system, &gamelist, Some(source_xml))?;

    if mode == MigrationMode::Move {
        let src_path = src.gamelist_path(system);
        // Antes de quitarla, queda guardada en las copias de seguridad del origen
        super::backup::backup_gamelist(src, system)?;
        fs::remove_file(&src_path).map_err(|e| format!("Failed to remove {}: {}", src_path.display(), e))?;
    }
    Ok(())
}

/// Clásica → ES-DE: las colecciones custom-*.cfg de la carpeta del sistema pasan a
/// <ES-DE>/collections con rutas %ROMPATH%, fusionándose con lo que ya haya.
fn migrate_classic_collections(
    dst: &Layout,
    system: &SystemRef,
    mode: MigrationMode,
    dry_run: bool,
    report: &mut MigrationReport,
) -> Result<(), String> {
    for file in collection_files(&system.rom_dir) {
        let Some(name) = collection_name(&file) else { continue };
        let content = fs::read_to_string(&file).unwrap_or_default();
        let entries: Vec<String> = content
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| dst.collection_entry(system, &format!("./{}", crate::layout::strip_dot_slash(l))))
            .collect();

        let dest = dst.collections_dir(system).join(super::collections::collection_file_name(dst, &name));
        write_collection(&dest, &entries, dry_run)?;
        if !dry_run && mode == MigrationMode::Move {
            let _ = fs::remove_file(&file);
        }
        push_unique(&mut report.collections, dest.to_string_lossy().to_string());
    }
    Ok(())
}

/// ES-DE → clásica: cada colección (que mezcla sistemas) se reparte en un custom-*.cfg
/// por sistema, con rutas relativas («./juego.zip»).
fn migrate_esde_collections(
    src: &Layout,
    dst: &Layout,
    rom_root: &Path,
    systems: &[PathBuf],
    mode: MigrationMode,
    dry_run: bool,
    report: &mut MigrationReport,
) -> Result<(), String> {
    // Cualquier sistema sirve para preguntar dónde están (en ES-DE es una carpeta común)
    let Some(any_system) = systems.first().map(SystemRef::from_rom_dir) else { return Ok(()) };
    let migrated: Vec<String> = systems.iter().map(|d| SystemRef::from_rom_dir(d).name).collect();

    for file in collection_files(&src.collections_dir(&any_system)) {
        let Some(name) = collection_name(&file) else { continue };
        let content = fs::read_to_string(&file).unwrap_or_default();

        let mut by_system: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut unconverted = 0;
        // Entradas de sistemas que no entran en esta migración: se dejan para otra vez
        let mut not_in_scope = 0;
        for line in content.lines().map(str::trim).filter(|l| !l.is_empty()) {
            match parse_esde_entry(line, rom_root) {
                Some((system, rom_path)) if migrated.contains(&system) => {
                    by_system.entry(system).or_default().push(rom_path)
                }
                Some((system, _)) if only_some_systems(rom_root, &system, &migrated) => not_in_scope += 1,
                _ => unconverted += 1,
            }
        }
        if unconverted > 0 {
            report.warnings.push(format!(
                "Collection '{}': {} entries could not be converted and were left out",
                name, unconverted
            ));
        }

        for (system_name, entries) in &by_system {
            let system = SystemRef::from_rom_dir(rom_root.join(system_name));
            let dest = dst.collections_dir(&system).join(super::collections::collection_file_name(dst, &name));
            write_collection(&dest, entries, dry_run)?;
            push_unique(&mut report.collections, dest.to_string_lossy().to_string());
        }
        // Solo se quita el original si se ha podido convertir entero
        if !dry_run && mode == MigrationMode::Move && unconverted == 0 && not_in_scope == 0 {
            let _ = fs::remove_file(&file);
        }
    }
    Ok(())
}

/// ¿La entrada es de un sistema que existe pero no se está migrando ahora?
fn only_some_systems(rom_root: &Path, system: &str, migrated: &[String]) -> bool {
    !migrated.iter().any(|m| m == system) && rom_root.join(system).is_dir()
}

/// «%ROMPATH%/snes/Juego.sfc» (o una ruta absoluta dentro de la carpeta de ROMs)
/// → ("snes", "./Juego.sfc").
fn parse_esde_entry(entry: &str, rom_root: &Path) -> Option<(String, String)> {
    let rest = match entry.strip_prefix("%ROMPATH%") {
        Some(r) => r.trim_start_matches(['/', '\\']).to_string(),
        None => Path::new(entry).strip_prefix(rom_root).ok()?.to_string_lossy().to_string(),
    };
    let rest = rest.replace('\\', "/");
    let (system, rom) = rest.split_once('/')?;
    if system.is_empty() || rom.is_empty() {
        return None;
    }
    Some((system.to_string(), format!("./{}", rom)))
}

fn write_collection(dest: &Path, entries: &[String], dry_run: bool) -> Result<(), String> {
    if dry_run || entries.is_empty() {
        return Ok(());
    }
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    }
    let existing = fs::read_to_string(dest).unwrap_or_default();
    let merged = super::collections::merge_collection_entries(&existing, entries);
    fs::write(dest, merged).map_err(|e| format!("Failed to write {}: {}", dest.display(), e))
}

fn collection_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && collection_name(p).is_some())
        .collect();
    files.sort();
    files
}

/// «custom-Mis Juegos.cfg» → «Mis Juegos»
fn collection_name(file: &Path) -> Option<String> {
    let name = file.file_name()?.to_string_lossy();
    let inner = name.strip_prefix("custom-")?.strip_suffix(".cfg")?;
    (!inner.is_empty()).then(|| inner.to_string())
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !list.contains(&value) {
        list.push(value);
    }
}

/// Si ES-DE está configurado para leer las ROMs de otra carpeta, no verá esta biblioteca.
fn esde_rom_dir_warning(dst: &Layout, rom_root: &Path) -> Option<String> {
    let Layout::Esde(esde) = dst else { return None };
    let detected = crate::layout::esde_settings::detect_esde_paths(&esde.home).ok()?;
    let configured = PathBuf::from(&detected.rom_root_path);
    let same = fs::canonicalize(&configured).ok() == fs::canonicalize(rom_root).ok() || configured == rom_root;
    (!same).then(|| {
        format!(
            "ES-DE is configured to read ROMs from '{}', but this library is in '{}'. Change ES-DE's ROM directory after migrating.",
            detected.rom_root_path,
            rom_root.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::EsdePaths;

    fn touch(path: &Path, content: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// Migra la biblioteca de prueba con los ajustes por defecto.
    fn run_on(
        f: &Fixture,
        src: &Layout,
        dst: &Layout,
        only: Option<&Vec<String>>,
        mode: MigrationMode,
        dry_run: bool,
        on_progress: &mut dyn FnMut(usize, usize, &str, &str),
    ) -> Result<MigrationReport, String> {
        let settings = AppSettings::default();
        let scope = Scope { rom_root: &f.roms, only: only.map(|o| o.as_slice()), settings: &settings };
        migrate(src, dst, &scope, mode, dry_run, on_progress)
    }

    struct Fixture {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        roms: PathBuf,
        esde: Layout,
    }

    /// Biblioteca clásica ficticia con un sistema, una portada, un vídeo y una colección.
    fn classic_library() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let roms = root.join("ROMs");
        let snes = roms.join("snes");
        touch(&snes.join("Juego Uno.sfc"), b"rom");
        touch(&snes.join("covers/Juego Uno.png"), b"cover");
        touch(&snes.join("videos/Juego Uno.mp4"), b"video");
        touch(&snes.join("images/Juego Uno-marquee.png"), b"legacy");
        touch(&snes.join("custom-Favoritos.cfg"), b"./Juego Uno.sfc\n");
        fs::write(
            snes.join("gamelist.xml"),
            r#"<?xml version="1.0"?>
<gameList>
  <game>
    <path>./Juego Uno.sfc</path>
    <name>Juego Uno</name>
    <image>./covers/Juego Uno.png</image>
    <marquee>./images/Juego Uno-marquee.png</marquee>
    <video>./videos/Juego Uno.mp4</video>
    <completed>true</completed>
  </game>
</gameList>
"#,
        )
        .unwrap();
        let esde = Layout::Esde(EsdePaths { home: root.join("ES-DE"), media_root: root.join("ES-DE/downloaded_media") });
        Fixture { _tmp: tmp, root, roms, esde }
    }

    #[test]
    fn la_vista_previa_no_toca_nada_y_cuenta_lo_mismo_que_la_ejecucion() {
        let f = classic_library();
        let plan = run_on(&f, &Layout::Classic, &f.esde, None, MigrationMode::Copy, true, &mut |_, _, _, _| {}).unwrap();
        assert!(!f.root.join("ES-DE").exists(), "dry run must not write anything");

        let run = run_on(&f, &Layout::Classic, &f.esde, None, MigrationMode::Copy, false, &mut |_, _, _, _| {}).unwrap();
        assert_eq!(plan.systems[0].media_transferred, run.systems[0].media_transferred);
        assert_eq!(run.systems[0].media_transferred, 3);
        // El marquee con nombre antiguo pasa ya renombrado
        assert_eq!(run.systems[0].media_renamed, 1);
        assert_eq!(plan.systems[0].media_renamed, 1);
    }

    #[test]
    fn con_lista_de_sistemas_solo_migra_esos() {
        let f = classic_library();
        touch(&f.roms.join("megadrive/Otro.md"), b"rom");
        touch(&f.roms.join("megadrive/gamelist.xml"), b"<gameList><game><path>./Otro.md</path></game></gameList>");
        let only = vec!["snes".to_string()];
        let report = run_on(&f, &Layout::Classic, &f.esde, Some(&only), MigrationMode::Copy, false, &mut |_, _, _, _| {}).unwrap();

        let names: Vec<_> = report.systems.iter().map(|s| s.name.clone()).collect();
        assert_eq!(names, vec!["snes"]);
        assert!(f.root.join("ES-DE/gamelists/snes/gamelist.xml").exists());
        assert!(!f.root.join("ES-DE/gamelists/megadrive").exists());
    }

    #[test]
    fn clasica_a_esde_copiando() {
        let f = classic_library();
        run_on(&f, &Layout::Classic, &f.esde, None, MigrationMode::Copy, false, &mut |_, _, _, _| {}).unwrap();

        let media = f.root.join("ES-DE/downloaded_media/snes");
        assert!(media.join("covers/Juego Uno.png").exists());
        assert!(media.join("videos/Juego Uno.mp4").exists());
        // El medio con nombre antiguo llega con el nombrado unificado (y no con el viejo)
        assert!(media.join("marquees/Juego Uno.png").exists());
        assert!(!media.join("images").exists());
        // Copiar deja el origen intacto
        assert!(f.roms.join("snes/covers/Juego Uno.png").exists());
        assert!(f.roms.join("snes/gamelist.xml").exists());

        let xml = fs::read_to_string(f.root.join("ES-DE/gamelists/snes/gamelist.xml")).unwrap();
        assert!(!xml.contains("<image>") && !xml.contains("<video>"));
        assert!(xml.contains("<completed>true</completed>"), "unknown fields survive: {}", xml);

        let collection = fs::read_to_string(f.root.join("ES-DE/collections/custom-Favoritos.cfg")).unwrap();
        assert_eq!(collection, "%ROMPATH%/snes/Juego Uno.sfc\n");
    }

    #[test]
    fn clasica_a_esde_moviendo_deja_el_origen_en_las_copias() {
        let f = classic_library();
        run_on(&f, &Layout::Classic, &f.esde, None, MigrationMode::Move, false, &mut |_, _, _, _| {}).unwrap();

        let snes = f.roms.join("snes");
        assert!(!snes.join("covers").exists(), "moved media folders are removed when empty");
        assert!(!snes.join("images").exists(), "legacy folders too");
        assert!(!snes.join("gamelist.xml").exists());
        assert!(fs::read_dir(snes.join(".backups")).unwrap().count() >= 1);
        assert!(!snes.join("custom-Favoritos.cfg").exists());
        // La ROM sigue en su sitio
        assert!(snes.join("Juego Uno.sfc").exists());
    }

    #[test]
    fn ida_y_vuelta_regenera_las_rutas_de_la_clasica() {
        let f = classic_library();
        run_on(&f, &Layout::Classic, &f.esde, None, MigrationMode::Move, false, &mut |_, _, _, _| {}).unwrap();
        let back = run_on(&f, &f.esde, &Layout::Classic, None, MigrationMode::Move, false, &mut |_, _, _, _| {}).unwrap();

        assert_eq!(back.systems[0].media_transferred, 3);
        let xml = fs::read_to_string(f.roms.join("snes/gamelist.xml")).unwrap();
        assert!(xml.contains("<image>./covers/Juego Uno.png</image>"), "{}", xml);
        assert!(xml.contains("<video>./videos/Juego Uno.mp4</video>"));
        assert!(xml.contains("<marquee>./marquees/Juego Uno.png</marquee>"), "{}", xml);
        assert!(xml.contains("<completed>true</completed>"));
        let collection = fs::read_to_string(f.roms.join("snes/custom-Favoritos.cfg")).unwrap();
        assert_eq!(collection, "./Juego Uno.sfc\n");
    }

    #[test]
    fn si_la_gamelist_apunta_a_otro_fichero_gana_ese() {
        let f = classic_library();
        // Ya hay un marquee unificado, pero la gamelist apunta al de nombre antiguo
        touch(&f.roms.join("snes/marquees/Juego Uno.png"), b"unificado");
        let report = run_on(&f, &Layout::Classic, &f.esde, None, MigrationMode::Copy, false, &mut |_, _, _, _| {}).unwrap();

        let dest = f.root.join("ES-DE/downloaded_media/snes/marquees/Juego Uno.png");
        assert_eq!(fs::read(dest).unwrap(), b"legacy");
        assert!(report.systems[0].media_skipped.iter().any(|s| s.starts_with("marquees/Juego Uno.png")));
    }

    #[test]
    fn las_carpetas_propias_de_esde_no_se_migran() {
        let f = classic_library();
        // ES-DE dentro de la carpeta de ROMs: su gamelists/ no es un sistema
        let esde = Layout::Esde(EsdePaths { home: f.roms.clone(), media_root: f.roms.join("downloaded_media") });
        touch(&f.roms.join("gamelists/cps1/gamelist.xml"), b"<gameList/>");
        let report = run_on(&f, &Layout::Classic, &esde, None, MigrationMode::Copy, true, &mut |_, _, _, _| {}).unwrap();
        let names: Vec<_> = report.systems.iter().map(|s| s.name.clone()).collect();
        assert_eq!(names, vec!["snes"]);
    }

    #[test]
    fn no_pisa_lo_que_ya_existe_en_el_destino() {
        let f = classic_library();
        touch(&f.root.join("ES-DE/downloaded_media/snes/covers/Juego Uno.png"), b"ya estaba");
        let report = run_on(&f, &Layout::Classic, &f.esde, None, MigrationMode::Copy, false, &mut |_, _, _, _| {}).unwrap();

        assert_eq!(report.systems[0].media_skipped, vec!["covers/Juego Uno.png".to_string()]);
        assert_eq!(
            fs::read(f.root.join("ES-DE/downloaded_media/snes/covers/Juego Uno.png")).unwrap(),
            b"ya estaba"
        );
    }

    #[test]
    fn entradas_de_coleccion_esde() {
        let roms = Path::new("/ficticio/ROMs");
        assert_eq!(
            parse_esde_entry("%ROMPATH%/snes/Sub/Juego.sfc", roms),
            Some(("snes".into(), "./Sub/Juego.sfc".into()))
        );
        assert_eq!(
            parse_esde_entry("/ficticio/ROMs/nes/a.nes", roms),
            Some(("nes".into(), "./a.nes".into()))
        );
        assert_eq!(parse_esde_entry("/otra/ruta/a.nes", roms), None);
    }
}
