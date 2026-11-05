//! ArcadeItalia (Arcade Database, adb.arcadeitalia.net) como fuente del scraper: solo
//! arcade, gratis y sin cuenta. Se consulta por el nombre corto de MAME (el del zip: «sf2»)
//! y trae datos, la historia del juego (arcade-history.com), imágenes y un vídeo corto.

use std::path::Path;

use reqwest::Client;
use serde_json::Value;

use super::scrape_common::{apply_data, apply_media, es_date, fetch_media, ScrapedData};
use crate::layout::{Layout, SystemRef};
use crate::models::game::Game;
use crate::models::scraper::{ScrapeCandidate, ScrapeResult};
use crate::models::AppSettings;

const API: &str = "https://adb.arcadeitalia.net/service_scraper.php?ajax=query_mame&game_name=";

/// Sistemas cuyas ROMs llevan el nombre corto de MAME (los que se consultan en ArcadeItalia)
pub fn is_arcade_system(folder_name: &str) -> bool {
    matches!(
        folder_name.to_lowercase().as_str(),
        "arcade" | "mame" | "fba" | "fbneo" | "cps" | "cps1" | "cps2" | "cps3" | "neogeo" | "naomi" | "naomi2" | "naomigd"
            | "atomiswave" | "model2" | "model3" | "stv"
    )
}

