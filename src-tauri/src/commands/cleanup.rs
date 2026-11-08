use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::{Layout, SystemRef};
use crate::media::{self, MediaIndex, MEDIA_KINDS};
use crate::models::GameList;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedMediaFile {
    /// Absolute path to the file
    pub path: String,
    /// Path relative to the system's media base (e.g. "covers/Mario.png")
    pub relative: String,
    /// File size in bytes
    pub size: u64,
}

/// Media subdirectories to scan for unused files.
pub(super) const MEDIA_DIRS: &[&str] = &[
    "images",
    "covers",
    "screenshots",
    "marquees",
    "titlescreens",
    "fanart",
    "3dboxes",
    "backcovers",
    "miximages",
    "videos",
    "physicalmedia",
    "manuals",
    "wheels",
    "media/images",
    "media/screenshots",
    "media/box2dfront",
];

const MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg",
    "mp4", "mkv", "avi", "mov", "wmv", "webm", "pdf",
];

/// Collect all media file paths referenced in the gamelist.
fn collect_referenced_paths(gamelist: &GameList, system_path: &Path) -> HashSet<String> {
    let mut referenced = HashSet::new();

    for game in &gamelist.games {
        // Every media field counts (fanart, titleshot, mix… are referenced too)
        for field in media::media_fields() {
            if let Some(media_path) = media::get_field(game, field) {
                // Resolve relative paths to absolute
                let resolved = if media_path.starts_with("./") {
                    system_path
                        .join(media_path.strip_prefix("./").unwrap_or(media_path))
                        .to_string_lossy()
                        .to_string()
                } else if media_path.starts_with('/') {
                    media_path.clone()
                } else {
                    system_path.join(media_path).to_string_lossy().to_string()
                };
                // Canonicalize for consistent comparison
                if let Ok(canonical) = fs::canonicalize(&resolved) {
                    referenced.insert(canonical.to_string_lossy().to_string());
                } else {
                    referenced.insert(resolved);
                }
            }
        }
    }

    referenced
}

/// Scan a directory recursively for media files.
fn scan_media_files(dir: &Path) -> Vec<(String, String, u64)> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return files;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(scan_media_files(&path));
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if MEDIA_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let abs = path.to_string_lossy().to_string();
                    files.push((abs, ext.to_string(), size));
                }
            }
        }
    }

    files
}

/// Find media files nobody uses:
/// - Classic: files in the media folders not referenced by any game in the gamelist.
/// - ES-DE: files whose name matches no ROM (ES-DE matches media by ROM name).
#[tauri::command(async)]
pub fn find_unused_media(app: tauri::AppHandle, system_path: String) -> Result<Vec<UnusedMediaFile>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let mut unused = match layout {
        Layout::Classic => find_unused_classic(&layout, &system, &system_path)?,
        Layout::Esde(_) => find_unused_esde(&layout, &system)?,
    };
    unused.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(unused)
}

fn find_unused_esde(layout: &Layout, system: &SystemRef) -> Result<Vec<UnusedMediaFile>, String> {
    let gamelist = crate::commands::gamelist::read_gamelist(layout, system)?;
    let rom_stems = rom_stems(system, &gamelist);
    let index = MediaIndex::build(layout, system);
    let media_base = layout.media_type_dir(system, "");

    let mut unused = Vec::new();
    for kind in MEDIA_KINDS.iter().filter(|k| k.in_esde) {
        for (stem, file) in index.files_of(kind) {
            if rom_stems.contains(stem) {
                continue;
            }
            let relative = file
                .strip_prefix(&media_base)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file.to_string_lossy().to_string());
            unused.push(UnusedMediaFile {
                path: file.to_string_lossy().to_string(),
                relative,
                size: fs::metadata(file).map(|m| m.len()).unwrap_or(0),
            });
        }
    }
    Ok(unused)
}

