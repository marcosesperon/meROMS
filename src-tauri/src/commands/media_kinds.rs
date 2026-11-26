//! Medios mal clasificados: una carátula guardada como captura, una captura como carátula…
//!
//! - `swap_media` → intercambia dos tipos de medio en uno o varios juegos (mueve los ficheros
//!   de carpeta, con el nombre de la ROM; en clásica también actualiza las rutas del XML).
//!   Si solo existe uno de los dos, simplemente lo cambia de tipo.
//! - `detect_swapped_media` → mira la forma de carátulas y capturas y propone las que parecen
//!   estar al revés (carátula apaisada / captura vertical).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::layout::{Layout, SystemRef};
use crate::media::{self, MediaIndex, MediaKind};
use crate::models::Game;

use super::progress::Progress;

/// Dos imágenes son «del mismo tipo» si su forma (ancho/alto) difiere menos de esto…
const SAME_SHAPE: f64 = 0.06;
/// …y una no tiene más de este múltiplo del área de la otra.
const SAME_AREA: f64 = 2.25;
/// Un grupo es «lo habitual» de una carpeta si tiene al menos estas imágenes…
const PROFILE_MIN_COUNT: usize = 3;
/// …y al menos esta parte de la carpeta.
const PROFILE_MIN_SHARE: f64 = 0.3;

#[derive(Debug, Clone, Default, Serialize)]
pub struct SwapResult {
    pub swapped: usize,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct MediaFileInfo {
    pub path: String,
    pub width: u32,
    pub height: u32,
}

/// Tamaño habitual de un tipo de imagen en el sistema (el grupo más grande de su carpeta)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MediaProfile {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct DetectResult {
    pub cover_profile: Option<MediaProfile>,
    pub screenshot_profile: Option<MediaProfile>,
    /// Lo habitual de covers/ parece una captura y lo de screenshots/ una carátula:
    /// el sistema entero está al revés (mejor «Intercambiar tipos de medio»)
    pub looks_swapped: bool,
    pub proposals: Vec<SwapProposal>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SwapProposal {
    pub game_path: String,
    pub game_name: String,
    pub cover: Option<MediaFileInfo>,
    pub screenshot: Option<MediaFileInfo>,
}

#[tauri::command(async)]
pub fn swap_media(
    app: tauri::AppHandle,
    system_path: String,
    game_paths: Vec<String>,
    field_a: String,
    field_b: String,
    task: Option<String>,
) -> Result<SwapResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    let a = media::kind_for_field(&field_a).ok_or_else(|| format!("Unknown media field: {}", field_a))?;
    let b = media::kind_for_field(&field_b).ok_or_else(|| format!("Unknown media field: {}", field_b))?;
    if a == b {
        return Ok(SwapResult::default());
    }
    let mut gamelist = super::gamelist::read_gamelist_with(&layout, &system, &progress)?;
    let before = gamelist.clone();
    media::journal_begin();
    let result = swap_in(&layout, &system, &mut gamelist.games, &game_paths, a, b, &progress);
    let files = media::journal_take();
    // En ES-DE la gamelist no guarda rutas de medios: no hay nada que reescribir
    if result.swapped > 0 && layout.writes_media_tags() {
        super::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    }
    // Para «Deshacer»
    let sys = system.rom_dir.to_string_lossy().to_string();
    super::undo::record(super::undo::UndoOp {
        kind: "swap".into(),
        system: sys.clone(),
        files,
        games: super::undo::changes_same_paths(&sys, &before, &gamelist),
        refs: super::undo::RefUndo::None,
        ..Default::default()
    });
    Ok(result)
}

