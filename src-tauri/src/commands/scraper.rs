use std::fs;
use std::path::Path;

use md5::{Digest, Md5};
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::Client;
use serde_json::Value;
use tauri::Emitter;
use tokio::time::{sleep, Duration};

use crate::layout::{Layout, SystemRef};
use crate::media;
use crate::models::game::Game;
use crate::models::scraper::{ScrapeProgress, ScrapeResult};
use crate::models::system::screenscraper_id_for_system;
use crate::models::AppSettings;
use crate::models::GameList;

const API_BASE: &str = "https://www.screenscraper.fr/api2";
const SOFTWARE_NAME: &str = "meROMS";
const REQUEST_DELAY_MS: u64 = 1500;

fn build_http_client() -> Result<Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        format!("{}/{}", SOFTWARE_NAME, "0.1.0")
            .parse()
            .unwrap(),
    );
    Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

fn compute_md5(file_path: &Path) -> Result<String, String> {
    let data = fs::read(file_path).map_err(|e| format!("Failed to read file for MD5: {}", e))?;
    let mut hasher = Md5::new();
    hasher.update(&data);
    Ok(format!("{:x}", hasher.finalize()))
}

fn build_api_url(
    endpoint: &str,
    settings: &AppSettings,
    extra_params: &[(&str, &str)],
) -> Result<String, String> {
    let devid = settings
        .ss_devid
        .as_deref()
        .ok_or("ScreenScraper devid not configured")?;
    let devpassword = settings
        .ss_devpassword
        .as_deref()
        .ok_or("ScreenScraper devpassword not configured")?;

    let mut url = format!(
        "{}/{}?devid={}&devpassword={}&softname={}&output=json",
        API_BASE,
        endpoint,
        urlencoding::encode(devid),
        urlencoding::encode(devpassword),
        SOFTWARE_NAME
    );

    if let Some(user) = &settings.ss_user {
        if !user.is_empty() {
            url.push_str(&format!("&ssid={}", urlencoding::encode(user)));
        }
    }
    if let Some(pass) = &settings.ss_password {
        if !pass.is_empty() {
            url.push_str(&format!("&sspassword={}", urlencoding::encode(pass)));
        }
    }

    for (key, value) in extra_params {
        url.push_str(&format!(
            "&{}={}",
            key,
            urlencoding::encode(value)
        ));
    }

    Ok(url)
}

/// Map app language code to ScreenScraper language code.
fn ss_language(settings: &AppSettings) -> &str {
    match settings.language.as_deref().unwrap_or("en") {
        "es" => "es",
        "fr" => "fr",
        "de" => "de",
        "it" => "it",
        "pt" => "pt",
        "ja" => "ja",
        "zh" => "zh",
        "ko" => "ko",
        "nl" => "nl",
        "sv" => "sv",
        "da" => "da",
        "no" => "no",
        "fi" => "fi",
        "hu" => "hu",
        "pl" => "pl",
        "cs" => "cs",
        "ro" => "ro",
        "ru" => "ru",
        _ => "en",
    }
}

/// Extract a text value from a ScreenScraper localized array, preferring a given language.
fn extract_localized(arr: &Value, lang: &str) -> Option<String> {
    let items = arr.as_array()?;
    // Try preferred language first
    for item in items {
        if item.get("langue").and_then(|l| l.as_str()) == Some(lang) {
            return item.get("text").and_then(|t| t.as_str()).map(String::from);
        }
    }
    // Try English
    for item in items {
        if item.get("langue").and_then(|l| l.as_str()) == Some("en") {
            return item.get("text").and_then(|t| t.as_str()).map(String::from);
        }
    }
    // Fallback to first
    items
        .first()
        .and_then(|item| item.get("text").and_then(|t| t.as_str()).map(String::from))
}

