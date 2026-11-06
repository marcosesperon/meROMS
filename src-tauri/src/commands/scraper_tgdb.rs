use std::collections::HashMap;
use std::fs;
use std::path::Path;

use reqwest::Client;
use serde_json::Value;

use crate::layout::{Layout, SystemRef};
use crate::media;
use crate::models::game::Game;
use crate::models::scraper::{ScrapeCandidate, ScrapeResult};
use crate::models::AppSettings;

const API_BASE: &str = "https://api.thegamesdb.net/v1";
const IMAGE_BASE: &str = "https://cdn.thegamesdb.net/images/original/";
const THUMB_BASE: &str = "https://cdn.thegamesdb.net/images/thumb/";

/// Maps ES-DE folder names to TheGamesDB platform IDs.
pub fn tgdb_platform_id(folder_name: &str) -> Option<u32> {
    match folder_name.to_lowercase().as_str() {
        "3do" => Some(25),
        "amiga" | "amiga600" | "amiga1200" => Some(4911),
        "amigacd32" => Some(4947),
        "amstradcpc" => Some(4914),
        "apple2" => Some(4942),
        "arcade" | "mame" | "fba" | "fbneo" | "cps" | "cps1" | "cps2" | "cps3" => Some(23),
        "arcadia" => Some(4963),
        "astrocde" => Some(4968),
        "atari2600" => Some(22),
        "atari5200" => Some(26),
        "atari7800" => Some(27),
        "atari800" | "atarixe" => Some(4943),
        "atarijaguar" | "jaguar" => Some(28),
        "atarilynx" | "lynx" => Some(4924),
        "atarist" => Some(4937),
        "bbcmicro" => Some(5013),
        "c64" => Some(40),
        "cdimono1" => Some(4917),
        "channelf" => Some(4928),
        "colecovision" => Some(31),
        "coco" => Some(4941),
        "dreamcast" => Some(16),
        "famicom" | "nes" => Some(7),
        "fds" => Some(4936),
        "fm7" => Some(4978),
        "fmtowns" => Some(4932),
        "gamegear" => Some(20),
        "gameandwatch" => Some(4950),
        "gamecom" => Some(4940),
        "gb" => Some(4),
        "gba" => Some(5),
        "gbc" => Some(41),
        "gc" | "gamecube" => Some(2),
        "genesis" => Some(18),
        "intellivision" => Some(32),
        "j2me" => Some(5018),
        "mastersystem" | "mark3" => Some(35),
        "megacd" | "segacd" => Some(21),
        "megadrive" | "megadrivejp" => Some(36),
        "megaduck" => Some(4948),
        "msx" | "msx1" | "msx2" | "msxturbor" => Some(4929),
        "n3ds" | "3ds" => Some(4912),
        "n64" => Some(3),
        "nds" => Some(8),
        "neogeo" => Some(24),
        "neogeocd" | "neogeocdjp" => Some(4956),
        "ngp" => Some(4922),
        "ngpc" => Some(4923),
        "odyssey2" | "videopac" => Some(4927),
        "oric" => Some(4986),
        "pc" | "dos" => Some(1),
        "pc88" => Some(4933),
        "pc98" => Some(4934),
        "pcengine" | "tg16" | "supergrafx" => Some(34),
        "pcenginecd" | "tg-cd" => Some(4955),
        "pcfx" => Some(4930),
        "pokemini" => Some(4957),
        "ps2" => Some(11),
        "ps3" => Some(12),
        "psp" => Some(13),
        "psvita" => Some(39),
        "psx" | "ps1" => Some(10),
        "pv1000" => Some(4964),
        "samcoupe" => Some(4979),
        "saturn" | "saturnjp" => Some(17),
        "sega32x" | "sega32xjp" | "sega32xna" => Some(33),
        "sg-1000" => Some(4949),
        "snes" | "snesna" | "sfc" => Some(6),
        "switch" => Some(4971),
        "vectrex" => Some(4939),
        "vic20" => Some(4945),
        "virtualboy" => Some(4918),
        "wii" => Some(9),
        "wiiu" => Some(38),
        "wonderswan" => Some(4925),
        "wonderswancolor" => Some(4926),
        "x1" => Some(4977),
        "x68000" => Some(4931),
        "zx81" => Some(5010),
        "zxspectrum" => Some(4913),
        "ngage" => Some(4938),
        "supervision" => Some(4959),
        "gamate" => Some(5004),
        _ => None,
    }
}