/// Media base names ES-DE would match: every gamelist entry plus every file or folder
/// found in the ROM folder (ES-DE lists ROMs even without a gamelist entry).
fn rom_stems(system: &SystemRef, gamelist: &GameList) -> HashSet<PathBuf> {
    let mut stems: HashSet<PathBuf> = gamelist
        .games
        .iter()
        .map(|g| crate::layout::rom_media_stem(system, &g.path))
        .collect();
    collect_rom_stems(&system.rom_dir, &system.rom_dir, &mut stems);
    stems
}

fn collect_rom_stems(root: &Path, dir: &Path, stems: &mut HashSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(root) else { continue };
        if path.is_dir() {
            // A folder can be a game folder or a "directory interpreted as file": keep both names
            stems.insert(rel.to_path_buf());
            collect_rom_stems(root, &path, stems);
        } else if let Some(stem) = rel.file_stem() {
            stems.insert(rel.with_file_name(stem));
        }
    }
}

fn find_unused_classic(layout: &Layout, system: &SystemRef, system_path: &str) -> Result<Vec<UnusedMediaFile>, String> {
    let sys_path = Path::new(system_path);

    // Load gamelist
    let gamelist = crate::commands::gamelist::read_gamelist(layout, system)?;
    let referenced = collect_referenced_paths(&gamelist, sys_path);

    let mut unused = Vec::new();

    // Scan all known media directories
    for subdir in MEDIA_DIRS {
        let dir = sys_path.join(subdir);
        if !dir.exists() {
            continue;
        }

        for (abs_path, _ext, size) in scan_media_files(&dir) {
            let canonical = fs::canonicalize(&abs_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| abs_path.clone());

            if !referenced.contains(&canonical) {
                let relative = abs_path
                    .strip_prefix(&format!("{}/", system_path))
                    .or_else(|| abs_path.strip_prefix(&system_path))
                    .unwrap_or(&abs_path)
                    .to_string();

                unused.push(UnusedMediaFile {
                    path: abs_path,
                    relative,
                    size,
                });
            }
        }
    }

    Ok(unused)
}

/// Leftover media of ONE game (game detail panel): files named after its ROM that it
/// doesn't use.
/// - ES-DE: in each media folder, the files with the ROM name that lose against the one
///   ES-DE shows (same extension preference as `MediaIndex`, e.g. the .jpg next to a .png).
/// - Classic: files in the media folders named after the ROM (or with an old suffix such as
///   "-image") that no media field of any game points to.
#[tauri::command(async)]
pub fn find_game_leftover_media(
    app: tauri::AppHandle,
    system_path: String,
    game_path: String,
) -> Result<Vec<UnusedMediaFile>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let mut leftovers = match layout {
        Layout::Classic => {
            let gamelist = crate::commands::gamelist::read_gamelist(&layout, &system)?;
            game_leftovers_classic(&system, &gamelist, &game_path)
        }
        Layout::Esde(_) => game_leftovers_esde(&layout, &system, &game_path),
    };
    leftovers.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(leftovers)
}

fn game_leftovers_esde(layout: &Layout, system: &SystemRef, game_path: &str) -> Vec<UnusedMediaFile> {
    let stem = crate::layout::rom_media_stem(system, game_path);
    let media_base = layout.media_type_dir(system, "");
    let mut leftovers = Vec::new();

    for kind in MEDIA_KINDS.iter().filter(|k| k.in_esde) {
        // Files of this type named after the ROM, in ES-DE's preference order
        let dir = layout.media_type_dir(system, kind.folder);
        let found: Vec<PathBuf> = files_named(&dir.join(&stem))
            .into_iter()
            .filter_map(|file| {
                let ext = media::extension_lower(&file)?;
                let priority = kind.extensions().iter().position(|e| *e == ext)?;
                Some((priority, file))
            })
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_values()
            .collect();
        // The first one is what ES-DE shows; the rest are leftovers
        for file in found.into_iter().skip(1) {
            leftovers.push(unused_file(&file, &media_base));
        }
    }
    leftovers
}

