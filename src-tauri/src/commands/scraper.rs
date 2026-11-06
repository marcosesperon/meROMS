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
use crate::models::scraper::{ScrapeCandidate, ScrapeProgress, ScrapeResult, ScrapeSearch};
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
        format!("{}/{}", SOFTWARE_NAME, env!("CARGO_PKG_VERSION"))
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
    // The user's developer credentials, or the ones built into the app
    let (devid, devpassword) = settings
        .ss_dev_credentials()
        .ok_or("ScreenScraper developer credentials not configured")?;

    let mut url = format!(
        "{}/{}?devid={}&devpassword={}&softname={}&output=json",
        API_BASE,
        endpoint,
        urlencoding::encode(&devid),
        urlencoding::encode(&devpassword),
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

/// The first item of a ScreenScraper list whose `key` (region / langue) comes first in
/// `preferred`; if none matches, the first item (as before). `keep` picks which items count.
fn pick_preferred<'a>(items: &'a [Value], key: &str, preferred: &[String], keep: impl Fn(&Value) -> bool) -> Option<&'a Value> {
    let candidates: Vec<&Value> = items.iter().filter(|i| keep(i)).collect();
    preferred
        .iter()
        .find_map(|p| candidates.iter().find(|i| i.get(key).and_then(|v| v.as_str()) == Some(p.as_str())).copied())
        .or_else(|| candidates.first().copied())
}

/// Text from a ScreenScraper localized array, in the preferred language order.
fn extract_localized(arr: &Value, languages: &[String]) -> Option<String> {
    let item = pick_preferred(arr.as_array()?, "langue", languages, |_| true)?;
    item.get("text").and_then(|t| t.as_str()).map(String::from)
}

/// Text from a ScreenScraper regional array (names, dates), in the preferred region order.
fn extract_regional(arr: &Value, regions: &[String]) -> Option<String> {
    let item = pick_preferred(arr.as_array()?, "region", regions, |_| true)?;
    item.get("text").and_then(|t| t.as_str()).map(String::from)
}

/// Media URL of a type, in the preferred region order.
fn find_media_url(medias: &Value, media_type: &str, regions: &[String]) -> Option<String> {
    find_media(medias, media_type, regions).map(|(url, _)| url)
}