/// ¿Parece un nombre corto de MAME? (una palabra, letras/números/«_», no muy largo)
pub fn looks_like_short_name(text: &str) -> bool {
    let t = text.trim();
    !t.is_empty() && t.len() <= 20 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn rom_stem(game: &Game) -> String {
    Path::new(&game.path).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Consulta un nombre corto. None = no lo conoce.
async fn query(client: &Client, short_name: &str) -> Result<Option<Value>, String> {
    let url = format!("{}{}", API, urlencoding::encode(short_name));
    let response = client.get(&url).send().await.map_err(|e| format!("ArcadeItalia request failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("ArcadeItalia error: HTTP {}", response.status()));
    }
    let body: Value = response.json().await.map_err(|e| format!("Failed to parse ArcadeItalia response: {}", e))?;
    Ok(first_result(&body))
}

fn first_result(body: &Value) -> Option<Value> {
    body["result"].as_array()?.iter().find(|r| r["game_name"].as_str().is_some_and(|n| !n.is_empty())).cloned()
}

fn text(r: &Value, key: &str) -> Option<String> {
    r[key].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// La historia sin la primera línea («published 35 years ago», se queda vieja) y sin las
/// secciones técnicas del final (TECHNICAL, TRIVIA, STAFF…): solo la descripción.
pub fn clean_history(history: &str) -> Option<String> {
    let mut lines: Vec<&str> = history.lines().collect();
    if lines.first().is_some_and(|l| l.contains("years ago")) {
        lines.remove(0);
    }
    let cut = lines.iter().position(|l| {
        let t = l.trim();
        t.starts_with("- ") && t.ends_with(" -") && t.len() > 4
    });
    let body = lines[..cut.unwrap_or(lines.len())].join("\n");
    let body = body.trim();
    (!body.is_empty()).then(|| body.to_string())
}

/// Datos de una respuesta de ArcadeItalia
pub fn scraped_data(r: &Value) -> ScrapedData {
    let manufacturer = text(r, "manufacturer");
    ScrapedData {
        name: text(r, "short_title").or_else(|| text(r, "title")),
        desc: text(r, "history").and_then(|h| clean_history(&h)),
        genre: text(r, "genre"),
        developer: manufacturer.clone(),
        publisher: manufacturer,
        players: r["players"].as_u64().map(|p| p.to_string()).or_else(|| text(r, "players")),
        rating: r["rate"].as_f64().or_else(|| text(r, "rate").and_then(|v| v.parse().ok())).map(|v| format!("{:.2}", (v / 100.0).clamp(0.0, 1.0))),
        releasedate: r["year"].as_u64().map(|y| y.to_string()).or_else(|| text(r, "year")).and_then(|y| es_date(&y)),
    }
}

/// Medios de una respuesta: (tipo del scraper, URL)
pub fn media_urls(r: &Value) -> Vec<(&'static str, String)> {
    [
        ("ss", "url_image_ingame"),
        ("sstitle", "url_image_title"),
        ("marquee", "url_image_marquee"),
        ("box-2D", "url_image_flyer"),
        ("video", "url_video_shortplay"),
    ]
    .into_iter()
    .filter_map(|(kind, key)| text(r, key).map(|url| (kind, url)))
    .collect()
}

async fn apply_result(client: &Client, r: &Value, game: &Game, layout: &Layout, system: &SystemRef, settings: &AppSettings) -> Game {
    let mut updated = apply_data(game, scraped_data(r), settings);
    let mut downloaded = Vec::new();
    for (kind, url) in media_urls(r) {
        downloaded.extend(fetch_media(client, &url, layout, system, settings, game, kind).await);
    }
    apply_media(&mut updated, downloaded);
    updated
}

/// Automático: por el nombre del zip. En sistemas que no son arcade, «no encontrado».
pub async fn scrape_game_adb(client: &Client, game: &Game, layout: &Layout, system: &SystemRef, settings: &AppSettings) -> Result<ScrapeResult, String> {
    if !is_arcade_system(&system.name) {
        return Ok(ScrapeResult { game: game.clone(), found: false });
    }
    match query(client, &rom_stem(game)).await? {
        Some(r) => Ok(ScrapeResult { game: apply_result(client, &r, game, layout, system, settings).await, found: true }),
        None => Ok(ScrapeResult { game: game.clone(), found: false }),
    }
}

/// Búsqueda manual: el nombre del zip del juego y, si el texto parece un nombre corto, ese también
pub async fn search_adb(client: &Client, game: &Game, query_text: &str) -> Result<Vec<ScrapeCandidate>, String> {
    let mut names = vec![rom_stem(game)];
    if looks_like_short_name(query_text) && !names.contains(&query_text.trim().to_lowercase()) {
        names.push(query_text.trim().to_lowercase());
    }
    let mut out = Vec::new();
    for name in names {
        if let Some(r) = query(client, &name).await? {
            out.push(ScrapeCandidate {
                source: "arcadeitalia".into(),
                id: text(&r, "game_name").unwrap_or(name),
                name: text(&r, "title").unwrap_or_default(),
                system: Some("Arcade".into()),
                year: text(&r, "year").or_else(|| r["year"].as_u64().map(|y| y.to_string())),
                developer: text(&r, "manufacturer"),
                thumbnail: text(&r, "url_image_flyer").or_else(|| text(&r, "url_image_title")),
                rom_match: false,
            });
        }
    }
    Ok(out)
}

/// El resultado elegido en la búsqueda manual (id = nombre corto)
pub async fn apply_adb_by_id(client: &Client, id: &str, game: &Game, layout: &Layout, system: &SystemRef, settings: &AppSettings) -> Result<ScrapeResult, String> {
    let r = query(client, id).await?.ok_or("ArcadeItalia: game not found")?;
    Ok(ScrapeResult { game: apply_result(client, &r, game, layout, system, settings).await, found: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!({"result": [{
            "game_name": "sf2",
            "title": "Street Fighter II: The World Warrior (World 910522)",
            "short_title": "Street Fighter II: The World Warrior",
            "manufacturer": "Capcom",
            "year": 1991,
            "genre": "Fighter / Versus",
            "players": 2,
            "rate": 90,
            "history": "Arcade Video game published 35 years ago:\n\nStreet Fighter II (c) 1991 Capcom.\nThe best.\n\n- TECHNICAL -\n\nMain CPU : 68000",
            "url_image_ingame": "https://adb/?mame=sf2&type=ingame&resize=0",
            "url_image_flyer": "https://adb/?mame=sf2&type=flyer&resize=0",
            "url_video_shortplay": "https://adb/download_file.php?codice=sf2&entity=shortplay",
            "url_image_marquee": ""
        }]})
    }

    #[test]
    fn reads_data_history_and_media() {
        let r = first_result(&sample()).unwrap();
        let d = scraped_data(&r);
        assert_eq!(d.name.as_deref(), Some("Street Fighter II: The World Warrior"));
        assert_eq!(d.desc.as_deref(), Some("Street Fighter II (c) 1991 Capcom.\nThe best."));
        assert_eq!(d.developer.as_deref(), Some("Capcom"));
        assert_eq!(d.players.as_deref(), Some("2"));
        assert_eq!(d.rating.as_deref(), Some("0.90"));
        assert_eq!(d.releasedate.as_deref(), Some("19910101T000000"));
        let kinds: Vec<&str> = media_urls(&r).iter().map(|(k, _)| *k).collect();
        assert_eq!(kinds, vec!["ss", "box-2D", "video"], "empty URLs are skipped");
        assert!(first_result(&serde_json::json!({"result": []})).is_none());
    }

    #[test]
    fn short_names_and_arcade_systems() {
        assert!(looks_like_short_name("sf2ua"));
        assert!(!looks_like_short_name("Street Fighter"));
        assert!(is_arcade_system("fbneo"));
        assert!(!is_arcade_system("snes"));
    }
}