fn game_leftovers_classic(system: &SystemRef, gamelist: &GameList, game_path: &str) -> Vec<UnusedMediaFile> {
    let stem = crate::layout::rom_media_stem(system, game_path);
    let Some(rom_name) = stem.file_name().and_then(|n| n.to_str()) else { return Vec::new() };
    let referenced = collect_referenced_paths(gamelist, &system.rom_dir);

    // Names that belong to OTHER ROMs, so "Mario" never takes "Mario-cart"'s files
    let other_names: HashSet<String> = gamelist
        .games
        .iter()
        .filter(|g| g.path != game_path)
        .filter_map(|g| {
            let other = crate::layout::rom_media_stem(system, &g.path);
            other.file_name().map(|n| n.to_string_lossy().to_lowercase())
        })
        .collect();
    let suffixes: HashSet<&str> = super::images::legacy_targets()
        .into_iter()
        .flat_map(|(_, _, suffixes)| suffixes)
        .collect();
    let belongs_to_rom = |file_stem: &str| -> bool {
        if other_names.contains(&file_stem.to_lowercase()) {
            return false;
        }
        file_stem == rom_name
            || file_stem
                .strip_prefix(rom_name)
                .is_some_and(|rest| suffixes.contains(rest))
    };

    // Every folder where meROMS (or older tools) may have left media
    let mut folders: Vec<&str> = MEDIA_DIRS.to_vec();
    for (dirs, _, _) in super::images::legacy_targets() {
        folders.extend(dirs);
    }
    folders.sort_unstable();
    folders.dedup();

    let sub = stem.parent().unwrap_or(Path::new(""));
    let mut leftovers = Vec::new();
    for folder in folders {
        let dir = system.rom_dir.join(folder).join(sub);
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for path in entries.flatten().map(|e| e.path()).filter(|p| p.is_file()) {
            let is_media = media::extension_lower(&path).is_some_and(|ext| MEDIA_EXTENSIONS.contains(&ext.as_str()));
            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !is_media || !belongs_to_rom(file_stem) {
                continue;
            }
            let canonical = fs::canonicalize(&path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());
            if !referenced.contains(&canonical) {
                leftovers.push(unused_file(&path, &system.rom_dir));
            }
        }
    }
    leftovers
}

/// Files in the same folder as `stem_path` whose name (without extension) is its file name.
fn files_named(stem_path: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (stem_path.parent(), stem_path.file_name()) else { return Vec::new() };
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.file_stem() == Some(name))
        .collect()
}

fn unused_file(path: &Path, base: &Path) -> UnusedMediaFile {
    UnusedMediaFile {
        path: path.to_string_lossy().to_string(),
        relative: path
            .strip_prefix(base)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string()),
        size: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    }
}

/// Un medio que sobra porque es idéntico a otro tipo del mismo juego
#[derive(Debug, Clone, Serialize)]
pub struct RepeatedFile {
    pub path: String,
    /// Campo de la gamelist del tipo (image, thumbnail…)
    pub field: String,
    pub size: u64,
}

/// Un juego con la misma imagen en varios tipos: cuál se queda y cuáles sobran
#[derive(Debug, Clone, Serialize)]
pub struct RepeatedMedia {
    pub game_path: String,
    pub game_name: String,
    pub keep_path: String,
    pub keep_field: String,
    pub remove: Vec<RepeatedFile>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TrashMediaResult {
    pub trashed: usize,
    pub failed: Vec<String>,
}

/// Orden fijo para decidir qué tipo se queda cuando no se sabe por el tamaño de la imagen.
/// Solo imágenes: vídeos y manuales nunca coinciden con una imagen.
const KEEP_ORDER: &[&str] = &["image", "boxart", "boxback", "titleshot", "thumbnail", "marquee", "wheel", "fanart", "mix", "cartridge"];

/// Juegos que tienen exactamente el mismo fichero (byte a byte) en más de un tipo de medio.
/// Con `game_path`, solo ese juego (desde el detalle).
#[tauri::command(async)]
pub fn find_repeated_media(app: tauri::AppHandle, system_path: String, game_path: Option<String>) -> Result<Vec<RepeatedMedia>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let games = super::gamelist::read_gamelist(&layout, &system)?.games;
    Ok(find_repeated(&layout, &system, &games, game_path.as_deref()))
}