/// URL of a media type and its `format` (png, jpg, mp4, pdf…) when ScreenScraper gives it:
/// its URLs end in «mediaJeu.php?…», so the extension can't be taken from them
fn find_media(medias: &Value, media_type: &str, regions: &[String]) -> Option<(String, Option<String>)> {
    let item = pick_preferred(medias.as_array()?, "region", regions, |i| i.get("type").and_then(|t| t.as_str()) == Some(media_type))?;
    let url = item.get("url").and_then(|u| u.as_str()).map(String::from)?;
    let format = item
        .get("format")
        .and_then(|f| f.as_str())
        .map(|f| f.trim().to_lowercase())
        .filter(|f| (2..=4).contains(&f.len()) && f.chars().all(|c| c.is_ascii_alphanumeric()));
    Some((url, format))
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

    // Un relleno aprendido (contraportada verde vacía, vídeo «not playable»…): como si no hubiera
    if crate::commands::placeholders::is_placeholder(&bytes) {
        return Err("Placeholder media, skipped".into());
    }

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
    // Preferred regions (names, dates, media) and languages (texts), from Settings
    let regions = settings.scrape_regions();
    let languages = settings.scrape_languages();

    // Extract metadata
    let name = extract_regional(&jeu["noms"], &regions)
        .or_else(|| existing_game.name.clone());

    let desc = extract_localized(&jeu["synopsis"], &languages);

    let genre = jeu["genres"]
        .as_array()
        .and_then(|genres| {
            genres.first().and_then(|g| {
                extract_localized(&g["noms"], &languages)
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

    let releasedate = extract_regional(&jeu["dates"], &regions);

    // What may be updated (Settings, or the choice made when launching «Scrape all»)
    let policy = settings.scrape_policy();

    // Download media
    let medias = &jeu["medias"];
    // (game field, value) of each media file actually downloaded
    let mut downloaded: Vec<(String, String)> = Vec::new();

    for media_type in media_types {
        // «Only if missing»: don't even ask for it if the game already has one (saves quota)
        match policy.media(media_type) {
            crate::models::settings::FieldPolicy::Keep => continue,
            crate::models::settings::FieldPolicy::IfEmpty
                if media::has_scraped_media(layout, system, settings, existing_game, media_type) =>
            {
                continue
            }
            _ => {}
        }
        // ScreenScraper uses "video" and "video-normalized" as media type names
        let search_type = if media_type == "video" {
            "video-normalized"
        } else {
            media_type.as_str()
        };

        if let Some((url, format)) = find_media(medias, search_type, &regions)
            .or_else(|| {
                if media_type == "video" {
                    find_media(medias, "video", &regions)
                } else {
                    None
                }
            })
        {
            let fallback = if media_type == "video" { "mp4" } else { "png" };
            // The format ScreenScraper gives (a manual is «pdf»); if not, from the URL as before
            let from_url = url.rsplit('.').next().unwrap_or(fallback).split('?').next().unwrap_or(fallback).to_string();
            let ext = format.unwrap_or(from_url);
            let ext = ext.as_str();

            let Some(target) = media::scraped_media_target(
                layout, system, settings, media_type, &existing_game.path, ext,
            ) else {
                continue; // media type without a known folder
            };

            if download_media(client, &url, &target.dest).await.is_ok() {
                // «Overwrite»: the old one with another extension (x.jpg vs the new x.png) goes away
                if policy.media(media_type) == crate::models::settings::FieldPolicy::Overwrite {
                    media::trash_replaced_extensions(layout, system, media_type, &existing_game.path, &target.dest);
                }
                downloaded.push((target.field, target.value));
            }
        }
    }

    // Clone the existing game and write what the scraper found, field by field, as the policy
    // says (overwrite / only if empty / leave). Fields the scraper doesn't touch stay as they are.
    let mut game = existing_game.clone();
    // Ids saved by older versions from TheGamesDB («1436») count as empty
    crate::commands::scraper_tgdb::forget_numeric_ids(&mut game);
    policy.data("name").apply(&mut game.name, name);
    policy.data("desc").apply(&mut game.desc, desc);
    policy.data("genre").apply(&mut game.genre, genre);
    policy.data("developer").apply(&mut game.developer, developer);
    policy.data("publisher").apply(&mut game.publisher, publisher);
    policy.data("players").apply(&mut game.players, players);
    policy.data("rating").apply(&mut game.rating, rating);
    policy.data("releasedate").apply(&mut game.releasedate, releasedate);
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
        arr.first().and_then(|f| extract_localized(&f["noms"], &languages))
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

/// GET a ScreenScraper endpoint and read the JSON. `None` = «nothing found» (HTTP 404 / 430),
/// as the automatic scrape always treated it.
async fn ss_get(client: &Client, settings: &AppSettings, endpoint: &str, params: &[(&str, &str)]) -> Result<Option<Value>, String> {
    let url = build_api_url(endpoint, settings, params)?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        if status.as_u16() == 404 || status.as_u16() == 430 {
            return Ok(None);
        }
        return Err(format!(
            "ScreenScraper error: HTTP {} — {}",
            status,
            body_text.chars().take(300).collect::<String>()
        ));
    }

    response
        .json()
        .await
        .map(Some)
        .map_err(|e| format!("Failed to parse API response: {}", e))
}

/// The game ScreenScraper recognises from the ROM file itself (MD5 + file name), if any
async fn ss_game_by_rom(client: &Client, settings: &AppSettings, game: &Game, system: &SystemRef) -> Result<Option<Value>, String> {
    let folder_name = system.name.as_str();
    let system_id = screenscraper_id_for_system(folder_name)
        .ok_or_else(|| format!("Unknown system for ScreenScraper: {}", folder_name))?;

    let rom_relative = game.path.strip_prefix("./").unwrap_or(&game.path);
    let rom_absolute = system.rom_dir.join(rom_relative);
    let system_id_str = system_id.to_string();

    // Un .zip/.7z con un solo fichero: ScreenScraper conoce el fichero de dentro (su CRC,
    // tamaño y nombre), no el comprimido. Se leen del índice, sin descomprimir.
    if let Some(inner) = crate::rom_hash::single_archive_entry(&rom_absolute) {
        let crc = inner.crc.to_uppercase();
        let size = inner.size.to_string();
        // Solo el nombre, sin la carpeta que pudiera llevar dentro del comprimido
        let inner_name = inner.name.rsplit(['/', '\\']).next().unwrap_or(&inner.name).to_string();
        let params = vec![
            ("systemeid", system_id_str.as_str()),
            ("crc", crc.as_str()),
            ("romtaille", size.as_str()),
            ("romnom", inner_name.as_str()),
        ];
        return Ok(ss_get(client, settings, "jeuInfos.php", &params).await?.and_then(|body| real_game(&body["response"]["jeu"])));
    }

    // Fichero suelto, o zip de arcade (varios ficheros: se identifica por el nombre del zip)
    let md5_hash = compute_md5(&rom_absolute)?;

    let rom_filename = Path::new(&game.path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&game.path);

    let params = vec![
        ("systemeid", system_id_str.as_str()),
        ("md5", md5_hash.as_str()),
        ("romnom", rom_filename),
    ];
    Ok(ss_get(client, settings, "jeuInfos.php", &params).await?.and_then(|body| real_game(&body["response"]["jeu"])))
}

/// ScreenScraper sometimes answers «nothing» with an empty or id-less game: that's no game
fn real_game(jeu: &Value) -> Option<Value> {
    jeu.get("id").is_some().then(|| jeu.clone())
}

/// Scrape a single game using ScreenScraper API.
async fn scrape_game_screenscraper(
    settings: &AppSettings,
    existing_game: &Game,
    layout: &Layout,
    system: &SystemRef,
) -> Result<ScrapeResult, String> {
    let client = build_http_client()?;
    let Some(jeu) = ss_game_by_rom(&client, settings, existing_game, system).await? else {
        return Ok(ScrapeResult {
            game: existing_game.clone(),
            found: false,
        });
    };

    let media_types = settings.media_types();
    let updated_game =
        process_scrape_response(&client, &jeu, existing_game, layout, system, &media_types, settings).await?;

    Ok(ScrapeResult {
        game: updated_game,
        found: true,
    })
}

// ---------------------------------------------------------------------------------------------
// Manual search: several results to pick from (the «Scrape» button of one game)
// ---------------------------------------------------------------------------------------------

/// Thumbnail URLs of the last ScreenScraper search, by game id. They carry the credentials,
/// so they stay here and the window asks for each picture with `scrape_thumbnail`.
static SS_THUMBNAILS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, String>>> =
    std::sync::OnceLock::new();

fn ss_thumbnails() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    SS_THUMBNAILS.get_or_init(Default::default)
}

/// The card of one ScreenScraper game, plus its cover URL (to be kept server-side)
fn parse_ss_candidate(jeu: &Value, regions: &[String], rom_match: bool) -> Option<(ScrapeCandidate, Option<String>)> {
    let id = jeu.get("id").and_then(|v| v.as_str().map(String::from).or_else(|| v.as_u64().map(|n| n.to_string())))?;
    let name = extract_regional(&jeu["noms"], regions)?;
    let year = extract_regional(&jeu["dates"], regions).filter(|d| d.len() >= 4).map(|d| d[..4].to_string());
    let text = |key: &str| jeu[key].get("text").and_then(|t| t.as_str()).map(String::from);
    let cover = find_media_url(&jeu["medias"], "box-2D", regions).or_else(|| find_media_url(&jeu["medias"], "ss", regions));
    Some((
        ScrapeCandidate {
            source: "screenscraper".into(),
            id,
            name,
            system: text("systeme"),
            year,
            developer: text("developpeur"),
            thumbnail: None,
            rom_match,
        },
        cover,
    ))
}

/// Games ScreenScraper finds for a text (up to 30), on one system (None = all)
async fn search_screenscraper(
    client: &Client,
    settings: &AppSettings,
    query: &str,
    system_id: Option<u32>,
) -> Result<Vec<(ScrapeCandidate, Option<String>)>, String> {
    let system_id = system_id.map(|id| id.to_string());
    let mut params = vec![("recherche", query)];
    if let Some(id) = &system_id {
        params.push(("systemeid", id.as_str()));
    }
    let regions = settings.scrape_regions();
    let Some(body) = ss_get(client, settings, "jeuRecherche.php", &params).await? else {
        return Ok(Vec::new());
    };
    Ok(body["response"]["jeux"]
        .as_array()
        .map(|games| games.iter().filter_map(|jeu| parse_ss_candidate(jeu, &regions, false)).collect())
        .unwrap_or_default())
}

/// One list: the ROM match first, then each source's results in order (without repeating any)
fn merge_candidates(rom_match: Option<ScrapeCandidate>, sources: Vec<Vec<ScrapeCandidate>>) -> Vec<ScrapeCandidate> {
    let mut list: Vec<ScrapeCandidate> = rom_match.into_iter().collect();
    for candidate in sources.into_iter().flatten() {
        if !list.iter().any(|c| c.source == candidate.source && c.id == candidate.id) {
            list.push(candidate);
        }
    }
    list
}

/// Manual search for one game: ScreenScraper (ROM match + text) and TheGamesDB (text) at once.
/// A source that fails doesn't spoil the other: its error goes in `errors`.
#[tauri::command]
pub async fn search_scrape(
    app: tauri::AppHandle,
    system_path: String,
    game_path: String,
    query: String,
    all_systems: bool,
) -> Result<ScrapeSearch, String> {
    let settings: AppSettings = crate::commands::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;
    if source_order(&app, &settings, &system).is_empty() {
        return Err(NO_SCRAPER.into());
    }
    let query = query.trim().to_string();
    let client = build_http_client()?;
    let mut errors = Vec::new();

    // TheGamesDB runs in parallel (another service, its own limits)
    let tgdb_task = settings.has_tgdb().then(|| {
        let client = client.clone();
        let api_key = settings.tgdb_apikey.clone().unwrap_or_default();
        let platform = if all_systems { None } else { crate::commands::scraper_tgdb::tgdb_platform_id(&system.name) };
        let query = query.clone();
        tauri::async_runtime::spawn(async move {
            crate::commands::scraper_tgdb::search_tgdb(&client, &api_key, &query, platform).await
        })
    });

    // ScreenScraper, one request after the other (free accounts only get one at a time)
    let mut rom_match = None;
    let mut ss = Vec::new();
    if settings.has_screenscraper() {
        let regions = settings.scrape_regions();
        let game = crate::commands::gamelist::read_gamelist(&layout, &system)?
            .games
            .into_iter()
            .find(|g| g.path == game_path);
        if let Some(game) = game {
            match ss_game_by_rom(&client, &settings, &game, &system).await {
                Ok(jeu) => rom_match = jeu.and_then(|jeu| parse_ss_candidate(&jeu, &regions, true)),
                Err(e) => errors.push(format!("ScreenScraper: {}", e)),
            }
        }
        if !query.is_empty() {
            let system_id = if all_systems { None } else { screenscraper_id_for_system(&system.name) };
            match search_screenscraper(&client, &settings, &query, system_id).await {
                Ok(found) => ss = found,
                Err(e) => errors.push(format!("ScreenScraper: {}", e)),
            }
        }
    }

    // Covers of this search, kept here for `scrape_thumbnail`
    let (rom_match, ss) = {
        let mut thumbnails = ss_thumbnails().lock().map_err(|e| e.to_string())?;
        thumbnails.clear();
        let mut keep_cover = |(candidate, cover): (ScrapeCandidate, Option<String>)| {
            if let Some(url) = cover {
                thumbnails.insert(candidate.id.clone(), url);
            }
            candidate
        };
        let rom_match = rom_match.map(&mut keep_cover);
        let ss: Vec<ScrapeCandidate> = ss.into_iter().map(&mut keep_cover).collect();
        (rom_match, ss)
    };

    let mut tgdb = Vec::new();
    if let Some(task) = tgdb_task {
        match task.await {
            Ok(Ok(found)) => tgdb = found,
            Ok(Err(e)) => errors.push(format!("TheGamesDB: {}", e)),
            Err(e) => errors.push(format!("TheGamesDB: {}", e)),
        }
    }

    // ArcadeItalia (solo arcade: por el nombre del zip) y LaunchBox (base descargada)
    let mut adb = Vec::new();
    if crate::commands::scraper_adb::is_arcade_system(&system.name) {
        let game = crate::commands::gamelist::read_gamelist(&layout, &system)?.games.into_iter().find(|g| g.path == game_path);
        if let Some(game) = game {
            match crate::commands::scraper_adb::search_adb(&client, &game, &query).await {
                Ok(found) => adb = found,
                Err(e) => errors.push(format!("ArcadeItalia: {}", e)),
            }
        }
    }
    let launchbox = if crate::commands::scraper_launchbox::is_downloaded(&app) && !query.is_empty() {
        crate::commands::scraper_launchbox::search_launchbox(&app, &system, &query, all_systems, &settings.scrape_regions())
    } else {
        Vec::new()
    };

    Ok(ScrapeSearch { candidates: merge_candidates(rom_match, vec![ss, adb, launchbox, tgdb]), errors })
}

/// Small cover of a ScreenScraper result of the last search, as a data: URI (None if it has none).
/// The window asks for them one by one, so free accounts don't go over their one request at a time.
#[tauri::command]
pub async fn scrape_thumbnail(id: String) -> Result<Option<String>, String> {
    use base64::Engine;
    let Some(url) = ss_thumbnails().lock().map_err(|e| e.to_string())?.get(&id).cloned() else {
        return Ok(None);
    };
    // ScreenScraper resizes it on its side: much less to download
    let url = format!("{}&maxwidth=160&maxheight=220", url);
    let response = build_http_client()?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;
    if !response.status().is_success() {
        return Ok(None);
    }
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("image/"))
        .unwrap_or("image/png")
        .to_string();
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    Ok(Some(format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(&bytes))))
}

/// The result picked in the manual search: its full data, written into the game as the
/// policy says (same code as the automatic scrape). Nothing is saved: the caller does it.
#[tauri::command]
pub async fn apply_scrape(
    app: tauri::AppHandle,
    system_path: String,
    game_path: String,
    source: String,
    id: String,
) -> Result<ScrapeResult, String> {
    let settings: AppSettings = crate::commands::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;
    let game = crate::commands::gamelist::read_gamelist(&layout, &system)?
        .games
        .into_iter()
        .find(|g| g.path == game_path)
        .ok_or("Game not found in gamelist")?;
    let client = build_http_client()?;

    match source.as_str() {
        "screenscraper" => {
            let body = ss_get(&client, &settings, "jeuInfos.php", &[("gameid", id.as_str())])
                .await?
                .ok_or("ScreenScraper: game not found")?;
            let jeu = real_game(&body["response"]["jeu"]).ok_or("ScreenScraper: game not found")?;
            let media_types = settings.media_types();
            let game = process_scrape_response(&client, &jeu, &game, &layout, &system, &media_types, &settings).await?;
            Ok(ScrapeResult { game, found: true })
        }
        "thegamesdb" => {
            let api_key = settings.tgdb_apikey.clone().filter(|k| !k.is_empty()).ok_or("TheGamesDB not configured")?;
            let id: u64 = id.parse().map_err(|_| format!("Invalid TheGamesDB id: {}", id))?;
            crate::commands::scraper_tgdb::apply_tgdb_by_id(&client, &api_key, id, &game, &layout, &system, &settings).await
        }
        "launchbox" => crate::commands::scraper_launchbox::apply_launchbox_by_id(&app, &client, &id, &game, &layout, &system, &settings).await,
        "arcadeitalia" => crate::commands::scraper_adb::apply_adb_by_id(&client, &id, &game, &layout, &system, &settings).await,
        other => Err(format!("Unknown scraper source: {}", other)),
    }
}

/// Scrape a single game — dispatches to ScreenScraper or TheGamesDB based on settings.
#[derive(serde::Serialize)]
pub struct ScrapeOrder {
    pub regions: Vec<String>,
    pub languages: Vec<String>,
}

/// Region and language order the scraper uses right now (saved, or defaults from the app language)
#[tauri::command]
pub fn get_scrape_order(app: tauri::AppHandle) -> Result<ScrapeOrder, String> {
    let settings = crate::commands::settings::get_settings(app)?;
    Ok(ScrapeOrder { regions: settings.scrape_regions(), languages: settings.scrape_languages() })
}

/// What the scraper may update right now (saved, or derived from the old media list)
#[tauri::command]
pub fn get_scrape_policy(app: tauri::AppHandle) -> Result<crate::models::settings::ScrapePolicy, String> {
    Ok(crate::commands::settings::get_settings(app)?.scrape_policy())
}

#[derive(serde::Serialize)]
pub struct ScraperStatus {
    /// This copy of the app carries ScreenScraper developer credentials
    pub built_in_dev: bool,
    /// ScreenScraper can be used (developer credentials from the app or the user)
    pub screenscraper: bool,
    /// TheGamesDB key configured
    pub tgdb: bool,
    /// LaunchBox database downloaded
    pub launchbox: bool,
}

/// What scrapers are available (the UI shows the ScreenScraper account fields and the scrape
/// buttons from this, since the built-in credentials aren't visible in the settings).
#[tauri::command]
pub fn scraper_status(app: tauri::AppHandle) -> Result<ScraperStatus, String> {
    let settings = crate::commands::settings::get_settings(app.clone())?;
    Ok(ScraperStatus {
        built_in_dev: crate::models::settings::built_in_ss_credentials().is_some(),
        screenscraper: settings.has_screenscraper(),
        tgdb: settings.has_tgdb(),
        launchbox: crate::commands::scraper_launchbox::is_downloaded(&app),
    })
}

const NO_SCRAPER: &str = "No scraper available. Add ScreenScraper or TheGamesDB credentials, or download the LaunchBox database, in Settings.";

/// Fuentes del scraper automático
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    ScreenScraper,
    ArcadeItalia,
    LaunchBox,
    TheGamesDb,
}

