use std::fs;
use std::path::Path;

use reqwest::Client;
use serde_json::Value;

use crate::layout::{Layout, SystemRef};
use crate::media;
use crate::models::game::Game;
use crate::models::scraper::ScrapeResult;
use crate::models::AppSettings;

const API_BASE: &str = "https://api.thegamesdb.net/v1";
const IMAGE_BASE: &str = "https://cdn.thegamesdb.net/images/original/";

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

/// Search for a game on TheGamesDB by name + platform.
pub async fn scrape_game_tgdb(
    client: &Client,
    api_key: &str,
    game: &Game,
    layout: &Layout,
    system: &SystemRef,
    settings: &AppSettings,
) -> Result<ScrapeResult, String> {
    let platform_id = tgdb_platform_id(&system.name);

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

    // Search by name
    let mut url = format!(
        "{}/Games/ByGameName?apikey={}&name={}&fields=players,publishers,genres,overview,rating&include=boxart",
        API_BASE,
        api_key,
        urlencoding::encode(&search_name)
    );
    if let Some(pid) = platform_id {
        url.push_str(&format!("&filter[platform]={}", pid));
    }

    let response = client
        .get(&url)
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

    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse TheGamesDB response: {}", e))?;

    let games_arr = match body["data"]["games"].as_array() {
        Some(arr) if !arr.is_empty() => arr,
        _ => {
            return Ok(ScrapeResult {
                game: game.clone(),
                found: false,
            });
        }
    };

    let jeu = &games_arr[0];
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

    // Resolve developers/publishers
    let developer = jeu["developers"]
        .as_array()
        .and_then(|ids| ids.first())
        .and_then(|id| id.as_u64())
        .map(|id| id.to_string()); // We'd need another API call for names

    let publisher = jeu["publishers"]
        .as_array()
        .and_then(|ids| ids.first())
        .and_then(|id| id.as_u64())
        .map(|id| id.to_string());

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
    let mut image: Option<String> = None;
    let mut thumbnail: Option<String> = None;

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
            } else if img_type == "screenshot" && need_thumbnail && thumbnail.is_none() {
                thumbnail = download_as(client, &img_url, layout, system, settings, "ss", &game.path, ext).await;
            }
        }
    }

    // If we have a game ID, fetch additional images
    if game_id > 0 && ((need_image && image.is_none()) || (need_thumbnail && thumbnail.is_none())) {
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

                    if img_type == "boxart" && need_image && image.is_none() {
                        let side = img["side"].as_str().unwrap_or("");
                        if side == "front" {
                            image = download_as(client, &img_url, layout, system, settings, "box-2D", &game.path, ext).await;
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
    policy.data("name").apply(&mut updated.name, name);
    policy.data("desc").apply(&mut updated.desc, desc);
    policy.data("genre").apply(&mut updated.genre, genre);
    policy.data("developer").apply(&mut updated.developer, developer);
    policy.data("publisher").apply(&mut updated.publisher, publisher);
    policy.data("players").apply(&mut updated.players, players);
    policy.data("rating").apply(&mut updated.rating, rating);
    policy.data("releasedate").apply(&mut updated.releasedate, releasedate);
    // Classic may map these media types to other fields; write each one where configured
    for (media_type, value) in [("box-2D", image), ("ss", thumbnail)] {
        if let (Some(value), Some(field)) = (value, media::field_for_scraper_type(layout, settings, media_type)) {
            media::set_field(&mut updated, &field, Some(value));
        }
    }

    Ok(ScrapeResult {
        game: updated,
        found: true,
    })
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