/// Fields asked for in every game query (developers come as ids, resolved with the name lists)
const GAME_FIELDS: &str = "players,publishers,genres,overview,rating";

/// GET a TheGamesDB URL and read the JSON (non-2xx → error with the start of the body)
async fn tgdb_get(client: &Client, url: &str) -> Result<Value, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("TheGamesDB request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "TheGamesDB API error: HTTP {} — {}",
            status,
            body.chars().take(300).collect::<String>()
        ));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse TheGamesDB response: {}", e))
}

/// Games whose name looks like `name`, on one platform (None = all of them)
async fn search_by_name(client: &Client, api_key: &str, name: &str, platform_id: Option<u32>) -> Result<Value, String> {
    let mut url = format!(
        "{}/Games/ByGameName?apikey={}&name={}&fields={}&include=boxart,platform",
        API_BASE,
        api_key,
        urlencoding::encode(name),
        GAME_FIELDS
    );
    if let Some(pid) = platform_id {
        url.push_str(&format!("&filter[platform]={}", pid));
    }
    tgdb_get(client, &url).await
}

/// Search for a game on TheGamesDB by name + platform and take the first result (automatic scrape).
pub async fn scrape_game_tgdb(
    client: &Client,
    api_key: &str,
    game: &Game,
    layout: &Layout,
    system: &SystemRef,
    settings: &AppSettings,
) -> Result<ScrapeResult, String> {
    // Use game name or derive from filename
    let search_name = game
        .name
        .clone()
        .unwrap_or_else(|| {
            Path::new(&game.path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&game.path)
                .to_string()
        });

    let body = search_by_name(client, api_key, &search_name, tgdb_platform_id(&system.name)).await?;
    match body["data"]["games"].as_array().and_then(|games| games.first()) {
        Some(jeu) => Ok(ScrapeResult {
            game: apply_tgdb_game(client, api_key, &body, jeu, game, layout, system, settings).await?,
            found: true,
        }),
        None => Ok(ScrapeResult {
            game: game.clone(),
            found: false,
        }),
    }
}

/// Manual search: every game TheGamesDB finds for `query`, as cards to pick from
pub async fn search_tgdb(
    client: &Client,
    api_key: &str,
    query: &str,
    platform_id: Option<u32>,
) -> Result<Vec<ScrapeCandidate>, String> {
    let body = search_by_name(client, api_key, query, platform_id).await?;
    let mut candidates = Vec::new();
    for (mut candidate, developer_id) in parse_tgdb_candidates(&body) {
        // The developer comes as an id: its name, from the list kept for the session
        if let Some(id) = developer_id {
            candidate.developer = tgdb_name(client, api_key, NameList::Developers, id).await;
        }
        candidates.push(candidate);
    }
    Ok(candidates)
}

