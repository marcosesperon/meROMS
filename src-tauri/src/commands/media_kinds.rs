//! Medios mal clasificados: una carátula guardada como captura, una captura como carátula…
//!
//! - `swap_media` → intercambia dos tipos de medio en uno o varios juegos (mueve los ficheros
//!   de carpeta, con el nombre de la ROM; en clásica también actualiza las rutas del XML).
//!   Si solo existe uno de los dos, simplemente lo cambia de tipo.
//! - `detect_swapped_media` → mira la forma de carátulas y capturas y propone las que parecen
//!   estar al revés (carátula apaisada / captura vertical).

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::layout::{Layout, SystemRef};
use crate::media::{self, MediaIndex, MediaKind};
use crate::models::Game;

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

#[derive(Debug, Clone, Serialize)]
pub struct MediaFileInfo {
    pub path: String,
    pub width: u32,
    pub height: u32,
}

/// Tamaño habitual de un tipo de imagen en el sistema (el grupo más grande de su carpeta)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MediaProfile {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectResult {
    pub cover_profile: Option<MediaProfile>,
    pub screenshot_profile: Option<MediaProfile>,
    /// Lo habitual de covers/ parece una captura y lo de screenshots/ una carátula:
    /// el sistema entero está al revés (mejor «Intercambiar tipos de medio»)
    pub looks_swapped: bool,
    pub proposals: Vec<SwapProposal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SwapProposal {
    pub game_path: String,
    pub game_name: String,
    pub cover: Option<MediaFileInfo>,
    pub screenshot: Option<MediaFileInfo>,
}

#[tauri::command]
pub fn swap_media(
    app: tauri::AppHandle,
    system_path: String,
    game_paths: Vec<String>,
    field_a: String,
    field_b: String,
) -> Result<SwapResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let a = media::kind_for_field(&field_a).ok_or_else(|| format!("Unknown media field: {}", field_a))?;
    let b = media::kind_for_field(&field_b).ok_or_else(|| format!("Unknown media field: {}", field_b))?;
    if a == b {
        return Ok(SwapResult::default());
    }
    let mut gamelist = super::gamelist::read_gamelist(&layout, &system)?;
    let before = gamelist.clone();
    media::journal_begin();
    let result = swap_in(&layout, &system, &mut gamelist.games, &game_paths, a, b);
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
    });
    Ok(result)
}

#[tauri::command]
pub fn detect_swapped_media(app: tauri::AppHandle, system_path: String) -> Result<DetectResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let games = super::gamelist::read_gamelist(&layout, &system)?.games;
    Ok(detect(&layout, &system, &games))
}

fn swap_in(layout: &Layout, system: &SystemRef, games: &mut [Game], game_paths: &[String], a: &MediaKind, b: &MediaKind) -> SwapResult {
    let index = MediaIndex::build(layout, system);
    let mut result = SwapResult::default();
    for game in games.iter_mut().filter(|g| game_paths.contains(&g.path)) {
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

fn detect(layout: &Layout, system: &SystemRef, games: &[Game]) -> DetectResult {
    let (Some(cover_kind), Some(shot_kind)) = (media::kind_for_field("image"), media::kind_for_field("thumbnail")) else {
        return DetectResult { cover_profile: None, screenshot_profile: None, looks_swapped: false, proposals: Vec::new() };
    };
    let index = MediaIndex::build(layout, system);

    // Carátula y captura de cada juego, con su tamaño
    let pairs: Vec<(&Game, Option<MediaFileInfo>, Option<MediaFileInfo>)> = games
        .iter()
        .map(|g| {
            let cover = current_file(layout, system, &index, g, cover_kind).and_then(|p| info(&p));
            let shot = current_file(layout, system, &index, g, shot_kind).and_then(|p| info(&p));
            (g, cover, shot)
        })
        .collect();

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

    DetectResult {
        looks_swapped: whole_system_swapped(cover_profile, shot_profile),
        cover_profile,
        screenshot_profile: shot_profile,
        proposals,
    }
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
fn same_type(w1: u32, h1: u32, w2: u32, h2: u32) -> bool {
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
    let ratio = c.width as f64 / c.height.max(1) as f64;
    area(c) * 2.0 <= area(s) && (1.1..=1.5).contains(&ratio)
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

        let result = detect(&layout, &system, &games);
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

        let result = detect(&layout, &system, &games);
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
        let result = detect(&layout, &system, &games);
        assert!(result.looks_swapped);
        assert!(result.proposals.is_empty());
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
        assert!(detect(&layout, &system, &games).proposals.is_empty());
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

        let result = swap_in(&layout, &system, &mut games, &["./Alf.sms".into(), "./Sola.sms".into()], a, b);
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

        let result = swap_in(&layout, &system, &mut games, &["./Alf.sms".into()], a, b);
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

        let result = swap_in(&Layout::Classic, &system, &mut games, &["./Alf.sms".into()], a, b);
        assert_eq!(result.swapped, 1);
        assert_eq!(games[0].image.as_deref(), Some("./covers/Alf.png"));
        assert_eq!(games[0].thumbnail.as_deref(), Some("./screenshots/Alf.png"));
        assert_eq!(fs::read(system.rom_dir.join("covers/Alf.png")).unwrap(), b"caratula");
        assert_eq!(fs::read(system.rom_dir.join("screenshots/Alf.png")).unwrap(), b"captura");
        assert!(!system.rom_dir.join("images").exists());
    }
}