/// Fuentes que se prueban para un sistema, en orden (gana la primera que encuentra el juego):
/// arcade: ScreenScraper → ArcadeItalia → LaunchBox; resto: ScreenScraper → LaunchBox → TheGamesDB.
/// Solo las disponibles (credenciales, base descargada, sistema arcade).
fn source_order(app: &tauri::AppHandle, settings: &AppSettings, system: &SystemRef) -> Vec<Source> {
    let arcade = crate::commands::scraper_adb::is_arcade_system(&system.name);
    let order: &[Source] = if arcade {
        &[Source::ScreenScraper, Source::ArcadeItalia, Source::LaunchBox]
    } else {
        &[Source::ScreenScraper, Source::LaunchBox, Source::TheGamesDb]
    };
    order
        .iter()
        .copied()
        .filter(|s| match s {
            Source::ScreenScraper => settings.has_screenscraper(),
            Source::ArcadeItalia => arcade,
            Source::LaunchBox => {
                crate::commands::scraper_launchbox::lb_platform(&system.name).is_some() && crate::commands::scraper_launchbox::is_downloaded(app)
            }
            Source::TheGamesDb => settings.has_tgdb(),
        })
        .collect()
}

async fn try_source(
    source: Source,
    app: &tauri::AppHandle,
    settings: &AppSettings,
    client: &Client,
    game: &Game,
    layout: &Layout,
    system: &SystemRef,
) -> Result<ScrapeResult, String> {
    match source {
        Source::ScreenScraper => scrape_game_screenscraper(settings, game, layout, system).await,
        Source::ArcadeItalia => crate::commands::scraper_adb::scrape_game_adb(client, game, layout, system, settings).await,
        Source::LaunchBox => crate::commands::scraper_launchbox::scrape_game_launchbox(app, client, game, layout, system, settings).await,
        Source::TheGamesDb => {
            let api_key = settings.tgdb_apikey.as_deref().unwrap_or_default();
            crate::commands::scraper_tgdb::scrape_game_tgdb(client, api_key, game, layout, system, settings).await
        }
    }
}

