//! Media dañada: ficheros vacíos, que no son lo que dice su extensión (una página de error
//! guardada como .png) o cortados por una descarga a medias.
//!
//! Se mira solo la estructura, sin decodificar: la cabecera, el final del fichero y, en los
//! MP4, la lista de bloques del nivel superior (saltando de uno en uno, sin leerlos).

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::Serialize;

use crate::media::{self, MediaIndex};

use super::progress::Progress;

/// Por qué un fichero de media está mal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Damage {
    /// 0 bytes
    Empty,
    /// No empieza como su formato (HTML de error, otro formato…)
    NotMedia,
    /// Le falta el final (descarga a medias)
    Truncated,
}

/// Cuánto del final se mira para buscar la marca de fin (algunos ficheros llevan relleno detrás)
const TAIL: u64 = 1024;

fn read_at(file: &mut File, offset: u64, len: usize) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = vec![0u8; len];
    let mut read = 0;
    while read < len {
        match file.read(&mut buf[read..]).ok()? {
            0 => break,
            n => read += n,
        }
    }
    buf.truncate(read);
    Some(buf)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Comprueba un fichero de media. None = está bien (o su formato no se comprueba).
pub fn check_file(path: &Path) -> Option<Damage> {
    let len = fs::metadata(path).ok()?.len();
    if len == 0 {
        return Some(Damage::Empty);
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let mut file = File::open(path).ok()?;
    let head = read_at(&mut file, 0, 16)?;
    let tail_start = len.saturating_sub(TAIL);
    let tail = read_at(&mut file, tail_start, (len - tail_start) as usize)?;
    // Una página web o un XML de error guardado con nombre de imagen o vídeo
    let looks_like_text = head.first().is_some_and(|b| *b == b'<') || head.starts_with(b"\xEF\xBB\xBF<");

    let riff = |kind: &[u8]| -> Option<Damage> {
        if head.len() < 12 || &head[0..4] != b"RIFF" || &head[8..12] != kind {
            return Some(Damage::NotMedia);
        }
        let declared = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as u64 + 8;
        (declared > len).then_some(Damage::Truncated)
    };

    match ext.as_str() {
        "png" => {
            if !head.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Some(Damage::NotMedia);
            }
            (!contains(&tail, b"IEND")).then_some(Damage::Truncated)
        }
        "jpg" | "jpeg" => {
            if !head.starts_with(&[0xFF, 0xD8, 0xFF]) {
                return Some(Damage::NotMedia);
            }
            (!contains(&tail, &[0xFF, 0xD9])).then_some(Damage::Truncated)
        }
        "gif" => {
            if !head.starts_with(b"GIF8") {
                return Some(Damage::NotMedia);
            }
            // El último byte que no es relleno tiene que ser el «;» de fin
            (tail.iter().rev().find(|b| **b != 0) != Some(&b';')).then_some(Damage::Truncated)
        }
        "webp" => riff(b"WEBP"),
        "avi" => riff(b"AVI "),
        "pdf" => {
            if looks_like_text || !contains(&head, b"%PDF") {
                return Some(Damage::NotMedia);
            }
            (!contains(&tail, b"%%EOF")).then_some(Damage::Truncated)
        }
        "webm" | "mkv" => (!head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3])).then_some(Damage::NotMedia),
        "mp4" | "m4v" | "mov" => check_mp4(&mut file, len, ext == "mov"),
        // Formatos que no se comprueban: solo se descarta texto con extensión de media
        _ => None,
    }
}

/// MP4/MOV: los bloques del nivel superior tienen que cubrir el fichero justo, el primero
/// tiene que ser `ftyp` (en MOV puede faltar) y tiene que haber `moov` (el índice).
fn check_mp4(file: &mut File, len: u64, is_mov: bool) -> Option<Damage> {
    let mut offset = 0u64;
    let mut first = true;
    let mut has_moov = false;
    while offset < len {
        let Some(header) = read_at(file, offset, 16) else { return Some(Damage::Truncated) };
        if header.len() < 8 {
            return Some(Damage::Truncated);
        }
        let size32 = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let kind = &header[4..8];
        if first && kind != b"ftyp" && !is_mov {
            return Some(Damage::NotMedia);
        }
        if !kind.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
            return Some(if first { Damage::NotMedia } else { Damage::Truncated });
        }
        first = false;
        if kind == b"moov" {
            has_moov = true;
        }
        let size = match size32 {
            0 => len - offset, // hasta el final
            1 if header.len() >= 16 => u64::from_be_bytes(header[8..16].try_into().ok()?),
            1 => return Some(Damage::Truncated),
            n => n,
        };
        if size < 8 || offset + size > len {
            return Some(Damage::Truncated);
        }
        offset += size;
    }
    (!has_moov).then_some(Damage::Truncated)
}