/// The cards of a search response, each with its developer id (still to be turned into a name)
fn parse_tgdb_candidates(body: &Value) -> Vec<(ScrapeCandidate, Option<u64>)> {
    let Some(games) = body["data"]["games"].as_array() else {
        return Vec::new();
    };
    let thumb_base = body["include"]["boxart"]["base_url"]["thumb"]
        .as_str()
        .unwrap_or(THUMB_BASE);
    games
        .iter()
        .filter_map(|jeu| {
            let id = jeu["id"].as_u64()?;
            let name = jeu["game_title"].as_str()?.to_string();
            // Platform name from the «include=platform» part (it comes in two shapes)
            let platform = jeu["platform"].as_u64().map(|p| p.to_string());
            let system = platform.as_deref().and_then(|p| {
                let platforms = &body["include"]["platform"];
                platforms["data"][p]["name"]
                    .as_str()
                    .or_else(|| platforms[p]["name"].as_str())
                    .map(String::from)
            });
            let year = jeu["release_date"].as_str().filter(|d| d.len() >= 4).map(|d| d[..4].to_string());
            // Front box art as the thumbnail (public CDN, it can be shown as it is)
            let thumbnail = body["include"]["boxart"]["data"][id.to_string()]
                .as_array()
                .and_then(|images| {
                    images.iter().find(|img| {
                        img["type"].as_str() == Some("boxart") && img["side"].as_str() == Some("front")
                    })
                })
                .and_then(|img| img["filename"].as_str())
                .map(|file| format!("{}{}", thumb_base, file));
            let developer_id = jeu["developers"].as_array().and_then(|ids| ids.first()).and_then(|id| id.as_u64());
            Some((
                ScrapeCandidate {
                    source: "thegamesdb".into(),
                    id: id.to_string(),
                    name,
                    system,
                    year,
                    developer: None,
                    thumbnail,
                    rom_match: false,
                },
                developer_id,
            ))
        })
        .collect()
}

/// Manual search, once picked: the full data of one TheGamesDB game, written into `game`
pub async fn apply_tgdb_by_id(
    client: &Client,
    api_key: &str,
    id: u64,
    game: &Game,
    layout: &Layout,
    system: &SystemRef,
    settings: &AppSettings,
) -> Result<ScrapeResult, String> {
    let url = format!(
        "{}/Games/ByGameID?apikey={}&id={}&fields={}&include=boxart",
        API_BASE, api_key, id, GAME_FIELDS
    );
    let body = tgdb_get(client, &url).await?;
    let jeu = body["data"]["games"]
        .as_array()
        .and_then(|games| games.first())
        .ok_or("TheGamesDB: game not found")?;
    Ok(ScrapeResult {
        game: apply_tgdb_game(client, api_key, &body, jeu, game, layout, system, settings).await?,
        found: true,
    })
}