#[tauri::command(async)]
pub fn detect_swapped_media(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<DetectResult, String> {
    let (a, sp) = (app.clone(), system_path.clone());
    super::system_status::remember(&app, &system_path, "swapped", "", move || detect_swapped_media_now(a, sp, task))
}

/// Lo que hace `detect_swapped_media` cuando no hay nada guardado que valga
fn detect_swapped_media_now(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<DetectResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    let games = super::gamelist::read_gamelist_with(&layout, &system, &progress)?.games;
    detect(&layout, &system, &games, &progress)
}

fn swap_in(layout: &Layout, system: &SystemRef, games: &mut [Game], game_paths: &[String], a: &MediaKind, b: &MediaKind, progress: &Progress) -> SwapResult {
    progress.phase("media");
    let index = MediaIndex::build(layout, system);
    let mut result = SwapResult::default();
    let wanted: std::collections::HashSet<&String> = game_paths.iter().collect();
    let total = games.iter().filter(|g| wanted.contains(&g.path)).count();
    for (i, game) in games.iter_mut().filter(|g| wanted.contains(&g.path)).enumerate() {
        // Cancelado: lo intercambiado hasta aquí se guarda (y Deshacer lo cubre)
        if progress.cancelled() {
            break;
        }
        progress.step("moving", i + 1, total, game.name.as_deref().unwrap_or(&game.path));
        match swap_one(layout, system, &index, game, a, b) {
            Ok(true) => result.swapped += 1,
            Ok(false) => {}
            Err(e) => result.skipped.push(format!("{} ({})", game.name.as_deref().unwrap_or(&game.path), e)),
        }
    }
    result
}

/// Fichero actual de un tipo para un juego: en clásica, el que apunta su campo (si existe);
/// si no, el que tiene el nombre unificado en la carpeta del tipo.
pub(super) fn current_file(layout: &Layout, system: &SystemRef, index: &MediaIndex, game: &Game, kind: &MediaKind) -> Option<PathBuf> {
    if layout.writes_media_tags() {
        if let Some(path) = media::get_field(game, kind.field).map(|v| media::resolve_media_path(system, v)) {
            if path.is_file() {
                return Some(path);
            }
        }
    }
    index.find(system, kind, &game.path).cloned()
}

/// Intercambia los ficheros de los tipos `a` y `b` de un juego. Ok(false) = no había nada.
fn swap_one(layout: &Layout, system: &SystemRef, index: &MediaIndex, game: &mut Game, a: &MediaKind, b: &MediaKind) -> Result<bool, String> {
    let file_a = current_file(layout, system, index, game, a);
    let file_b = current_file(layout, system, index, game, b);
    if file_a.is_none() && file_b.is_none() {
        return Ok(false);
    }
    if file_a.is_some() && file_a == file_b {
        return Err("same file in both".into());
    }

    // Adónde va cada uno: el de A a la carpeta de B y al revés, con su propia extensión
    let target = |file: &Path, kind: &MediaKind| {
        let ext = media::extension_lower(file).unwrap_or_else(|| "png".into());
        layout.media_file_path(system, kind.folder, &game.path, &ext)
    };
    let to_b = file_a.as_ref().map(|f| target(f, b));
    let to_a = file_b.as_ref().map(|f| target(f, a));

    // Un destino ocupado por un tercer fichero no se pisa
    if let Some(dest) = &to_b {
        if dest.exists() && Some(dest) != file_b.as_ref() {
            return Err(format!("{} already exists", dest.display()));
        }
    }
    if let Some(dest) = &to_a {
        if dest.exists() && Some(dest) != file_a.as_ref() {
            return Err(format!("{} already exists", dest.display()));
        }
    }

    // A se aparta con un nombre temporal para que B pueda ocupar su sitio si coinciden
    let parked = match &file_a {
        Some(from) => {
            let tmp = from.with_file_name(format!(
                "{}.meroms-swap",
                from.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
            ));
            media::move_file(from, &tmp)?;
            Some(tmp)
        }
        None => None,
    };
    if let (Some(from), Some(dest)) = (&file_b, &to_a) {
        if let Err(e) = media::move_file(from, dest) {
            // Deshacer lo de A para no dejarlo a medias
            if let (Some(tmp), Some(orig)) = (&parked, &file_a) {
                let _ = media::move_file(tmp, orig);
            }
            return Err(e);
        }
    }
    if let (Some(tmp), Some(dest)) = (&parked, &to_b) {
        media::move_file(tmp, dest)?;
    }

    media::set_field(game, a.field, to_a.as_ref().map(|p| media::media_ref(layout, system, p)));
    media::set_field(game, b.field, to_b.as_ref().map(|p| media::media_ref(layout, system, p)));

    // Carpetas antiguas que se han quedado vacías (p. ej. images/ en clásica)
    for old in file_a.iter().chain(file_b.iter()) {
        if let Some(dir) = old.parent() {
            if dir.starts_with(&system.rom_dir) {
                media::remove_empty_dirs_up_to(dir, &system.rom_dir);
            }
        }
    }
    Ok(true)
}

/// Qué parece una imagen comparada con lo habitual del sistema
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Looks {
    Cover,
    Screenshot,
    Unknown,
}

fn detect(layout: &Layout, system: &SystemRef, games: &[Game], progress: &Progress) -> Result<DetectResult, String> {
    let (Some(cover_kind), Some(shot_kind)) = (media::kind_for_field("image"), media::kind_for_field("thumbnail")) else {
        return Ok(DetectResult { cover_profile: None, screenshot_profile: None, looks_swapped: false, proposals: Vec::new() });
    };
    progress.phase("media");
    let index = MediaIndex::build(layout, system);

    // Carátula y captura de cada juego, con su tamaño
    let mut pairs: Vec<(&Game, Option<MediaFileInfo>, Option<MediaFileInfo>)> = Vec::with_capacity(games.len());
    for (i, g) in games.iter().enumerate() {
        progress.check()?;
        progress.step("images", i + 1, games.len(), g.name.as_deref().unwrap_or(&g.path));
        let cover = current_file(layout, system, &index, g, cover_kind).and_then(|p| info(&p));
        let shot = current_file(layout, system, &index, g, shot_kind).and_then(|p| info(&p));
        pairs.push((g, cover, shot));
    }

    // Lo habitual de cada carpeta en este sistema
    let cover_profile = profile(pairs.iter().filter_map(|(_, c, _)| c.as_ref()));
    let shot_profile = profile(pairs.iter().filter_map(|(_, _, s)| s.as_ref()));
    // Si las dos carpetas tienen lo mismo, no hay forma de distinguir nada
    let distinguishable = match (cover_profile, shot_profile) {
        (Some(c), Some(s)) => !same_type(c.width, c.height, s.width, s.height),
        _ => true,
    };

    let classify = |i: &MediaFileInfo| looks_like(i, cover_profile, shot_profile);

    let mut proposals = Vec::new();
    if distinguishable {
        for (game, cover, shot) in pairs {
            let c = cover.as_ref().map(&classify);
            let s = shot.as_ref().map(&classify);
            // Lo que hay está al revés y lo otro también (o no existe). Lo dudoso, nunca.
            let propose = (c == Some(Looks::Screenshot) && matches!(s, None | Some(Looks::Cover)))
                || (s == Some(Looks::Cover) && matches!(c, None | Some(Looks::Screenshot)));
            if propose {
                proposals.push(SwapProposal {
                    game_path: game.path.clone(),
                    game_name: game.name.clone().unwrap_or_else(|| game.path.clone()),
                    cover,
                    screenshot: shot,
                });
            }
        }
    }

    // Todo al revés: se arregla intercambiando el sistema entero, no juego a juego (lo de cada
    // carpeta se ha comparado con lo habitual, que también está al revés)
    let looks_swapped = whole_system_swapped(cover_profile, shot_profile);
    if looks_swapped {
        proposals.clear();
    }
    Ok(DetectResult {
        looks_swapped,
        cover_profile,
        screenshot_profile: shot_profile,
        proposals,
    })
}

/// Tamaños habituales de carátula y de captura en un sistema (también lo usa «Limpiar Media»
/// para decidir cuál de dos ficheros idénticos se queda).
pub(super) fn usual_sizes(layout: &Layout, system: &SystemRef, games: &[Game]) -> (Option<MediaProfile>, Option<MediaProfile>) {
    let (Some(cover_kind), Some(shot_kind)) = (media::kind_for_field("image"), media::kind_for_field("thumbnail")) else {
        return (None, None);
    };
    let index = MediaIndex::build(layout, system);
    let covers: Vec<MediaFileInfo> = games.iter().filter_map(|g| current_file(layout, system, &index, g, cover_kind).and_then(|p| info(&p))).collect();
    let shots: Vec<MediaFileInfo> = games.iter().filter_map(|g| current_file(layout, system, &index, g, shot_kind).and_then(|p| info(&p))).collect();
    (profile(covers.iter()), profile(shots.iter()))
}

/// La forma que más se repite en un medio (campo de la gamelist: `image`, `thumbnail`…) del
/// sistema, para dar esa forma a las tarjetas de la cuadrícula. `None` si no hay bastantes
/// imágenes de ese tipo (o es un vídeo/PDF): entonces la cuadrícula usa su tabla fija.
///
/// Medirla lee la cabecera de cada imagen, y con miles de juegos en un disco externo o en red
/// eso tarda. Por eso se guarda en `media_shape.json` (carpeta de datos de la app) con la
/// «firma» de la carpeta de ese medio: mientras la firma no cambie, se devuelve lo guardado sin
/// abrir ninguna imagen. `force` la vuelve a medir (tras scrapear o cambiar medios: un fichero
/// sobrescrito con el mismo nombre no cambia la firma).
#[tauri::command(async)]
pub fn usual_media_shape(app: tauri::AppHandle, system_path: String, field: String, force: Option<bool>) -> Result<Option<MediaProfile>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let Some(kind) = media::kind_for_field(&field) else { return Ok(None) };
    let cache = app.path().app_data_dir().ok().map(|d| d.join("media_shape.json"));
    cached_shape(cache.as_deref(), &layout, &system, kind, force.unwrap_or(false), || {
        let games = super::gamelist::read_gamelist(&layout, &system)?.games;
        Ok(usual_shape(&layout, &system, &games, kind))
    })
}