/// Un fichero de media dañado de un juego
#[derive(Debug, Clone, Serialize)]
pub struct DamagedMedia {
    pub path: String,
    pub field: String,
    pub game_path: String,
    pub game_name: String,
    pub damage: Damage,
    pub size: u64,
    /// Tipos del scraper que rellenan ese campo (para volver a descargarlo)
    pub scraper_types: Vec<String>,
}

/// Media dañada del sistema (o de un juego, con `game_path`)
#[tauri::command(async)]
pub fn find_damaged_media(app: tauri::AppHandle, system_path: String, game_path: Option<String>, task: Option<String>) -> Result<Vec<DamagedMedia>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    let games = super::gamelist::read_gamelist_with(&layout, &system, &progress)?.games;
    progress.phase("media");
    let index = MediaIndex::build(&layout, &system);
    let mut out = Vec::new();
    let wanted: Vec<_> = games.iter().filter(|g| game_path.as_ref().is_none_or(|p| p == &g.path)).collect();
    for (i, game) in wanted.iter().enumerate() {
        progress.check()?;
        for kind in media::MEDIA_KINDS {
            let Some(path) = super::media_kinds::current_file(&layout, &system, &index, game, kind) else { continue };
            progress.step("checking", i + 1, wanted.len(), &path.to_string_lossy());
            let Some(damage) = check_file(&path) else { continue };
            out.push(DamagedMedia {
                path: path.to_string_lossy().to_string(),
                field: kind.field.to_string(),
                game_path: game.path.clone(),
                game_name: game.name.clone().unwrap_or_else(|| game.path.clone()),
                damage,
                size: fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
                scraper_types: crate::models::settings::SCRAPE_MEDIA_TYPES
                    .iter()
                    .filter(|t| media::kind_for_scraper_type(t).is_some_and(|k| k.field == kind.field))
                    .map(|t| t.to_string())
                    .collect(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn png() -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
        b.extend_from_slice(&[0, 0, 0, 13]);
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&[0u8; 17]);
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(b"IEND");
        b.extend_from_slice(&[0xAE, 0x42, 0x60, 0x82]);
        b
    }

    fn mp4_box(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        b.extend_from_slice(kind);
        b.extend_from_slice(body);
        b
    }

    #[test]
    fn images_whole_cut_empty_or_not_images() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        let good = png();
        assert_eq!(check_file(&file(d, "ok.png", &good)), None);
        assert_eq!(check_file(&file(d, "cut.png", &good[..good.len() - 10])), Some(Damage::Truncated));
        assert_eq!(check_file(&file(d, "empty.png", b"")), Some(Damage::Empty));
        assert_eq!(check_file(&file(d, "error.png", b"<!DOCTYPE html><html>Error 429</html>")), Some(Damage::NotMedia));

        let jpg = [&[0xFF, 0xD8, 0xFF, 0xE0][..], &[1u8; 200], &[0xFF, 0xD9]].concat();
        assert_eq!(check_file(&file(d, "ok.jpg", &jpg)), None);
        // Relleno detrás de la marca de fin: se ve bien, no cuenta
        let padded = [&jpg[..], &[0u8; 30]].concat();
        assert_eq!(check_file(&file(d, "padded.jpg", &padded)), None);
        assert_eq!(check_file(&file(d, "cut.jpg", &jpg[..150])), Some(Damage::Truncated));

        assert_eq!(check_file(&file(d, "ok.pdf", b"%PDF-1.4\n...\n%%EOF\n")), None);
        assert_eq!(check_file(&file(d, "cut.pdf", b"%PDF-1.4\n...")), Some(Damage::Truncated));
        // Formato que no se comprueba
        assert_eq!(check_file(&file(d, "x.bmp", b"BM...")), None);
    }

    #[test]
    fn mp4_needs_its_index_and_whole_boxes() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0isom");
        let moov = mp4_box(b"moov", &[0u8; 40]);
        let mdat = mp4_box(b"mdat", &[7u8; 500]);

        let ok = [ftyp.clone(), moov.clone(), mdat.clone()].concat();
        assert_eq!(check_file(&file(d, "ok.mp4", &ok)), None);
        // Índice al final (lo habitual en descargas sin «faststart»), cortado antes de llegar
        let moov_last = [ftyp.clone(), mdat.clone(), moov.clone()].concat();
        assert_eq!(check_file(&file(d, "moov_last.mp4", &moov_last)), None);
        assert_eq!(check_file(&file(d, "cut.mp4", &moov_last[..moov_last.len() - 20])), Some(Damage::Truncated));
        // Sin índice
        assert_eq!(check_file(&file(d, "nomoov.mp4", &[ftyp, mdat].concat())), Some(Damage::Truncated));
        assert_eq!(check_file(&file(d, "html.mp4", b"<html>not found</html>")), Some(Damage::NotMedia));
    }
}