/// Write one TheGamesDB game (`jeu`, from `body`) into `game`: data and images, as the
/// policy says. Shared by the automatic scrape and the manual search.
#[allow(clippy::too_many_arguments)]
async fn apply_tgdb_game(
    client: &Client,
    api_key: &str,
    body: &Value,
    jeu: &Value,
    game: &Game,
    layout: &Layout,
    system: &SystemRef,
    settings: &AppSettings,
) -> Result<Game, String> {
    let game_id = jeu["id"].as_u64().unwrap_or(0);

    // Extract metadata
    let name = jeu["game_title"]
        .as_str()
        .map(String::from)
        .or_else(|| game.name.clone());

    let desc = jeu["overview"].as_str().map(String::from);

    let players = jeu["players"]
        .as_u64()
        .map(|p| p.to_string())
        .or_else(|| jeu["players"].as_str().map(String::from));

    let rating = jeu["rating"]
        .as_str()
        .and_then(|r| r.parse::<f64>().ok())
        .map(|r| format!("{:.2}", r / 10.0)); // TGDB uses 0-10

    let releasedate = jeu["release_date"].as_str().and_then(|d| {
        // TGDB format: "YYYY-MM-DD", convert to ES format "YYYYMMDDTHHMMSS"
        let cleaned = d.replace('-', "");
        if cleaned.len() >= 8 {
            Some(format!("{}T000000", &cleaned[..8]))
        } else {
            None
        }
    });

    // Resolve genres from IDs
    let genre = jeu["genres"]
        .as_array()
        .and_then(|ids| ids.first())
        .and_then(|id| id.as_u64())
        .and_then(|id| resolve_tgdb_genre(id));

    // Developers/publishers come as ids: their names come from TheGamesDB's lists
    // (fetched once per session). If a name can't be found, the field is left alone
    // (before, the bare id number was written).
    let first_id = |key: &str| jeu[key].as_array().and_then(|ids| ids.first()).and_then(|id| id.as_u64());
    let developer = match first_id("developers") {
        Some(id) => tgdb_name(client, api_key, NameList::Developers, id).await,
        None => None,
    };
    let publisher = match first_id("publishers") {
        Some(id) => tgdb_name(client, api_key, NameList::Publishers, id).await,
        None => None,
    };

    // Extract images from the include.boxart section.
    // Front box art counts as ScreenScraper's "box-2D" and screenshots as "ss",
    // so they land in the same unified folders (covers/, screenshots/).
    // Whether to download each type, as the policy says (overwrite / only if missing / leave)
    let policy = settings.scrape_policy();
    let wanted = |media_type: &str| match policy.media(media_type) {
        crate::models::settings::FieldPolicy::Overwrite => true,
        crate::models::settings::FieldPolicy::IfEmpty => !media::has_scraped_media(layout, system, settings, game, media_type),
        crate::models::settings::FieldPolicy::Keep => false,
    };
    let need_image = wanted("box-2D");
    let need_thumbnail = wanted("ss");
    // The back of the box, as ScreenScraper's «box-2D-back» (backcovers/)
    let need_back = wanted("box-2D-back");
    let mut image: Option<String> = None;
    let mut thumbnail: Option<String> = None;
    let mut back: Option<String> = None;

    let base_url = body["include"]["boxart"]["base_url"]["original"]
        .as_str()
        .unwrap_or(IMAGE_BASE);

    let game_id_str = game_id.to_string();
    if let Some(images) = body["include"]["boxart"]["data"][&game_id_str].as_array() {
        for img in images {
            let img_type = img["type"].as_str().unwrap_or("");
            let filename = img["filename"].as_str().unwrap_or("");
            let side = img["side"].as_str().unwrap_or("");

            if filename.is_empty() {
                continue;
            }

            let img_url = format!("{}{}", base_url, filename);
            let ext = filename.rsplit('.').next().unwrap_or("jpg");

            if img_type == "boxart" && side == "front" && need_image && image.is_none() {
                image = download_as(client, &img_url, layout, system, settings, "box-2D", &game.path, ext).await;
            } else if img_type == "boxart" && side == "back" && need_back && back.is_none() {
                back = download_as(client, &img_url, layout, system, settings, "box-2D-back", &game.path, ext).await;
            } else if img_type == "screenshot" && need_thumbnail && thumbnail.is_none() {
                thumbnail = download_as(client, &img_url, layout, system, settings, "ss", &game.path, ext).await;
            }
        }
    }

    // If we have a game ID, fetch additional images
    if game_id > 0 && ((need_image && image.is_none()) || (need_thumbnail && thumbnail.is_none()) || (need_back && back.is_none())) {
        if let Ok(extra_images) = fetch_game_images(client, api_key, game_id).await {
            let img_base = extra_images["data"]["base_url"]["original"]
                .as_str()
                .unwrap_or(IMAGE_BASE);

            if let Some(imgs) = extra_images["data"]["images"][&game_id_str].as_array() {
                for img in imgs {
                    let img_type = img["type"].as_str().unwrap_or("");
                    let filename = img["filename"].as_str().unwrap_or("");
                    if filename.is_empty() {
                        continue;
                    }

                    let img_url = format!("{}{}", img_base, filename);
                    let ext = filename.rsplit('.').next().unwrap_or("jpg");

                    if img_type == "boxart" {
                        let side = img["side"].as_str().unwrap_or("");
                        if side == "front" && need_image && image.is_none() {
                            image = download_as(client, &img_url, layout, system, settings, "box-2D", &game.path, ext).await;
                        } else if side == "back" && need_back && back.is_none() {
                            back = download_as(client, &img_url, layout, system, settings, "box-2D-back", &game.path, ext).await;
                        }
                    } else if img_type == "screenshot" && need_thumbnail && thumbnail.is_none() {
                        thumbnail = download_as(client, &img_url, layout, system, settings, "ss", &game.path, ext).await;
                    }
                }
            }
        }
    }

    // Clone existing game and overwrite only fields that TGDB provided
    let mut updated = game.clone();
    forget_numeric_ids(&mut updated);
    policy.data("name").apply(&mut updated.name, name);
    policy.data("desc").apply(&mut updated.desc, desc);
    policy.data("genre").apply(&mut updated.genre, genre);
    policy.data("developer").apply(&mut updated.developer, developer);
    policy.data("publisher").apply(&mut updated.publisher, publisher);
    policy.data("players").apply(&mut updated.players, players);
    policy.data("rating").apply(&mut updated.rating, rating);
    policy.data("releasedate").apply(&mut updated.releasedate, releasedate);
    // Classic may map these media types to other fields; write each one where configured
    for (media_type, value) in [("box-2D", image), ("ss", thumbnail), ("box-2D-back", back)] {
        if let (Some(value), Some(field)) = (value, media::field_for_scraper_type(layout, settings, media_type)) {
            media::set_field(&mut updated, &field, Some(value));
        }
    }

    Ok(updated)
}