fn find_repeated(layout: &Layout, system: &SystemRef, games: &[crate::models::Game], only: Option<&str>) -> Vec<RepeatedMedia> {
    let index = MediaIndex::build(layout, system);
    // Lo habitual del sistema (siempre con todos los juegos, aunque se mire uno solo)
    let (cover_profile, shot_profile) = super::media_kinds::usual_sizes(layout, system, games);
    let mut out = Vec::new();

    for game in games.iter().filter(|g| only.is_none_or(|p| p == g.path)) {
        // Fichero actual de cada tipo de imagen, agrupando por tamaño en bytes (barato) y,
        // solo si coincide, por la huella MD5 del contenido
        let mut files: Vec<(&'static str, PathBuf, u64)> = Vec::new();
        for field in KEEP_ORDER {
            let Some(kind) = media::kind_for_field(field) else { continue };
            if let Some(path) = super::media_kinds::current_file(layout, system, &index, game, kind) {
                let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                if size > 0 && !files.iter().any(|(_, p, _)| *p == path) {
                    files.push((kind.field, path, size));
                }
            }
        }
        let mut groups: Vec<Vec<(&'static str, PathBuf, u64)>> = Vec::new();
        for (i, (_, _, size)) in files.iter().enumerate() {
            if files[..i].iter().any(|(_, _, s)| s == size) || files.iter().filter(|(_, _, s)| s == size).count() < 2 {
                continue; // ya agrupado, o sin pareja posible
            }
            let same_size: Vec<_> = files.iter().filter(|(_, _, s)| s == size).cloned().collect();
            let mut by_hash: Vec<(String, Vec<(&'static str, PathBuf, u64)>)> = Vec::new();
            for f in same_size {
                let Some(hash) = file_md5(&f.1) else { continue };
                match by_hash.iter_mut().find(|(h, _)| *h == hash) {
                    Some((_, list)) => list.push(f),
                    None => by_hash.push((hash, vec![f])),
                }
            }
            groups.extend(by_hash.into_iter().map(|(_, list)| list).filter(|l| l.len() > 1));
        }

        for group in groups {
            // Cuál se queda: si la imagen parece carátula (o captura) según lo habitual del
            // sistema y ese tipo está en el grupo, ese; si no, el primero del orden fijo
            let looks = super::media_kinds::info(&group[0].1)
                .map(|i| super::media_kinds::looks_like(&i, cover_profile, shot_profile));
            let preferred = match looks {
                Some(super::media_kinds::Looks::Cover) => Some("image"),
                Some(super::media_kinds::Looks::Screenshot) => Some("thumbnail"),
                _ => None,
            };
            let keep = preferred
                .and_then(|f| group.iter().find(|(field, _, _)| *field == f))
                .unwrap_or(&group[0]) // el grupo ya va en el orden fijo
                .clone();
            out.push(RepeatedMedia {
                game_path: game.path.clone(),
                game_name: game.name.clone().unwrap_or_else(|| game.path.clone()),
                keep_path: keep.1.to_string_lossy().to_string(),
                keep_field: keep.0.to_string(),
                remove: group
                    .iter()
                    .filter(|(field, _, _)| *field != keep.0)
                    .map(|(field, path, size)| RepeatedFile { path: path.to_string_lossy().to_string(), field: field.to_string(), size: *size })
                    .collect(),
            });
        }
    }
    out
}

fn file_md5(path: &Path) -> Option<String> {
    use md5::{Digest, Md5};
    let bytes = fs::read(path).ok()?;
    Some(format!("{:x}", Md5::digest(&bytes)))
}

/// Un campo de medio de la gamelist que apunta a un fichero que no existe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokenMediaRef {
    pub game_path: String,
    pub game_name: String,
    pub field: String,
    /// La ruta tal como está en la gamelist
    pub value: String,
}

/// Campos de medios que apuntan a ficheros que no existen (solo clásica: en ES-DE la
/// gamelist no guarda rutas de medios). Con `game_path`, solo ese juego.
#[tauri::command(async)]
pub fn find_broken_media_refs(app: tauri::AppHandle, system_path: String, game_path: Option<String>) -> Result<Vec<BrokenMediaRef>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    if !layout.writes_media_tags() {
        return Ok(Vec::new());
    }
    let gamelist = super::gamelist::read_gamelist(&layout, &system)?;
    Ok(broken_refs(&system, &gamelist, game_path.as_deref()))
}

fn broken_refs(system: &SystemRef, gamelist: &GameList, only: Option<&str>) -> Vec<BrokenMediaRef> {
    let mut out = Vec::new();
    for game in gamelist.games.iter().filter(|g| only.is_none_or(|p| p == g.path)) {
        for field in media::media_fields() {
            let Some(value) = media::get_field(game, field) else { continue };
            if value.trim().is_empty() || is_remote(value) || media::resolve_media_path(system, value).exists() {
                continue;
            }
            out.push(BrokenMediaRef {
                game_path: game.path.clone(),
                game_name: game.name.clone().unwrap_or_else(|| game.path.clone()),
                field: field.to_string(),
                value: value.clone(),
            });
        }
    }
    out
}

/// Una URL no es un fichero local: no se da por rota
fn is_remote(value: &str) -> bool {
    let v = value.to_lowercase();
    v.starts_with("http://") || v.starts_with("https://")
}

/// Quita de la gamelist los campos indicados, si siguen apuntando a un fichero que no existe
/// (una sola escritura, con copia de seguridad). Devuelve cuántos se quitaron.
#[tauri::command(async)]
pub fn clear_media_refs(app: tauri::AppHandle, system_path: String, refs: Vec<BrokenMediaRef>) -> Result<usize, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    if !layout.writes_media_tags() {
        return Ok(0);
    }
    let mut gamelist = super::gamelist::read_gamelist(&layout, &system)?;
    let cleared = clear_refs(&system, &mut gamelist, &refs);
    if cleared > 0 {
        super::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    }
    Ok(cleared)
}

fn clear_refs(system: &SystemRef, gamelist: &mut GameList, refs: &[BrokenMediaRef]) -> usize {
    let mut cleared = 0;
    for r in refs {
        let Some(game) = gamelist.games.iter_mut().find(|g| g.path == r.game_path) else { continue };
        // Solo si el campo sigue igual y el fichero sigue sin existir (por si algo cambió entre medias)
        let still_broken = media::get_field(game, &r.field)
            .is_some_and(|v| *v == r.value && !media::resolve_media_path(system, v).exists());
        if still_broken {
            media::set_field(game, &r.field, None);
            cleared += 1;
        }
    }
    cleared
}

/// Manda medios a la papelera (no usados o repetidos). Solo acepta ficheros de la carpeta del
/// sistema o de su carpeta de medios. En clásica vacía los campos de la gamelist que apuntaban
/// a ellos (con copia de seguridad).
#[tauri::command(async)]
pub fn trash_media(app: tauri::AppHandle, system_path: String, paths: Vec<String>) -> Result<TrashMediaResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let (allowed, mut failed) = allowed_media_paths(&layout, &system, &paths);

    // Todos de una vez (un solo sonido de papelera en macOS)
    let errors = media::trash_files(&allowed);
    failed.extend(errors.iter().map(|(f, e)| format!("{} ({})", f.display(), e)));
    let not_trashed: HashSet<&PathBuf> = errors.iter().map(|(f, _)| f).collect();
    let trashed: HashSet<PathBuf> = allowed.iter().filter(|f| !not_trashed.contains(f)).cloned().collect();

    if layout.writes_media_tags() && !trashed.is_empty() {
        let mut gamelist = super::gamelist::read_gamelist(&layout, &system)?;
        if clear_fields_pointing_to(&system, &mut gamelist, &trashed) {
            super::gamelist::write_gamelist(&layout, &system, &gamelist)?;
        }
    }
    Ok(TrashMediaResult { trashed: trashed.len(), failed })
}