/// Automatic scrape of one game, without a manual search (kept for completeness; the UI uses
/// the manual search for single games). Same source order as Scrape All.
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

    if source_order(&app, &settings, &system).is_empty() {
        return Err(NO_SCRAPER.into());
    }
    let client = build_http_client()?;
    match scrape_one(&app, &settings, &client, &existing_game, &layout, &system).await {
        ("success", game) => Ok(ScrapeResult { game, found: true }),
        (_, game) => Ok(ScrapeResult { game, found: false }),
    }
}

/// Scrape a single game internally (used by batch): each source in order until one finds it.
/// Returns (status_str, updated_game): "success", "not_found" or "error" (every source failed).
async fn scrape_one(
    app: &tauri::AppHandle,
    settings: &AppSettings,
    client: &Client,
    game: &Game,
    layout: &Layout,
    system: &SystemRef,
) -> (&'static str, Game) {
    let mut answered = false;
    for source in source_order(app, settings, system) {
        match try_source(source, app, settings, client, game, layout, system).await {
            Ok(r) if r.found => return ("success", r.game),
            Ok(_) => answered = true,
            Err(_) => {}
        }
    }
    if answered {
        ("not_found", game.clone())
    } else {
        ("error", game.clone())
    }
}

/// «Scrape All» saves the gamelist every this many games (and at the end)
const SAVE_EVERY: usize = 10;