/// Extract a regional text value, preferring "wor" (world) then "us" then "eu" then first.
fn extract_regional(arr: &Value, _field: &str) -> Option<String> {
    let items = arr.as_array()?;
    let preferred = ["wor", "us", "eu", "jp", "ss"];
    for region in &preferred {
        for item in items {
            if item.get("region").and_then(|r| r.as_str()) == Some(region) {
                return item.get("text").and_then(|t| t.as_str()).map(String::from);
            }
        }
    }
    items
        .first()
        .and_then(|item| item.get("text").and_then(|t| t.as_str()).map(String::from))
}

/// Find media URL for a given type, preferring certain regions.
fn find_media_url(medias: &Value, media_type: &str) -> Option<String> {
    let items = medias.as_array()?;
    let preferred_regions = ["wor", "us", "eu", "jp", "ss"];

    // Try preferred regions
    for region in &preferred_regions {
        for item in items {
            if item.get("type").and_then(|t| t.as_str()) == Some(media_type)
                && item.get("region").and_then(|r| r.as_str()) == Some(region)
            {
                return item.get("url").and_then(|u| u.as_str()).map(String::from);
            }
        }
    }

    // Fallback: first match with no region filter
    for item in items {
        if item.get("type").and_then(|t| t.as_str()) == Some(media_type) {
            return item.get("url").and_then(|u| u.as_str()).map(String::from);
        }
    }

    None
}

// Where each media type is saved and which field it fills is decided by
// media::scraped_media_target() (unified naming + active structure).

/// Download an image from URL and save to disk.
async fn download_media(
    client: &Client,
    url: &str,
    dest_path: &Path,
) -> Result<(), String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Download HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read download body: {}", e))?;

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).ok();
    }

    fs::write(dest_path, &bytes).map_err(|e| format!("Failed to save media: {}", e))
}

/// Parse the API response JSON and extract game metadata + download media.
async fn process_scrape_response(
    client: &Client,
    jeu: &Value,
    existing_game: &Game,
    layout: &Layout,
    system: &SystemRef,
    media_types: &[String],
    settings: &AppSettings,
) -> Result<Game, String> {
    // Preferred language from app settings
    let lang = ss_language(settings);

    // Extract metadata
    let name = extract_regional(&jeu["noms"], "noms")
        .or_else(|| existing_game.name.clone());

    let desc = extract_localized(&jeu["synopsis"], lang);

    let genre = jeu["genres"]
        .as_array()
        .and_then(|genres| {
            genres.first().and_then(|g| {
                extract_localized(&g["noms"], lang)
            })
        });

    let developer = jeu["developpeur"]
        .get("text")
        .and_then(|t| t.as_str())
        .map(String::from);

    let publisher = jeu["editeur"]
        .get("text")
        .and_then(|t| t.as_str())
        .map(String::from);

    let players = jeu["joueurs"]
        .get("text")
        .and_then(|t| t.as_str())
        .map(String::from);

    // Rating: ScreenScraper uses 0-20 scale, convert to 0-1
    let rating = jeu["note"]
        .get("text")
        .and_then(|t| t.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|r| format!("{:.2}", r / 20.0));

    let releasedate = extract_regional(&jeu["dates"], "dates");

    // Download media
    let medias = &jeu["medias"];
    // (game field, value) of each media file actually downloaded
    let mut downloaded: Vec<(String, String)> = Vec::new();

    for media_type in media_types {
        // ScreenScraper uses "video" and "video-normalized" as media type names
        let search_type = if media_type == "video" {
            "video-normalized"
        } else {
            media_type.as_str()
        };

        if let Some(url) = find_media_url(medias, search_type)
            .or_else(|| {
                if media_type == "video" {
                    find_media_url(medias, "video")
                } else {
                    None
                }
            })
        {
            let ext = url
                .rsplit('.')
                .next()
                .unwrap_or(if media_type == "video" { "mp4" } else { "png" })
                .split('?')
                .next()
                .unwrap_or(if media_type == "video" { "mp4" } else { "png" });

            let Some(target) = media::scraped_media_target(
                layout, system, settings, media_type, &existing_game.path, ext,
            ) else {
                continue; // media type without a known folder
            };

            if download_media(client, &url, &target.dest).await.is_ok() {
                downloaded.push((target.field, target.value));
            }
        }
    }

    // Clone existing game and overwrite only the fields that the scraper found.
    // This preserves all fields the scraper doesn't touch (favorite, hidden, etc.)
    let mut game = existing_game.clone();
    if name.is_some() { game.name = name; }
    if desc.is_some() { game.desc = desc; }
    if genre.is_some() { game.genre = genre; }
    if developer.is_some() { game.developer = developer; }
    if publisher.is_some() { game.publisher = publisher; }
    if players.is_some() { game.players = players; }
    if rating.is_some() { game.rating = rating; }
    if releasedate.is_some() { game.releasedate = releasedate; }
    for (field, value) in downloaded {
        media::set_field(&mut game, &field, Some(value));
    }

    // Extract ScreenScraper game ID
    if let Some(id) = jeu.get("id").and_then(|v| v.as_u64()) {
        game.id = Some(id.to_string());
        game.source = Some("screenscraper".to_string());
    }

    // Extract additional metadata from ScreenScraper
    if let Some(family) = jeu.get("familles").and_then(|f| f.as_array()).and_then(|arr| {
        arr.first().and_then(|f| extract_localized(&f["noms"], "en"))
    }) {
        game.family = Some(family);
    }

    if let Some(genreid) = jeu.get("genres").and_then(|g| g.as_array()).and_then(|arr| {
        arr.first().and_then(|g| g.get("id").and_then(|id| id.as_u64()).map(|id| id.to_string()))
    }) {
        game.genreid = Some(genreid);
    }

    Ok(game)
}