/// Rutas que son ficheros dentro de la carpeta del sistema o de su carpeta de medios (ya
/// resueltas, así `../` o enlaces no se escapan); el resto, como fallo con el motivo.
fn allowed_media_paths(layout: &Layout, system: &SystemRef, paths: &[String]) -> (Vec<PathBuf>, Vec<String>) {
    let roots: Vec<PathBuf> = [system.rom_dir.clone(), layout.media_type_dir(system, "")]
        .into_iter()
        .filter_map(|r| r.canonicalize().ok())
        .collect();
    let mut ok = Vec::new();
    let mut rejected = Vec::new();
    for path in paths {
        match Path::new(path).canonicalize() {
            Ok(real) if real.is_file() && roots.iter().any(|r| real.starts_with(r)) => ok.push(real),
            _ => rejected.push(format!("{} (not a media file of this system)", path)),
        }
    }
    (ok, rejected)
}

/// Vacía los campos de medios que apuntan a ficheros quitados. Devuelve si cambió algo.
fn clear_fields_pointing_to(system: &SystemRef, gamelist: &mut GameList, removed: &HashSet<PathBuf>) -> bool {
    let mut changed = false;
    for game in &mut gamelist.games {
        for field in media::media_fields() {
            let points_to_removed = media::get_field(game, field)
                .map(|v| media::resolve_media_path(system, v))
                .and_then(|p| p.canonicalize().ok().or(Some(p)))
                .is_some_and(|p| removed.contains(&p));
            if points_to_removed {
                media::set_field(game, field, None);
                changed = true;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::EsdePaths;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn esde_game_leftovers_are_the_formats_es_de_does_not_show() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Esde(EsdePaths {
            home: tmp.path().join("ES-DE"),
            media_root: tmp.path().join("ES-DE/downloaded_media"),
        });
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        touch(&system.rom_dir.join("Juego.sfc"));
        let media = tmp.path().join("ES-DE/downloaded_media/snes");
        touch(&media.join("covers/Juego.png"));
        touch(&media.join("covers/Juego.jpg"));
        touch(&media.join("fanart/Juego.jpg"));
        touch(&media.join("covers/Otro.jpg"));

        let leftovers = game_leftovers_esde(&layout, &system, "./Juego.sfc");
        let names: Vec<_> = leftovers.iter().map(|u| u.relative.clone()).collect();
        assert_eq!(names, vec!["covers/Juego.jpg"]);
    }

    #[test]
    fn classic_game_leftovers_skip_used_files_and_other_roms() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("covers/Juego.png"));
        touch(&system.rom_dir.join("covers/Juego.jpg"));
        touch(&system.rom_dir.join("images/Juego-image.png"));
        touch(&system.rom_dir.join("images/Juego-cart.png"));
        touch(&system.rom_dir.join("images/Juego-thumb.png"));
        touch(&system.rom_dir.join("covers/Otro.png"));
        let gamelist = GameList {
            games: vec![
                crate::models::Game {
                    path: "./Juego.sfc".into(),
                    image: Some("./covers/Juego.png".into()),
                    thumbnail: Some("./images/Juego-thumb.png".into()),
                    ..Default::default()
                },
                // Another ROM whose name looks like "Juego" + a suffix
                crate::models::Game { path: "./Juego-cart.sfc".into(), ..Default::default() },
            ],
            ..Default::default()
        };

        let leftovers = game_leftovers_classic(&system, &gamelist, "./Juego.sfc");
        let mut names: Vec<_> = leftovers.iter().map(|u| u.relative.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["covers/Juego.jpg", "images/Juego-image.png"]);
    }

    #[test]
    fn esde_unused_means_no_rom_with_that_name() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Esde(EsdePaths {
            home: tmp.path().join("ES-DE"),
            media_root: tmp.path().join("ES-DE/downloaded_media"),
        });
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        // A ROM on disk without gamelist entry still "uses" its media
        touch(&system.rom_dir.join("Sin Entrada.sfc"));
        let media = tmp.path().join("ES-DE/downloaded_media/snes");
        touch(&media.join("covers/Sin Entrada.png"));
        touch(&media.join("covers/Huerfano.png"));
        touch(&media.join("fanart/Huerfano.jpg"));

        let unused = find_unused_esde(&layout, &system).unwrap();
        let mut names: Vec<_> = unused.iter().map(|u| u.relative.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["covers/Huerfano.png", "fanart/Huerfano.jpg"]);
    }

    #[test]
    fn classic_counts_every_media_field_as_used() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("fanart/Juego.png"));
        let gamelist = GameList {
            games: vec![crate::models::Game {
                path: "./Juego.sfc".into(),
                fanart: Some("./fanart/Juego.png".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        crate::commands::gamelist::write_gamelist(&Layout::Classic, &system, &gamelist).unwrap();

        let unused = find_unused_classic(&Layout::Classic, &system, system.rom_dir.to_str().unwrap()).unwrap();
        assert!(unused.is_empty(), "fanart is referenced, so it is not unused: {:?}", unused);
    }

    fn png(path: &Path, width: u32, height: u32, extra: u8) {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, extra]);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn esde(root: &Path) -> Layout {
        Layout::Esde(EsdePaths { home: root.join("ES-DE"), media_root: root.join("ES-DE/downloaded_media") })
    }

    fn game(path: &str) -> crate::models::Game {
        crate::models::Game { path: path.into(), name: Some(path.into()), ..Default::default() }
    }

    #[test]
    fn identical_cover_and_screenshot_keeps_the_cover_when_it_cannot_tell() {
        // Como cps1: el flyer está en covers/ y también en screenshots/ (idéntico)
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/cps1"));
        let media_dir = tmp.path().join("ES-DE/downloaded_media/cps1");
        let mut games = Vec::new();
        for name in ["a", "b", "c", "d"] {
            png(&media_dir.join(format!("covers/{name}.png")), 300, 420, 1);
            png(&media_dir.join(format!("screenshots/{name}.png")), 300, 420, 1);
            games.push(game(&format!("./{name}.zip")));
        }
        // Distinto contenido aunque mismo tamaño en bytes: no es repetido
        png(&media_dir.join("covers/e.png"), 300, 420, 1);
        png(&media_dir.join("screenshots/e.png"), 300, 420, 2);
        games.push(game("./e.zip"));

        let found = find_repeated(&layout, &system, &games, None);
        assert_eq!(found.len(), 4);
        assert!(found.iter().all(|r| r.keep_field == "image" && r.remove.len() == 1 && r.remove[0].field == "thumbnail"));
    }

    #[test]
    fn keeps_the_type_the_image_looks_like() {
        // Capturas habituales 256x224 y carátulas 640x468: una captura repetida como carátula
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/snes"));
        let media_dir = tmp.path().join("ES-DE/downloaded_media/snes");
        let mut games = Vec::new();
        for name in ["a", "b", "c"] {
            png(&media_dir.join(format!("covers/{name}.png")), 640, 468, 1);
            png(&media_dir.join(format!("screenshots/{name}.png")), 256, 224, 1);
            games.push(game(&format!("./{name}.sfc")));
        }
        png(&media_dir.join("covers/x.png"), 256, 224, 9);
        png(&media_dir.join("screenshots/x.png"), 256, 224, 9);
        games.push(game("./x.sfc"));

        let found = find_repeated(&layout, &system, &games, Some("./x.sfc"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].keep_field, "thumbnail");
        assert_eq!(found[0].remove[0].field, "image");
    }

    #[test]
    fn trash_only_accepts_media_of_this_system() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/snes"));
        touch(&tmp.path().join("ES-DE/downloaded_media/snes/covers/a.png"));
        touch(&system.rom_dir.join("images/b.png"));
        touch(&tmp.path().join("ES-DE/downloaded_media/nes/covers/c.png"));
        let p = |rel: &str| tmp.path().join(rel).to_string_lossy().to_string();

        let (ok, rejected) = allowed_media_paths(
            &layout,
            &system,
            &[p("ES-DE/downloaded_media/snes/covers/a.png"), p("roms/snes/images/b.png"), p("ES-DE/downloaded_media/nes/covers/c.png"), p("nope.png")],
        );
        assert_eq!(ok.len(), 2);
        assert_eq!(rejected.len(), 2);
    }

    #[test]
    fn classic_clears_the_fields_of_removed_media() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("cps1"));
        touch(&system.rom_dir.join("screenshots/a.png"));
        let removed: HashSet<PathBuf> = [system.rom_dir.join("screenshots/a.png").canonicalize().unwrap()].into();
        let mut g = game("./a.zip");
        g.image = Some("./covers/a.png".into());
        g.thumbnail = Some("./screenshots/a.png".into());
        let mut gamelist = GameList { games: vec![g], ..Default::default() };

        assert!(clear_fields_pointing_to(&system, &mut gamelist, &removed));
        assert_eq!(gamelist.games[0].thumbnail, None);
        assert_eq!(gamelist.games[0].image.as_deref(), Some("./covers/a.png"));
    }

    #[test]
    fn broken_refs_are_media_fields_pointing_to_missing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("covers/a.png"));
        let mut g = game("./a.sfc");
        g.image = Some("./covers/a.png".into());
        g.thumbnail = Some("./screenshots/a.png".into()); // no existe
        g.video = Some("https://ejemplo.invalid/a.mp4".into()); // remota: no cuenta
        let mut other = game("./b.sfc");
        other.marquee = Some("./marquees/b.png".into()); // no existe
        let mut gamelist = GameList { games: vec![g, other], ..Default::default() };

        let all = broken_refs(&system, &gamelist, None);
        let fields: Vec<_> = all.iter().map(|r| (r.game_path.as_str(), r.field.as_str())).collect();
        assert_eq!(fields, vec![("./a.sfc", "thumbnail"), ("./b.sfc", "marquee")]);
        assert_eq!(broken_refs(&system, &gamelist, Some("./b.sfc")).len(), 1);

        assert_eq!(clear_refs(&system, &mut gamelist, &all), 2);
        assert_eq!(gamelist.games[0].thumbnail, None);
        assert_eq!(gamelist.games[0].image.as_deref(), Some("./covers/a.png"));
        assert_eq!(gamelist.games[1].marquee, None);
    }

    #[test]
    fn a_ref_whose_file_appeared_is_not_cleared() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        let mut g = game("./a.sfc");
        g.image = Some("./covers/a.png".into());
        let mut gamelist = GameList { games: vec![g], ..Default::default() };
        let refs = broken_refs(&system, &gamelist, None);
        touch(&system.rom_dir.join("covers/a.png")); // aparece entre medias
        assert_eq!(clear_refs(&system, &mut gamelist, &refs), 0);
        assert!(gamelist.games[0].image.is_some());
    }
}
