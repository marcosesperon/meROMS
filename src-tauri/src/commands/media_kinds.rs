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

/// Ancho/alto a partir del cual una imagen es claramente apaisada (una captura).
const LANDSCAPE_RATIO: f64 = 1.2;
/// Ancho/alto por debajo del cual es vertical o casi cuadrada (una carátula, también de CD).
const COVER_RATIO: f64 = 1.1;

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
    let result = swap_in(&layout, &system, &mut gamelist.games, &game_paths, a, b);
    // En ES-DE la gamelist no guarda rutas de medios: no hay nada que reescribir
    if result.swapped > 0 && layout.writes_media_tags() {
        super::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    }
    Ok(result)
}

#[tauri::command]
pub fn detect_swapped_media(app: tauri::AppHandle, system_path: String) -> Result<Vec<SwapProposal>, String> {
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
fn current_file(layout: &Layout, system: &SystemRef, index: &MediaIndex, game: &Game, kind: &MediaKind) -> Option<PathBuf> {
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

fn detect(layout: &Layout, system: &SystemRef, games: &[Game]) -> Vec<SwapProposal> {
    let (Some(cover_kind), Some(shot_kind)) = (media::kind_for_field("image"), media::kind_for_field("thumbnail")) else {
        return Vec::new();
    };
    let index = MediaIndex::build(layout, system);
    let mut out = Vec::new();

    for game in games {
        let cover = current_file(layout, system, &index, game, cover_kind).and_then(|p| info(&p));
        let shot = current_file(layout, system, &index, game, shot_kind).and_then(|p| info(&p));
        let ratio = |i: &MediaFileInfo| i.width as f64 / i.height.max(1) as f64;
        let cover_wrong = cover.as_ref().is_some_and(|c| ratio(c) >= LANDSCAPE_RATIO);
        let shot_wrong = shot.as_ref().is_some_and(|s| ratio(s) <= COVER_RATIO);

        // Solo si lo que hay está al revés y lo otro no está bien: si la captura es una
        // captura correcta, no se convierte en carátula
        let propose = (cover_wrong && (shot.is_none() || shot_wrong)) || (shot_wrong && (cover.is_none() || cover_wrong));
        if propose {
            out.push(SwapProposal {
                game_path: game.path.clone(),
                game_name: game.name.clone().unwrap_or_else(|| game.path.clone()),
                cover,
                screenshot: shot,
            });
        }
    }
    out
}

/// Tamaño de una imagen leyendo solo su cabecera. None si no se puede leer.
fn info(path: &Path) -> Option<MediaFileInfo> {
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

    #[test]
    fn detecta_caratula_y_captura_al_reves() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/mastersystem"));
        let media = tmp.path().join("ES-DE/downloaded_media/mastersystem");
        // Cambiadas (como en la biblioteca del usuario)
        png(&media.join("covers/Alf.png"), 256, 192);
        png(&media.join("screenshots/Alf.png"), 484, 680);
        // Correctas
        png(&media.join("covers/Bien.png"), 484, 680);
        png(&media.join("screenshots/Bien.png"), 256, 192);
        // Carátula apaisada pero la captura también es una captura: no se toca
        png(&media.join("covers/Raro.png"), 256, 192);
        png(&media.join("screenshots/Raro.png"), 256, 192);
        // Captura vertical sin carátula: se propone
        png(&media.join("screenshots/Sola.png"), 484, 680);

        let games = vec![game("./Alf.sms"), game("./Bien.sms"), game("./Raro.sms"), game("./Sola.sms")];
        let found: Vec<_> = detect(&layout, &system, &games).into_iter().map(|p| p.game_path).collect();
        assert_eq!(found, vec!["./Alf.sms", "./Sola.sms"]);
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