/// Scrape a single game using ScreenScraper API.
async fn scrape_game_screenscraper(
    settings: &AppSettings,
    existing_game: &Game,
    layout: &Layout,
    system: &SystemRef,
) -> Result<ScrapeResult, String> {
    let folder_name = system.name.as_str();
    let system_id = screenscraper_id_for_system(folder_name)
        .ok_or_else(|| format!("Unknown system for ScreenScraper: {}", folder_name))?;

    let rom_relative = existing_game.path.strip_prefix("./").unwrap_or(&existing_game.path);
    let rom_absolute = system.rom_dir.join(rom_relative);
    let md5_hash = compute_md5(&rom_absolute)?;

    let rom_filename = Path::new(&existing_game.path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&existing_game.path);

    let system_id_str = system_id.to_string();
    let params = vec![
        ("systemeid", system_id_str.as_str()),
        ("md5", md5_hash.as_str()),
        ("romnom", rom_filename),
    ];

    let url = build_api_url("jeuInfos.php", settings, &params)?;

    let client = build_http_client()?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        if status.as_u16() == 404 || status.as_u16() == 430 {
            return Ok(ScrapeResult {
                game: existing_game.clone(),
                found: false,
            });
        }
        return Err(format!(
            "ScreenScraper error: HTTP {} — {}",
            status,
            body_text.chars().take(300).collect::<String>()
        ));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    let jeu = &body["response"]["jeu"];
    if jeu.is_null() {
        return Ok(ScrapeResult {
            game: existing_game.clone(),
            found: false,
        });
    }

    let media_types = settings.media_types();
    let updated_game =
        process_scrape_response(&client, jeu, existing_game, layout, system, &media_types, settings).await?;

    Ok(ScrapeResult {
        game: updated_game,
        found: true,
    })
}

