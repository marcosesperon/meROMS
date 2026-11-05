//! Lo que comparten las fuentes del scraper: escribir los datos encontrados según la regla
//! («Qué actualiza el scraper») y descargar un medio al sitio que le toca.

use std::fs;

use reqwest::Client;

use crate::layout::{Layout, SystemRef};
use crate::media;
use crate::models::game::Game;
use crate::models::settings::FieldPolicy;
use crate::models::AppSettings;

/// Datos que trae una fuente (None = no lo tiene)
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ScrapedData {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub genre: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub players: Option<String>,
    /// 0..1 con dos decimales
    pub rating: Option<String>,
    /// Formato de EmulationStation: AAAAMMDDT000000
    pub releasedate: Option<String>,
}

/// Copia del juego con los datos encontrados, campo a campo según la regla
pub fn apply_data(game: &Game, data: ScrapedData, settings: &AppSettings) -> Game {
    let policy = settings.scrape_policy();
    let mut out = game.clone();
    // Ids guardados por versiones antiguas desde TheGamesDB («1436») cuentan como vacíos
    crate::commands::scraper_tgdb::forget_numeric_ids(&mut out);
    policy.data("name").apply(&mut out.name, data.name);
    policy.data("desc").apply(&mut out.desc, data.desc);
    policy.data("genre").apply(&mut out.genre, data.genre);
    policy.data("developer").apply(&mut out.developer, data.developer);
    policy.data("publisher").apply(&mut out.publisher, data.publisher);
    policy.data("players").apply(&mut out.players, data.players);
    policy.data("rating").apply(&mut out.rating, data.rating);
    policy.data("releasedate").apply(&mut out.releasedate, data.releasedate);
    out
}

/// Fecha «AAAA-MM-DD», «AAAA-MM» o «AAAA» → formato de EmulationStation
pub fn es_date(date: &str) -> Option<String> {
    let digits: String = date.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.len() {
        n if n >= 8 => Some(format!("{}T000000", &digits[..8])),
        6 => Some(format!("{}01T000000", digits)),
        4 => Some(format!("{}0101T000000", digits)),
        _ => None,
    }
}

/// ¿Hay que descargar este tipo de medio para este juego? (regla + «solo si falta»)
pub fn wants_media(layout: &Layout, system: &SystemRef, settings: &AppSettings, game: &Game, media_type: &str) -> bool {
    match settings.scrape_policy().media(media_type) {
        FieldPolicy::Overwrite => true,
        FieldPolicy::IfEmpty => !media::has_scraped_media(layout, system, settings, game, media_type),
        FieldPolicy::Keep => false,
    }
}

/// Extensión de lo descargado: de la URL si la trae; si no, por las primeras bytes
fn extension_for(url: &str, bytes: &[u8], media_type: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    if let Some(ext) = path.rsplit('/').next().and_then(|f| f.rsplit_once('.')).map(|(_, e)| e.to_lowercase()) {
        if (2..=4).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()) && ext != "php" {
            return ext;
        }
    }
    if bytes.starts_with(b"\x89PNG") {
        "png".into()
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "jpg".into()
    } else if bytes.starts_with(b"GIF8") {
        "gif".into()
    } else if bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        "webp".into()
    } else if bytes.len() > 8 && &bytes[4..8] == b"ftyp" {
        "mp4".into()
    } else if media_type == "video" {
        "mp4".into()
    } else {
        "png".into()
    }
}

/// Descarga un medio y lo guarda con el nombre unificado. Devuelve (campo, valor) para la
/// gamelist, o None si no se quería, no se pudo o era un relleno aprendido.
#[allow(clippy::too_many_arguments)]
pub async fn fetch_media(
    client: &Client,
    url: &str,
    layout: &Layout,
    system: &SystemRef,
    settings: &AppSettings,
    game: &Game,
    media_type: &str,
) -> Option<(String, String)> {
    if !wants_media(layout, system, settings, game, media_type) {
        return None;
    }
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    // Vacío, una página de error o un relleno aprendido: como si no hubiera
    if bytes.is_empty() || bytes.first() == Some(&b'<') || crate::commands::placeholders::is_placeholder(&bytes) {
        return None;
    }
    let ext = extension_for(url, &bytes, media_type);
    let target = media::scraped_media_target(layout, system, settings, media_type, &game.path, &ext)?;
    if let Some(parent) = target.dest.parent() {
        fs::create_dir_all(parent).ok()?;
    }
    fs::write(&target.dest, &bytes).ok()?;
    // «Sobrescribir»: el anterior con otra extensión (x.jpg frente al nuevo x.png) se va
    if settings.scrape_policy().media(media_type) == FieldPolicy::Overwrite {
        media::trash_replaced_extensions(layout, system, media_type, &game.path, &target.dest);
    }
    Some((target.field, target.value))
}

/// Pone en el juego los campos de los medios descargados
pub fn apply_media(game: &mut Game, downloaded: Vec<(String, String)>) {
    for (field, value) in downloaded {
        media::set_field(game, &field, Some(value));
    }
}

/// Nombre «comparable»: minúsculas, sin etiquetas («(USA)», «[!]»), sin la palabra «the»,
/// «&» como «and» y solo letras y números separados por un espacio.
pub fn normalize_title(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0u32;
    for c in name.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            '&' => out.push_str(" and "),
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => out.push(' '),
        }
    }
    // «The» fuera en cualquier sitio: «Zelda, The - A Link…» y «The Zelda: A Link…» quedan igual
    let words: Vec<&str> = out.split_whitespace().filter(|w| *w != "the").collect();
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_compare_without_tags_articles_or_punctuation() {
        assert_eq!(normalize_title("The Legend of Zelda: A Link to the Past (USA)"), "legend of zelda a link to past");
        assert_eq!(normalize_title("Legend of Zelda, The - A Link to the Past [!]"), "legend of zelda a link to past");
        assert_eq!(normalize_title("Sonic & Knuckles"), "sonic and knuckles");
        assert_eq!(normalize_title("Street Fighter II' - Champion Edition"), "street fighter ii champion edition");
    }

    #[test]
    fn dates_to_emulationstation_format() {
        assert_eq!(es_date("1994-03-11").as_deref(), Some("19940311T000000"));
        assert_eq!(es_date("1994").as_deref(), Some("19940101T000000"));
        assert_eq!(es_date("abc"), None);
    }

    #[test]
    fn extension_from_url_or_content() {
        assert_eq!(extension_for("https://x/a/b.jpg", b"", "box-2D"), "jpg");
        assert_eq!(extension_for("https://adb/?mame=sf2&type=ingame", b"\x89PNG....", "ss"), "png");
        assert_eq!(extension_for("https://adb/download_file.php?x=1", b"\0\0\0\x18ftypmp42", "video"), "mp4");
    }
}