/// «Stop» pressed: the batch ends before its next game
static CANCEL_SCRAPE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Stop the running «Scrape All» (the game in progress finishes and what's done is saved)
#[tauri::command]
pub fn cancel_scrape() {
    CANCEL_SCRAPE.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[derive(serde::Serialize)]
pub struct ScrapeRunResult {
    /// Ended by «Stop»: the rest stays in the resume reminder
    pub stopped: bool,
}

/// ROM paths a batch will scrape, in gamelist order: all of them, or only `only`
fn paths_to_scrape(gamelist: &GameList, only: Option<&[String]>) -> Vec<String> {
    gamelist
        .games
        .iter()
        .filter(|g| only.map_or(true, |only| only.contains(&g.path)))
        .map(|g| g.path.clone())
        .collect()
}

/// Batch scrape all games in a system, emitting progress events.
/// Saves every SAVE_EVERY games (one backup, before starting) and keeps a reminder of the games
/// still to do, so a run cut halfway — app closed, crash or «Stop» — can be continued.
#[tauri::command]
pub async fn scrape_system(
    app: tauri::AppHandle,
    system_path: String,
    game_paths: Option<Vec<String>>,
    // What may be updated this time (chosen when launching); None = what's in Settings
    policy: Option<crate::models::settings::ScrapePolicy>,
    // Continue the run left halfway in this system (its games and rules)
    resume: Option<bool>,
) -> Result<ScrapeRunResult, String> {
    use super::scrape_resume::{self, ResumeEntry};

    let (layout, system) = super::system_context(&app, &system_path)?;
    let mut gamelist = crate::commands::gamelist::read_gamelist(&layout, &system)?;
    let resume_file = scrape_resume::resume_file(&app)?;
    let resume_key = scrape_resume::key(&layout, &system);

    // Continuing: the games, rules, total and start date of the run left halfway
    let previous = if resume == Some(true) {
        Some(scrape_resume::get(&resume_file, &resume_key).ok_or("Nothing to continue in this system")?)
    } else {
        None
    };
    let (only, policy) = match &previous {
        Some(entry) => (Some(entry.remaining.clone()), entry.policy.clone()),
        None => (game_paths, policy),
    };
    // Con `game_paths` solo se scrapean esos juegos (p. ej. las ROMs recién añadidas);
    // el resto pasa tal cual
    let todo = paths_to_scrape(&gamelist, only.as_deref());
    let total = todo.len();

    if total == 0 {
        // Lo que quedaba ya no está en la gamelist: no hay nada que continuar
        scrape_resume::remove(&resume_file, &resume_key)?;
        return Err("No games to scrape".to_string());
    }

    let mut settings: AppSettings = crate::commands::settings::get_settings(app.clone())?;
    if let Some(policy) = &policy {
        settings.scrape_fields = Some(policy.clone());
    }

    if source_order(&app, &settings, &system).is_empty() {
        return Err(NO_SCRAPER.into());
    }

    // The reminder, written now: if the app dies, it already says what's left
    let reminder = |remaining: &[String]| ResumeEntry {
        key: resume_key.clone(),
        remaining: remaining.to_vec(),
        policy: policy.clone(),
        total: previous.as_ref().map_or(total, |p| p.total),
        started_at: previous.as_ref().map_or_else(|| chrono::Utc::now().timestamp(), |p| p.started_at),
    };
    scrape_resume::put(&resume_file, reminder(&todo))?;

    // One backup for the whole run: the gamelist as it was before it
    crate::commands::backup::backup_gamelist(&layout, &system)?;

    CANCEL_SCRAPE.store(false, std::sync::atomic::Ordering::SeqCst);
    let client = build_http_client()?;
    let mut done = 0;
    let mut stopped = false;

    for (i, path) in todo.iter().enumerate() {
        if CANCEL_SCRAPE.load(std::sync::atomic::Ordering::SeqCst) {
            stopped = true;
            break;
        }
        let Some(index) = gamelist.games.iter().position(|g| &g.path == path) else {
            done += 1;
            continue;
        };
        let game = gamelist.games[index].clone();
        let game_name = game.name.clone().unwrap_or_else(|| game.path.clone());

        let _ = app.emit(
            "scrape-progress",
            ScrapeProgress {
                current: i + 1,
                total,
                game_name: game_name.clone(),
                game_path: game.path.clone(),
                status: "scraping".to_string(),
            },
        );

        if i > 0 {
            sleep(Duration::from_millis(REQUEST_DELAY_MS)).await;
        }

        let (status, updated_game) =
            scrape_one(&app, &settings, &client, &game, &layout, &system).await;
        gamelist.games[index] = updated_game;
        done += 1;

        let _ = app.emit(
            "scrape-progress",
            ScrapeProgress {
                current: i + 1,
                total,
                game_name,
                game_path: game.path.clone(),
                status: status.to_string(),
            },
        );

        // Cada SAVE_EVERY juegos: gamelist y recordatorio al día (el último guardado va abajo)
        if done % SAVE_EVERY == 0 && done < total {
            crate::commands::gamelist::write_gamelist_no_backup(&layout, &system, &gamelist)?;
            scrape_resume::put(&resume_file, reminder(&todo[done..]))?;
        }
    }

    crate::commands::gamelist::write_gamelist_no_backup(&layout, &system, &gamelist)?;
    if done < total {
        scrape_resume::put(&resume_file, reminder(&todo[done..]))?;
    } else {
        scrape_resume::remove(&resume_file, &resume_key)?;
    }

    Ok(ScrapeRunResult { stopped })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn follows_the_preferred_order_and_falls_back_to_the_first() {
        let names = serde_json::json!([
            {"region": "us", "text": "Alien Syndrome (USA)"},
            {"region": "eu", "text": "Alien Syndrome (Europa)"},
            {"region": "jp", "text": "Alien Syndrome (Japón)"},
        ]);
        assert_eq!(extract_regional(&names, &langs(&["eu", "wor", "us"])).as_deref(), Some("Alien Syndrome (Europa)"));
        assert_eq!(extract_regional(&names, &langs(&["us", "eu"])).as_deref(), Some("Alien Syndrome (USA)"));
        // Ninguna coincide: la primera
        assert_eq!(extract_regional(&names, &langs(&["br"])).as_deref(), Some("Alien Syndrome (USA)"));

        let synopsis = serde_json::json!([{"langue": "en", "text": "English"}, {"langue": "es", "text": "Español"}]);
        assert_eq!(extract_localized(&synopsis, &langs(&["es", "en"])).as_deref(), Some("Español"));
        assert_eq!(extract_localized(&synopsis, &langs(&["fr", "en"])).as_deref(), Some("English"));

        let medias = serde_json::json!([
            {"type": "ss", "region": "eu", "url": "ss-eu"},
            {"type": "box-2D", "region": "us", "url": "box-us"},
            {"type": "box-2D", "region": "eu", "url": "box-eu"},
        ]);
        assert_eq!(find_media_url(&medias, "box-2D", &langs(&["eu", "us"])).as_deref(), Some("box-eu"));
        assert_eq!(find_media_url(&medias, "box-2D", &langs(&["jp"])).as_deref(), Some("box-us"));
        assert_eq!(find_media_url(&medias, "video", &langs(&["eu"])), None);

        // El manual: la extensión sale de «format», no de la URL (mediaJeu.php?…)
        let medias = serde_json::json!([
            {"type": "manuel", "region": "eu", "url": "https://x.test/api2/mediaJeu.php?media=manuel(eu)", "format": "PDF"},
            {"type": "box-2D-back", "region": "eu", "url": "https://x.test/back"},
        ]);
        let (url, format) = find_media(&medias, "manuel", &langs(&["eu"])).unwrap();
        assert!(url.contains("manuel"));
        assert_eq!(format.as_deref(), Some("pdf"));
        assert_eq!(find_media(&medias, "box-2D-back", &langs(&["eu"])).unwrap().1, None);
    }

    #[test]
    fn default_orders_depend_on_the_app_language() {
        let mut s = AppSettings::default();
        s.language = Some("es".into());
        assert_eq!(s.scrape_regions()[0], "eu");
        assert_eq!(s.scrape_languages(), langs(&["es", "en"]));
        s.language = Some("en".into());
        assert_eq!(s.scrape_regions()[0], "us");
        assert_eq!(s.scrape_languages(), langs(&["en"]));
        s.ss_languages = Some(langs(&["fr"]));
        assert_eq!(s.scrape_languages(), langs(&["fr"]));
    }

    fn card(source: &str, id: &str) -> ScrapeCandidate {
        ScrapeCandidate {
            source: source.into(),
            id: id.into(),
            name: format!("Juego {}", id),
            system: None,
            year: None,
            developer: None,
            thumbnail: None,
            rom_match: false,
        }
    }

    #[test]
    fn rom_match_goes_first_and_is_not_repeated() {
        let mut matched = card("screenscraper", "2");
        matched.rom_match = true;
        let list = merge_candidates(
            Some(matched),
            vec![vec![card("screenscraper", "1"), card("screenscraper", "2")], vec![card("thegamesdb", "2")]],
        );
        let ids: Vec<(&str, &str, bool)> = list.iter().map(|c| (c.source.as_str(), c.id.as_str(), c.rom_match)).collect();
        // El mismo id en otra fuente es otro juego: se queda
        assert_eq!(ids, vec![("screenscraper", "2", true), ("screenscraper", "1", false), ("thegamesdb", "2", false)]);
    }

    #[test]
    fn reads_a_screenscraper_search_result() {
        let jeu = serde_json::json!({
            "id": "1234",
            "noms": [{"region": "us", "text": "Alien Syndrome"}, {"region": "jp", "text": "Alien Syndrome (J)"}],
            "systeme": {"id": "1", "text": "Megadrive"},
            "developpeur": {"id": "5", "text": "Sega"},
            "dates": [{"region": "jp", "text": "1987-07-01"}],
            "medias": [
                {"type": "ss", "region": "wor", "url": "https://example.test/ss"},
                {"type": "box-2D", "region": "us", "url": "https://example.test/box-us"},
            ],
        });
        let (c, cover) = parse_ss_candidate(&jeu, &langs(&["us"]), false).unwrap();
        assert_eq!(c.id, "1234");
        assert_eq!(c.name, "Alien Syndrome");
        assert_eq!(c.system.as_deref(), Some("Megadrive"));
        assert_eq!(c.developer.as_deref(), Some("Sega"));
        assert_eq!(c.year.as_deref(), Some("1987"));
        assert_eq!(cover.as_deref(), Some("https://example.test/box-us"));
        // Sin miniatura todavía: se pide aparte
        assert_eq!(c.thumbnail, None);

        // Un «juego» vacío (ScreenScraper sin resultados) no cuenta
        assert!(parse_ss_candidate(&serde_json::json!({}), &langs(&["us"]), false).is_none());
        assert!(real_game(&serde_json::json!({})).is_none());
    }

    #[test]
    fn a_batch_follows_the_gamelist_order_and_only_the_wanted_games() {
        let gamelist = GameList {
            games: ["./a.sfc", "./b.sfc", "./c.sfc"]
                .iter()
                .map(|p| Game { path: p.to_string(), ..Default::default() })
                .collect(),
            ..Default::default()
        };
        assert_eq!(paths_to_scrape(&gamelist, None), langs(&["./a.sfc", "./b.sfc", "./c.sfc"]));
        // Continuar: solo los que quedaban (y uno que ya no está en la gamelist no cuenta)
        let remaining = langs(&["./c.sfc", "./zz.sfc", "./b.sfc"]);
        assert_eq!(paths_to_scrape(&gamelist, Some(&remaining)), langs(&["./b.sfc", "./c.sfc"]));
    }
}