/// Scrape a single game — dispatches to ScreenScraper or TheGamesDB based on settings.
#[tauri::command]
pub async fn scrape_game(
    app: tauri::AppHandle,
    system_path: String,
    game_path: String,
) -> Result<ScrapeResult, String> {
    let settings: AppSettings = crate::commands::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;

    let gamelist = crate::commands::gamelist::read_gamelist(&layout, &system)?;
    let existing_game = gamelist
        .games
        .iter()
        .find(|g| g.path == game_path)
        .cloned()
        .ok_or("Game not found in gamelist")?;

    // Try ScreenScraper first if configured
    if settings.has_screenscraper() {
        return scrape_game_screenscraper(&settings, &existing_game, &layout, &system).await;
    }

    // Fallback to TheGamesDB
    if settings.has_tgdb() {
        let client = build_http_client()?;
        let api_key = settings.tgdb_apikey.as_deref().unwrap();
        return crate::commands::scraper_tgdb::scrape_game_tgdb(
            &client,
            api_key,
            &existing_game,
            &layout,
            &system,
            &settings,
        )
        .await;
    }

    Err("No scraper configured. Add ScreenScraper or TheGamesDB credentials in Settings.".into())
}

/// Scrape a single game internally (used by batch). Returns (status_str, updated_game).
async fn scrape_one(
    settings: &AppSettings,
    client: &Client,
    game: &Game,
    layout: &Layout,
    system: &SystemRef,
) -> (&'static str, Game) {
    if settings.has_screenscraper() {
        match scrape_game_screenscraper(settings, game, layout, system).await {
            Ok(r) if r.found => return ("success", r.game),
            Ok(_) => {}
            Err(_) => {}
        }
    }

    if settings.has_tgdb() {
        let api_key = settings.tgdb_apikey.as_deref().unwrap();
        match crate::commands::scraper_tgdb::scrape_game_tgdb(
            client,
            api_key,
            game,
            layout,
            system,
            settings,
        )
        .await
        {
            Ok(r) if r.found => return ("success", r.game),
            Ok(_) => return ("not_found", game.clone()),
            Err(_) => return ("error", game.clone()),
        }
    }

    ("not_found", game.clone())
}

/// Batch scrape all games in a system, emitting progress events.
#[tauri::command]
pub async fn scrape_system(
    app: tauri::AppHandle,
    system_path: String,
    game_paths: Option<Vec<String>>,
) -> Result<(), String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let gamelist = crate::commands::gamelist::read_gamelist(&layout, &system)?;
    // Con `game_paths` solo se scrapean esos juegos (p. ej. las ROMs recién añadidas);
    // el resto pasa tal cual
    let wanted = |game: &Game| game_paths.as_ref().map_or(true, |only| only.contains(&game.path));
    let total = gamelist.games.iter().filter(|g| wanted(g)).count();

    if total == 0 {
        return Err("No games to scrape".to_string());
    }

    let settings: AppSettings = crate::commands::settings::get_settings(app.clone())?;

    if !settings.has_screenscraper() && !settings.has_tgdb() {
        return Err(
            "No scraper configured. Add ScreenScraper or TheGamesDB credentials in Settings."
                .into(),
        );
    }

    let client = build_http_client()?;
    let mut updated_games: Vec<Game> = Vec::new();

    let mut i = 0;
    for game in &gamelist.games {
        if !wanted(game) {
            updated_games.push(game.clone());
            continue;
        }
        let game_name = game.name.clone().unwrap_or_else(|| game.path.clone());

        let _ = app.emit(
            "scrape-progress",
            ScrapeProgress {
                current: i + 1,
                total,
                game_name: game_name.clone(),
                status: "scraping".to_string(),
            },
        );

        if i > 0 {
            sleep(Duration::from_millis(REQUEST_DELAY_MS)).await;
        }

        let (status, updated_game) =
            scrape_one(&settings, &client, game, &layout, &system).await;
        updated_games.push(updated_game);

        let _ = app.emit(
            "scrape-progress",
            ScrapeProgress {
                current: i + 1,
                total,
                game_name,
                status: status.to_string(),
            },
        );
        i += 1;
    }

    let updated_gamelist = GameList {
        games: updated_games,
        folders: gamelist.folders,
        provider: gamelist.provider,
    };
    crate::commands::gamelist::write_gamelist(&layout, &system, &updated_gamelist)?;

    Ok(())
}