/// Formas guardadas, por carpeta del medio (ruta absoluta: vale para cualquier perfil)
type ShapeCache = HashMap<String, CachedShape>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedShape {
    signature: FolderSignature,
    shape: Option<MediaProfile>,
}

/// Lo que cambia cuando se añaden, quitan o renombran medios: cuántas entradas tiene la carpeta y
/// su fecha. En clásica cuenta también la fecha del gamelist (los medios salen de sus rutas).
/// Mirarla es una sola lectura de carpeta, no una por imagen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FolderSignature {
    entries: usize,
    modified: u64,
    gamelist: Option<u64>,
}

/// Que dos medidas a la vez no se pisen el fichero
static SHAPE_LOCK: Mutex<()> = Mutex::new(());

fn modified_secs(path: &Path) -> Option<u64> {
    let time = fs::metadata(path).ok()?.modified().ok()?;
    time.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs())
}

fn signature(layout: &Layout, system: &SystemRef, dir: &Path) -> FolderSignature {
    FolderSignature {
        entries: fs::read_dir(dir).map(|it| it.count()).unwrap_or(0),
        modified: modified_secs(dir).unwrap_or(0),
        gamelist: if layout.writes_media_tags() { modified_secs(&layout.gamelist_path(system)) } else { None },
    }
}