/// Older versions saved TheGamesDB ids («1436») as developer/publisher. A value that is only
/// digits is treated as empty, so «only if empty» replaces it with the real name.
pub(crate) fn forget_numeric_ids(game: &mut Game) {
    for field in [&mut game.developer, &mut game.publisher] {
        if field.as_deref().is_some_and(|v| !v.trim().is_empty() && v.trim().chars().all(|c| c.is_ascii_digit())) {
            *field = None;
        }
    }
}

/// TheGamesDB lists that map ids to names
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum NameList {
    Developers,
    Publishers,
}

impl NameList {
    fn endpoint(self) -> &'static str {
        match self {
            NameList::Developers => "Developers",
            NameList::Publishers => "Publishers",
        }
    }
    fn key(self) -> &'static str {
        match self {
            NameList::Developers => "developers",
            NameList::Publishers => "publishers",
        }
    }
}

/// id → name lists already downloaded in this session (each list is one API call, and it
/// counts against the monthly allowance, so it's fetched only once)
static NAME_CACHE: std::sync::OnceLock<std::sync::Mutex<HashMap<NameList, HashMap<u64, String>>>> = std::sync::OnceLock::new();

/// Name of a developer/publisher by its TheGamesDB id (None if it can't be found)
async fn tgdb_name(client: &Client, api_key: &str, list: NameList, id: u64) -> Option<String> {
    let cache = NAME_CACHE.get_or_init(Default::default);
    if let Some(names) = cache.lock().ok()?.get(&list) {
        return names.get(&id).cloned();
    }
    let url = format!("{}/{}?apikey={}", API_BASE, list.endpoint(), api_key);
    let body: Value = client.get(&url).send().await.ok()?.json().await.ok()?;
    let names = parse_name_list(&body, list.key());
    let name = names.get(&id).cloned();
    cache.lock().ok()?.insert(list, names);
    name
}

/// `{"data": {"developers": {"1": {"id": 1, "name": "…"}, …}}}` (or an array) → id → name
fn parse_name_list(body: &Value, key: &str) -> HashMap<u64, String> {
    let entries: Vec<&Value> = match &body["data"][key] {
        Value::Object(map) => map.values().collect(),
        Value::Array(items) => items.iter().collect(),
        _ => Vec::new(),
    };
    entries
        .into_iter()
        .filter_map(|e| Some((e["id"].as_u64()?, e["name"].as_str()?.trim().to_string())))
        .filter(|(_, name)| !name.is_empty())
        .collect()
}