/// Lo guardado si la firma de la carpeta no ha cambiado; si no (o con `force`), mide y lo guarda.
/// Sin fichero de caché (tests sin carpeta de datos), siempre mide.
fn cached_shape(
    cache_file: Option<&Path>,
    layout: &Layout,
    system: &SystemRef,
    kind: &MediaKind,
    force: bool,
    measure: impl FnOnce() -> Result<Option<MediaProfile>, String>,
) -> Result<Option<MediaProfile>, String> {
    let Some(cache_file) = cache_file else { return measure() };
    let dir = layout.media_type_dir(system, kind.folder);
    let key = dir.to_string_lossy().to_string();
    let sig = signature(layout, system, &dir);
    let read = || -> ShapeCache { fs::read_to_string(cache_file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default() };
    if !force {
        if let Some(saved) = read().get(&key).filter(|c| c.signature == sig) {
            return Ok(saved.shape);
        }
    }
    let shape = measure()?;
    let _guard = SHAPE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = read();
    all.insert(key, CachedShape { signature: sig, shape });
    // Si no se puede guardar, no pasa nada: la próxima vez se vuelve a medir
    if let Some(parent) = cache_file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(&all) {
        let _ = fs::write(cache_file, text);
    }
    Ok(shape)
}

fn usual_shape(layout: &Layout, system: &SystemRef, games: &[Game], kind: &MediaKind) -> Option<MediaProfile> {
    let index = MediaIndex::build(layout, system);
    let images: Vec<MediaFileInfo> = games.iter().filter_map(|g| current_file(layout, system, &index, g, kind).and_then(|p| info(&p))).collect();
    profile(images.iter())
}

/// Qué parece una imagen comparada con los tamaños habituales («no se sabe» si encaja con
/// los dos o con ninguno)
pub(super) fn looks_like(i: &MediaFileInfo, cover: Option<MediaProfile>, shot: Option<MediaProfile>) -> Looks {
    let is_cover = cover.is_some_and(|p| same_type(i.width, i.height, p.width, p.height));
    let is_shot = shot.is_some_and(|p| same_type(i.width, i.height, p.width, p.height));
    match (is_cover, is_shot) {
        (true, false) => Looks::Cover,
        (false, true) => Looks::Screenshot,
        _ => Looks::Unknown,
    }
}

/// ¿Dos tamaños son del mismo tipo de imagen? (misma forma y área parecida)
pub(super) fn same_type(w1: u32, h1: u32, w2: u32, h2: u32) -> bool {
    let ratio = |w: u32, h: u32| w as f64 / h.max(1) as f64;
    let shape = (ratio(w1, h1) / ratio(w2, h2) - 1.0).abs() <= SAME_SHAPE;
    let (a1, a2) = ((w1 as f64) * (h1 as f64), (w2 as f64) * (h2 as f64));
    let area = a1.max(a2) / a1.min(a2).max(1.0) <= SAME_AREA;
    shape && area
}

/// Lo habitual de una carpeta: se agrupan los tamaños parecidos y gana el grupo más grande
/// (si es lo bastante grande). Su representante es el tamaño exacto más repetido del grupo.
fn profile<'a>(images: impl Iterator<Item = &'a MediaFileInfo>) -> Option<MediaProfile> {
    // Tamaños exactos y cuántas veces salen, de más a menos repetidos
    let mut sizes: Vec<((u32, u32), usize)> = Vec::new();
    let mut total = 0;
    for img in images {
        total += 1;
        match sizes.iter_mut().find(|(s, _)| *s == (img.width, img.height)) {
            Some((_, n)) => *n += 1,
            None => sizes.push(((img.width, img.height), 1)),
        }
    }
    sizes.sort_by(|a, b| b.1.cmp(&a.1));

    // Grupos: cada tamaño se une al primer grupo cuyo representante sea del mismo tipo
    let mut groups: Vec<((u32, u32), usize)> = Vec::new();
    for ((w, h), n) in sizes {
        match groups.iter_mut().find(|((gw, gh), _)| same_type(w, h, *gw, *gh)) {
            Some((_, count)) => *count += n,
            None => groups.push(((w, h), n)),
        }
    }
    let ((width, height), count) = groups.into_iter().max_by_key(|(_, n)| *n)?;
    (count >= PROFILE_MIN_COUNT && count as f64 >= total as f64 * PROFILE_MIN_SHARE).then_some(MediaProfile { width, height })
}

/// El sistema entero parece al revés: lo habitual de covers/ es bastante más pequeño que lo
/// de screenshots/ y tiene forma de pantalla (apaisada, entre 1,1 y 1,5).
fn whole_system_swapped(cover: Option<MediaProfile>, shot: Option<MediaProfile>) -> bool {
    let (Some(c), Some(s)) = (cover, shot) else { return false };
    let area = |p: MediaProfile| p.width as f64 * p.height as f64;
    let ratio = |p: MediaProfile| p.width as f64 / p.height.max(1) as f64;
    // La «portada» tiene forma de pantalla 4:3 y es bastante más pequeña que la «captura»
    let screen_as_cover = area(c) * 2.0 <= area(s) && (1.1..=1.5).contains(&ratio(c));
    // La «portada» es claramente apaisada y la «captura» claramente vertical, con forma de caja
    // (MSX: 480×250 y 484×680). Una caja apaisada (SNES PAL, N64) va con capturas apaisadas, y
    // las capturas verticales (recreativas de pantalla vertical) van con flyers verticales
    let wide_cover_tall_shot = ratio(c) >= 1.15 && ratio(s) <= 0.87;
    screen_as_cover || wide_cover_tall_shot
}

/// Tamaño de una imagen leyendo solo su cabecera. None si no se puede leer.
pub(super) fn info(path: &Path) -> Option<MediaFileInfo> {
    let size = imagesize::size(path).ok()?;
    Some(MediaFileInfo {
        path: path.to_string_lossy().to_string(),
        width: size.width as u32,
        height: size.height as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// PNG mínimo: solo la cabecera con el tamaño (es lo único que se lee)
    fn png(path: &Path, width: u32, height: u32) {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn game(path: &str) -> Game {
        Game { path: path.into(), name: Some(path.into()), ..Default::default() }
    }

    fn esde(root: &Path) -> Layout {
        Layout::Esde(crate::layout::EsdePaths { home: root.join("ES-DE"), media_root: root.join("ES-DE/downloaded_media") })
    }

    fn kinds() -> (&'static MediaKind, &'static MediaKind) {
        (media::kind_for_field("image").unwrap(), media::kind_for_field("thumbnail").unwrap())
    }

    fn found(result: &DetectResult) -> Vec<&str> {
        result.proposals.iter().map(|p| p.game_path.as_str()).collect()
    }

    #[test]
    fn la_forma_de_la_tarjeta_es_la_que_mas_se_repite() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/n64"));
        let media = tmp.path().join("ES-DE/downloaded_media/n64");
        let (cover, shot) = kinds();
        let mut games = Vec::new();
        // Tres carátulas europeas apaisadas (de tamaños parecidos) y una americana vertical
        for (name, w, h) in [("A", 640, 470), ("B", 642, 468), ("C", 640, 471), ("D", 300, 420)] {
            png(&media.join(format!("covers/{name}.png")), w, h);
            games.push(game(&format!("./{name}.z64")));
        }
        let shape = usual_shape(&layout, &system, &games, cover).unwrap();
        assert!(shape.width > shape.height, "apaisada: {:?}", (shape.width, shape.height));
        // Sin capturas no hay forma: la cuadrícula usa la tabla
        assert!(usual_shape(&layout, &system, &games, shot).is_none());
    }

    #[test]
    fn la_forma_guardada_se_usa_mientras_la_carpeta_no_cambia() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/n64"));
        let covers = tmp.path().join("ES-DE/downloaded_media/n64/covers");
        let cache = tmp.path().join("data/media_shape.json");
        let (cover, _) = kinds();
        png(&covers.join("A.png"), 300, 420);
        let measures = std::cell::Cell::new(0);
        let shape = |force: bool| {
            cached_shape(Some(&cache), &layout, &system, cover, force, || {
                measures.set(measures.get() + 1);
                Ok(Some(MediaProfile { width: 300, height: 420 }))
            })
            .unwrap()
        };
        // La primera vez mide; la segunda sale de lo guardado
        assert_eq!(shape(false), Some(MediaProfile { width: 300, height: 420 }));
        assert_eq!(shape(false), Some(MediaProfile { width: 300, height: 420 }));
        assert_eq!(measures.get(), 1);
        // Con «force», mide aunque la carpeta sea la misma
        shape(true);
        assert_eq!(measures.get(), 2);
        // Un fichero más en la carpeta cambia la firma: vuelve a medir
        png(&covers.join("B.png"), 300, 420);
        shape(false);
        assert_eq!(measures.get(), 3);
    }

    #[test]
    fn snes_con_caratulas_apaisadas_solo_propone_el_cambiado() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/snes"));
        let media = tmp.path().join("ES-DE/downloaded_media/snes");
        let mut games = Vec::new();
        for name in ["A", "B", "C", "D"] {
            png(&media.join(format!("covers/{name}.png")), 640, 468);
            png(&media.join(format!("screenshots/{name}.png")), 256, 224);
            games.push(game(&format!("./{name}.sfc")));
        }
        // Carátula sin captura: está bien, no se toca (antes se proponía)
        png(&media.join("covers/Sola.png"), 640, 471);
        games.push(game("./Sola.sfc"));
        // Este sí está al revés
        png(&media.join("covers/Mal.png"), 256, 224);
        png(&media.join("screenshots/Mal.png"), 640, 468);
        games.push(game("./Mal.sfc"));
        // Captura enorme: dudosa, no se propone
        png(&media.join("covers/Grande.png"), 640, 468);
        png(&media.join("screenshots/Grande.png"), 2100, 1551);
        games.push(game("./Grande.sfc"));

        let result = detect(&layout, &system, &games, &Progress::none()).unwrap();
        assert_eq!(result.cover_profile, Some(MediaProfile { width: 640, height: 468 }));
        assert_eq!(result.screenshot_profile, Some(MediaProfile { width: 256, height: 224 }));
        assert!(!result.looks_swapped);
        assert_eq!(found(&result), vec!["./Mal.sfc"]);
    }

    #[test]
    fn master_system_con_tamanos_de_captura_parecidos() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/mastersystem"));
        let media = tmp.path().join("ES-DE/downloaded_media/mastersystem");
        let mut games = Vec::new();
        for (i, shot) in [(256, 192), (248, 192), (256, 192), (248, 192)].into_iter().enumerate() {
            png(&media.join(format!("covers/J{i}.png")), 484, 680);
            png(&media.join(format!("screenshots/J{i}.png")), shot.0, shot.1);
            games.push(game(&format!("./J{i}.sms")));
        }
        png(&media.join("covers/Alf.png"), 248, 192);
        png(&media.join("screenshots/Alf.png"), 484, 680);
        games.push(game("./Alf.sms"));
        // Solo captura, pero con forma de carátula: se propone
        png(&media.join("screenshots/Sola.png"), 456, 640);
        games.push(game("./Sola.sms"));

        let result = detect(&layout, &system, &games, &Progress::none()).unwrap();
        assert_eq!(found(&result), vec!["./Alf.sms", "./Sola.sms"]);
    }

    #[test]
    fn sistema_entero_al_reves_avisa() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/mastersystem"));
        let media = tmp.path().join("ES-DE/downloaded_media/mastersystem");
        let mut games = Vec::new();
        for name in ["A", "B", "C", "D"] {
            png(&media.join(format!("covers/{name}.png")), 256, 192);
            png(&media.join(format!("screenshots/{name}.png")), 484, 680);
            games.push(game(&format!("./{name}.sms")));
        }
        let result = detect(&layout, &system, &games, &Progress::none()).unwrap();
        assert!(result.looks_swapped);
        assert!(result.proposals.is_empty());
    }

    #[test]
    fn portadas_apaisadas_y_capturas_verticales_es_todo_al_reves() {
        let profile = |width, height| Some(MediaProfile { width, height });
        // MSX: «portadas» de 480×250 y «capturas» con forma de caja
        assert!(whole_system_swapped(profile(480, 250), profile(484, 680)));
        // Normal: caja vertical y captura 4:3, o caja apaisada (SNES PAL) con captura apaisada
        assert!(!whole_system_swapped(profile(484, 680), profile(320, 224)));
        assert!(!whole_system_swapped(profile(640, 456), profile(256, 224)));
        // Recreativa de pantalla vertical con flyer vertical
        assert!(!whole_system_swapped(profile(500, 700), profile(224, 288)));
    }

    #[test]
    fn mismo_perfil_en_las_dos_carpetas_no_propone_nada() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/psx"));
        let media = tmp.path().join("ES-DE/downloaded_media/psx");
        let mut games = Vec::new();
        for name in ["A", "B", "C"] {
            png(&media.join(format!("covers/{name}.png")), 640, 480);
            png(&media.join(format!("screenshots/{name}.png")), 640, 480);
            games.push(game(&format!("./{name}.cue")));
        }
        assert!(detect(&layout, &system, &games, &Progress::none()).unwrap().proposals.is_empty());
    }

    #[test]
    fn intercambia_los_dos_ficheros_y_mueve_si_solo_hay_uno() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/mastersystem"));
        let media = tmp.path().join("ES-DE/downloaded_media/mastersystem");
        fs::create_dir_all(media.join("covers")).unwrap();
        fs::create_dir_all(media.join("screenshots")).unwrap();
        fs::write(media.join("covers/Alf.png"), b"captura").unwrap();
        fs::write(media.join("screenshots/Alf.png"), b"caratula").unwrap();
        fs::write(media.join("screenshots/Sola.jpg"), b"caratula sola").unwrap();
        let mut games = vec![game("./Alf.sms"), game("./Sola.sms")];
        let (a, b) = kinds();

        let result = swap_in(&layout, &system, &mut games, &["./Alf.sms".into(), "./Sola.sms".into()], a, b, &Progress::none());
        assert_eq!(result.swapped, 2);
        assert_eq!(fs::read(media.join("covers/Alf.png")).unwrap(), b"caratula");
        assert_eq!(fs::read(media.join("screenshots/Alf.png")).unwrap(), b"captura");
        assert_eq!(fs::read(media.join("covers/Sola.jpg")).unwrap(), b"caratula sola");
        assert!(!media.join("screenshots/Sola.jpg").exists());
    }

    #[test]
    fn destino_ocupado_por_otro_fichero_se_salta() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/mastersystem"));
        let media = tmp.path().join("ES-DE/downloaded_media/mastersystem");
        fs::create_dir_all(media.join("covers")).unwrap();
        fs::create_dir_all(media.join("screenshots")).unwrap();
        fs::write(media.join("covers/Alf.jpg"), b"a").unwrap();
        // screenshots tiene Alf.png (el que se usa) y también Alf.jpg: el jpg de covers no cabe
        fs::write(media.join("screenshots/Alf.png"), b"b").unwrap();
        fs::write(media.join("screenshots/Alf.jpg"), b"otro").unwrap();
        let mut games = vec![game("./Alf.sms")];
        let (a, b) = kinds();

        let result = swap_in(&layout, &system, &mut games, &["./Alf.sms".into()], a, b, &Progress::none());
        assert_eq!(result.swapped, 0);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(fs::read(media.join("covers/Alf.jpg")).unwrap(), b"a");
        assert_eq!(fs::read(media.join("screenshots/Alf.jpg")).unwrap(), b"otro");
    }

    #[test]
    fn en_clasica_actualiza_las_rutas_y_recoge_los_nombres_antiguos() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("mastersystem"));
        fs::create_dir_all(system.rom_dir.join("images")).unwrap();
        fs::write(system.rom_dir.join("images/Alf-image.png"), b"captura").unwrap();
        fs::write(system.rom_dir.join("images/Alf-thumb.png"), b"caratula").unwrap();
        let mut g = game("./Alf.sms");
        g.image = Some("./images/Alf-image.png".into());
        g.thumbnail = Some("./images/Alf-thumb.png".into());
        let mut games = vec![g];
        let (a, b) = kinds();

        let result = swap_in(&Layout::Classic, &system, &mut games, &["./Alf.sms".into()], a, b, &Progress::none());
        assert_eq!(result.swapped, 1);
        assert_eq!(games[0].image.as_deref(), Some("./covers/Alf.png"));
        assert_eq!(games[0].thumbnail.as_deref(), Some("./screenshots/Alf.png"));
        assert_eq!(fs::read(system.rom_dir.join("covers/Alf.png")).unwrap(), b"caratula");
        assert_eq!(fs::read(system.rom_dir.join("screenshots/Alf.png")).unwrap(), b"captura");
        assert!(!system.rom_dir.join("images").exists());
    }
}