async fn fetch_game_images(client: &Client, api_key: &str, game_id: u64) -> Result<Value, String> {
    let url = format!(
        "{}/Games/Images?apikey={}&games_id={}",
        API_BASE, api_key, game_id
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    resp.json().await.map_err(|e| e.to_string())
}

/// Download one image as the given scraper media type; returns the value for the game field.
#[allow(clippy::too_many_arguments)]
async fn download_as(
    client: &Client,
    url: &str,
    layout: &Layout,
    system: &SystemRef,
    settings: &AppSettings,
    media_type: &str,
    rom_path: &str,
    ext: &str,
) -> Option<String> {
    let target = media::scraped_media_target(layout, system, settings, media_type, rom_path, ext)?;
    download_image(client, url, &target.dest).await.ok()?;
    // «Overwrite»: the old one with another extension (x.jpg vs the new x.png) goes away
    if settings.scrape_policy().media(media_type) == crate::models::settings::FieldPolicy::Overwrite {
        media::trash_replaced_extensions(layout, system, media_type, rom_path, &target.dest);
    }
    Some(target.value)
}

async fn download_image(client: &Client, url: &str, dest: &Path) -> Result<(), String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    // Un relleno aprendido: como si no hubiera
    if crate::commands::placeholders::is_placeholder(&bytes) {
        return Err("Placeholder media, skipped".into());
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(dest, &bytes).map_err(|e| e.to_string())
}

/// Basic genre resolution for common TGDB genre IDs.
fn resolve_tgdb_genre(id: u64) -> Option<String> {
    match id {
        1 => Some("Action".into()),
        2 => Some("Adventure".into()),
        3 => Some("Construction and Management Simulation".into()),
        4 => Some("RPG".into()),
        5 => Some("Puzzle".into()),
        6 => Some("Strategy".into()),
        7 => Some("Racing".into()),
        8 => Some("Shooter".into()),
        9 => Some("Sports".into()),
        10 => Some("Fighting".into()),
        11 => Some("Platformer".into()),
        12 => Some("Stealth".into()),
        13 => Some("Music".into()),
        14 => Some("Horror".into()),
        15 => Some("MMO".into()),
        16 => Some("Sandbox".into()),
        17 => Some("Flight Simulator".into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_developer_and_publisher_lists() {
        let body: Value = serde_json::from_str(
            r#"{"data": {"count": 2, "developers": {"1": {"id": 1, "name": "Capcom"}, "7": {"id": 7, "name": " Konami "}}}}"#,
        )
        .unwrap();
        let names = parse_name_list(&body, "developers");
        assert_eq!(names.get(&1).map(String::as_str), Some("Capcom"));
        assert_eq!(names.get(&7).map(String::as_str), Some("Konami"));

        // Formato en lista, y sin datos
        let array: Value = serde_json::from_str(r#"{"data": {"publishers": [{"id": 3, "name": "Sega"}]}}"#).unwrap();
        assert_eq!(parse_name_list(&array, "publishers").get(&3).map(String::as_str), Some("Sega"));
        assert!(parse_name_list(&serde_json::json!({}), "publishers").is_empty());
    }

    #[test]
    fn numeric_developer_and_publisher_count_as_empty() {
        let mut game = Game { developer: Some("1436".into()), publisher: Some("Capcom".into()), ..Default::default() };
        forget_numeric_ids(&mut game);
        assert_eq!(game.developer, None);
        assert_eq!(game.publisher.as_deref(), Some("Capcom"));
    }

    #[test]
    fn reads_a_search_response_as_cards() {
        let body = serde_json::json!({
            "data": {"games": [
                {"id": 10, "game_title": "Alien Syndrome", "release_date": "1988-01-01", "platform": 18, "developers": [7]},
                {"id": 11, "game_title": "Sin plataforma"},
                {"game_title": "Sin id"},
            ]},
            "include": {
                "boxart": {
                    "base_url": {"thumb": "https://cdn.example.test/thumb/"},
                    "data": {"10": [
                        {"type": "boxart", "side": "back", "filename": "back/10.jpg"},
                        {"type": "boxart", "side": "front", "filename": "front/10.jpg"},
                    ]},
                },
                "platform": {"data": {"18": {"id": 18, "name": "Sega Mega Drive"}}},
            },
        });
        let cards = parse_tgdb_candidates(&body);
        assert_eq!(cards.len(), 2);
        let (first, developer) = &cards[0];
        assert_eq!(first.source, "thegamesdb");
        assert_eq!(first.id, "10");
        assert_eq!(first.year.as_deref(), Some("1988"));
        assert_eq!(first.system.as_deref(), Some("Sega Mega Drive"));
        assert_eq!(first.thumbnail.as_deref(), Some("https://cdn.example.test/thumb/front/10.jpg"));
        assert_eq!(*developer, Some(7));
        let (second, _) = &cards[1];
        assert_eq!(second.system, None);
        assert_eq!(second.thumbnail, None);
    }
}
