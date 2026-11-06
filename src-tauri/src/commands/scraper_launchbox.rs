//! LaunchBox Games DB como fuente del scraper. Es una base de datos que se descarga entera
//! (`Metadata.zip`, ~110 MB; dentro un XML de ~500 MB): sin cuenta, sin cuota y sin esperas.
//!
//! Al descargarla se lee el XML poco a poco y se guarda resumida en la carpeta de datos de la
//! app, un fichero por plataforma (`launchbox/<plataforma>.json`): al scrapear un sistema solo
//! se carga el suyo. Las imágenes se bajan de su web al usarlas.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use super::scrape_common::{apply_data, apply_media, es_date, fetch_media, normalize_title, ScrapedData};
use crate::layout::{Layout, SystemRef};
use crate::models::game::Game;
use crate::models::scraper::{ScrapeCandidate, ScrapeResult};
use crate::models::AppSettings;

const METADATA_URL: &str = "https://gamesdb.launchbox-app.com/Metadata.zip";
const IMAGE_BASE: &str = "https://images.launchbox-app.com/";

// ---------- Tablas ----------

/// Plataforma de LaunchBox de cada sistema (carpeta de ES-DE)
pub fn lb_platform(folder_name: &str) -> Option<&'static str> {
    Some(match folder_name.to_lowercase().as_str() {
        "3do" => "3DO Interactive Multiplayer",
        "adam" => "Coleco ADAM",
        "amiga" | "amiga600" | "amiga1200" => "Commodore Amiga",
        "amigacd32" => "Commodore Amiga CD32",
        "amstradcpc" => "Amstrad CPC",
        "androidapps" | "androidgames" => "Android",
        "apple2" => "Apple II",
        "apple2gs" => "Apple IIGS",
        "arcade" | "mame" | "fba" | "fbneo" | "cps" | "cps1" | "cps2" | "cps3" => "Arcade",
        "arcadia" => "Emerson Arcadia 2001",
        "archimedes" => "Acorn Archimedes",
        "arduboy" => "Arduboy",
        "astrocde" => "Bally Astrocade",
        "atari2600" => "Atari 2600",
        "atari5200" => "Atari 5200",
        "atari7800" => "Atari 7800",
        "atari800" => "Atari 800",
        "atarijaguar" => "Atari Jaguar",
        "atarijaguarcd" => "Atari Jaguar CD",
        "atarilynx" => "Atari Lynx",
        "atarist" => "Atari ST",
        "atarixe" => "Atari XEGS",
        "atomiswave" => "Sammy Atomiswave",
        "bbcmicro" => "BBC Microcomputer System",
        "c64" => "Commodore 64",
        "cdimono1" => "Philips CD-i",
        "cdtv" => "Commodore CDTV",
        "channelf" => "Fairchild Channel F",
        "coco" => "TRS-80 Color Computer",
        "colecovision" => "ColecoVision",
        "crvision" => "VTech CreatiVision",
        "dos" | "pc" => "MS-DOS",
        "dragon32" | "tanodragon" => "Dragon 32/64",
        "dreamcast" => "Sega Dreamcast",
        "electron" => "Acorn Electron",
        "famicom" | "nes" => "Nintendo Entertainment System",
        "fds" => "Nintendo Famicom Disk System",
        "fm7" => "Fujitsu FM-7",
        "fmtowns" => "Fujitsu FM Towns Marty",
        "gameandwatch" => "Nintendo Game & Watch",
        "gamecom" => "Tiger Game.com",
        "gamegear" => "Sega Game Gear",
        "gb" => "Nintendo Game Boy",
        "gba" => "Nintendo Game Boy Advance",
        "gbc" => "Nintendo Game Boy Color",
        "gc" => "Nintendo GameCube",
        "genesis" | "megadrive" | "megadrivejp" => "Sega Genesis",
        "gmaster" => "Hartung Game Master",
        "gx4000" => "Amstrad GX4000",
        "intellivision" => "Mattel Intellivision",
        "macintosh" => "Apple Mac OS",
        "mark3" | "mastersystem" => "Sega Master System",
        "megacd" | "megacdjp" | "segacd" => "Sega CD",
        "megaduck" => "Mega Duck",
        "model2" => "Sega Model 2",
        "model3" => "Sega Model 3",
        "msx" | "msx1" => "Microsoft MSX",
        "msx2" => "Microsoft MSX2",
        "multivision" => "Othello Multivision",
        "n3ds" => "Nintendo 3DS",
        "n64" => "Nintendo 64",
        "n64dd" => "Nintendo 64DD",
        "naomi" | "naomigd" => "Sega Naomi",
        "naomi2" => "Sega Naomi 2",
        "nds" => "Nintendo DS",
        "neogeo" => "SNK Neo Geo AES",
        "neogeocd" | "neogeocdjp" => "SNK Neo Geo CD",
        "ngage" => "Nokia N-Gage",
        "ngp" => "SNK Neo Geo Pocket",
        "ngpc" => "SNK Neo Geo Pocket Color",
        "odyssey2" | "videopac" => "Magnavox Odyssey 2",
        "openbor" => "OpenBOR",
        "oric" => "Oric Atmos",
        "pc88" => "NEC PC-8801",
        "pc98" => "NEC PC-9801",
        "pcengine" | "tg16" => "NEC TurboGrafx-16",
        "pcenginecd" | "tg-cd" => "NEC TurboGrafx-CD",
        "pcfx" => "NEC PC-FX",
        "pico8" => "PICO-8",
        "plus4" => "Commodore Plus 4",
        "pokemini" => "Nintendo Pokemon Mini",
        "ps2" => "Sony Playstation 2",
        "ps3" => "Sony Playstation 3",
        "psp" => "Sony PSP",
        "psvita" => "Sony Playstation Vita",
        "psx" => "Sony Playstation",
        "pv1000" => "Casio PV-1000",
        "samcoupe" => "SAM Coupé",
        "satellaview" => "Nintendo Satellaview",
        "saturn" | "saturnjp" => "Sega Saturn",
        "scummvm" => "ScummVM",
        "scv" => "Epoch Super Cassette Vision",
        "sega32x" | "sega32xjp" | "sega32xna" => "Sega 32X",
        "sfc" | "snes" | "snesna" => "Super Nintendo Entertainment System",
        "sg-1000" => "Sega SG-1000",
        "spectravideo" => "Spectravideo",
        "stv" => "Sega ST-V",
        "supergrafx" => "PC Engine SuperGrafx",
        "supervision" => "Watara Supervision",
        "supracan" => "Funtech Super Acan",
        "switch" => "Nintendo Switch",
        "ti99" => "Texas Instruments TI 99/4A",
        "uzebox" => "Uzebox",
        "vectrex" => "GCE Vectrex",
        "vic20" => "Commodore VIC-20",
        "virtualboy" => "Nintendo Virtual Boy",
        "vpinball" => "Pinball",
        "vsmile" => "VTech V.Smile",
        "wasm4" => "WASM-4",
        "wii" => "Nintendo Wii",
        "wiiu" => "Nintendo Wii U",
        "windows" | "windows9x" => "Windows",
        "windows3x" => "Windows 3.X",
        "wonderswan" => "WonderSwan",
        "wonderswancolor" => "WonderSwan Color",
        "x1" => "Sharp X1",
        "x68000" => "Sharp X68000",
        "zx81" => "Sinclair ZX-81",
        "zxspectrum" => "Sinclair ZX Spectrum",
        _ => return None,
    })
}

/// Tipo de imagen de LaunchBox → tipo del scraper de meROMS (los que «Qué actualiza el
/// scraper» deja elegir). Los demás no se guardan. Se aplica al crear la base local: un tipo
/// nuevo aquí solo llega tras volver a descargarla.
fn media_type_for(lb_type: &str) -> Option<&'static str> {
    Some(match lb_type {
        "Box - Front" => "box-2D",
        "Box - 3D" => "box-3D",
        "Screenshot - Gameplay" => "ss",
        "Screenshot - Game Title" => "sstitle",
        "Clear Logo" => "wheel",
        "Arcade - Marquee" => "marquee",
        "Fanart - Background" => "fanart",
        "Box - Back" => "box-2D-back",
        "Cart - Front" | "Disc" => "support-2D",
        _ => return None,
    })
}

/// Regiones de LaunchBox que corresponden a cada código de región de Ajustes (ScreenScraper)
fn lb_regions(code: &str) -> &'static [&'static str] {
    match code {
        "wor" => &["World"],
        "eu" => &["Europe"],
        "us" => &["North America", "United States"],
        "jp" => &["Japan"],
        "uk" => &["United Kingdom"],
        "fr" => &["France"],
        "de" => &["Germany"],
        "es" => &["Spain"],
        "it" => &["Italy"],
        "nl" => &["The Netherlands"],
        "au" => &["Australia", "Oceania"],
        "br" => &["Brazil"],
        "kr" => &["Korea"],
        "cn" => &["China"],
        "asi" => &["Asia"],
        "ru" => &["Russia"],
        "ca" => &["Canada"],
        _ => &[],
    }
}

// ---------- Datos guardados ----------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LbImage {
    /// Tipo del scraper de meROMS (box-2D, ss…)
    #[serde(rename = "t")]
    pub media_type: String,
    #[serde(rename = "f")]
    pub file: String,
    #[serde(rename = "r", default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LbGame {
    pub id: u64,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alt: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    /// «AAAA-MM-DD» o solo el año
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publ: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub players: Option<String>,
    /// Nota de la comunidad, 0..5
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<LbImage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LbMeta {
    /// Cuándo se descargó (segundos Unix)
    downloaded_at: i64,
    games: usize,
    platforms: usize,
    /// Nombres de las plataformas (los ficheros van por «slug»)
    #[serde(default)]
    names: Vec<String>,
}

/// Lo que enseña Ajustes
#[derive(Debug, Clone, Serialize)]
pub struct LaunchBoxStatus {
    pub downloaded: bool,
    pub downloaded_at: Option<i64>,
    pub games: usize,
    pub platforms: usize,
}

fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    Ok(dir.join("launchbox"))
}

fn slug(platform: &str) -> String {
    platform
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn read_meta(dir: &Path) -> Option<LbMeta> {
    serde_json::from_str(&fs::read_to_string(dir.join("meta.json")).ok()?).ok()
}

/// ¿Está descargada? (lo usa scraper_status)
pub fn is_downloaded(app: &tauri::AppHandle) -> bool {
    data_dir(app).ok().and_then(|d| read_meta(&d)).is_some()
}

#[tauri::command]
pub fn launchbox_status(app: tauri::AppHandle) -> Result<LaunchBoxStatus, String> {
    let meta = read_meta(&data_dir(&app)?);
    Ok(LaunchBoxStatus {
        downloaded: meta.is_some(),
        downloaded_at: meta.as_ref().map(|m| m.downloaded_at),
        games: meta.as_ref().map_or(0, |m| m.games),
        platforms: meta.as_ref().map_or(0, |m| m.platforms),
    })
}

#[tauri::command]
pub fn launchbox_delete(app: tauri::AppHandle) -> Result<(), String> {
    let dir = data_dir(&app)?;
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    clear_cache();
    Ok(())
}

// ---------- Descarga y lectura ----------

#[derive(Debug, Clone, Serialize)]
struct LbProgress {
    /// «download» | «parse»
    phase: &'static str,
    current: u64,
    total: u64,
}

/// Descarga Metadata.zip y lo guarda resumido por plataforma. Emite `launchbox-progress`.
#[tauri::command]
pub async fn launchbox_download(app: tauri::AppHandle) -> Result<LaunchBoxStatus, String> {
    let base = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let zip_path = base.join("launchbox_metadata.zip");

    // Sin límite de tiempo total (son 110 MB), solo para conectar
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .user_agent(format!("meROMS/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client.get(METADATA_URL).send().await.map_err(|e| format!("Download failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }
    let total = response.content_length().unwrap_or(0);
    let mut file = File::create(&zip_path).map_err(|e| e.to_string())?;
    let mut done = 0u64;
    let mut last_emit = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(|e| format!("Download failed: {}", e))? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        if done - last_emit > 1 << 20 {
            last_emit = done;
            let _ = app.emit("launchbox-progress", LbProgress { phase: "download", current: done, total });
        }
    }
    drop(file);

    let dir = data_dir(&app)?;
    let app2 = app.clone();
    let zip2 = zip_path.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        build_database(&zip2, &dir, |current, total| {
            let _ = app2.emit("launchbox-progress", LbProgress { phase: "parse", current, total });
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&zip_path);
    result?;
    clear_cache();
    launchbox_status(app)
}

/// Lector que va contando lo leído (para la barra mientras se lee el XML)
struct Counting<R, F: FnMut(u64)> {
    inner: R,
    read: u64,
    last: u64,
    on_progress: F,
}

impl<R: Read, F: FnMut(u64)> Read for Counting<R, F> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        if self.read - self.last > 5 << 20 {
            self.last = self.read;
            (self.on_progress)(self.read);
        }
        Ok(n)
    }
}

/// Lee el zip y deja la base resumida en `dir` (se escribe aparte y se cambia al final, así
/// una lectura fallida no estropea la copia anterior)
fn build_database(zip_path: &Path, dir: &Path, mut progress: impl FnMut(u64, u64)) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(File::open(zip_path).map_err(|e| e.to_string())?).map_err(|e| format!("Not a valid zip: {}", e))?;

    let (platforms, games) = {
        let entry = zip.by_name("Metadata.xml").map_err(|_| "Metadata.xml not found in the zip".to_string())?;
        let total = entry.size();
        let reader = Counting { inner: entry, read: 0, last: 0, on_progress: |n| progress(n, total) };
        parse_metadata(BufReader::with_capacity(1 << 20, reader))?
    };
    let mame = match zip.by_name("Mame.xml") {
        Ok(entry) => parse_mame(BufReader::new(entry))?,
        Err(_) => HashMap::new(),
    };

    let tmp = dir.with_extension("tmp");
    if tmp.exists() {
        fs::remove_dir_all(&tmp).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    for (platform, list) in &platforms {
        let json = serde_json::to_vec(list).map_err(|e| e.to_string())?;
        fs::write(tmp.join(format!("{}.json", slug(platform))), json).map_err(|e| e.to_string())?;
    }
    fs::write(tmp.join("mame.json"), serde_json::to_vec(&mame).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut names: Vec<String> = platforms.keys().cloned().collect();
    names.sort();
    let meta = LbMeta { downloaded_at: chrono::Utc::now().timestamp(), games, platforms: platforms.len(), names };
    fs::write(tmp.join("meta.json"), serde_json::to_vec(&meta).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, dir).map_err(|e| e.to_string())?;
    Ok(())
}

/// Lee Metadata.xml: juegos por plataforma, con sus nombres alternativos e imágenes útiles.
/// Devuelve también cuántos juegos hay.
pub fn parse_metadata<R: std::io::BufRead>(input: R) -> Result<(HashMap<String, Vec<LbGame>>, usize), String> {
    let mut reader = Reader::from_reader(input);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut section: Vec<u8> = Vec::new();
    let mut field: Option<String> = None;
    let mut fields: HashMap<String, String> = HashMap::new();

    let mut platforms: HashMap<String, Vec<LbGame>> = HashMap::new();
    let mut where_is: HashMap<u64, (String, usize)> = HashMap::new();
    // Nombres alternativos e imágenes pueden venir antes o después de su juego: se guardan aparte
    let mut alts: Vec<(u64, String)> = Vec::new();
    let mut images: Vec<(u64, LbImage)> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                if depth == 1 {
                    section = name;
                    fields.clear();
                } else if depth == 2 {
                    field = Some(String::from_utf8_lossy(&name).to_string());
                }
                depth += 1;
            }
            Ok(Event::Text(t)) if depth == 3 => {
                if let Some(f) = &field {
                    let text = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                    fields.entry(f.clone()).or_default().push_str(&text);
                }
            }
            Ok(Event::CData(t)) if depth == 3 => {
                if let Some(f) = &field {
                    fields.entry(f.clone()).or_default().push_str(&String::from_utf8_lossy(&t));
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 2 {
                    field = None;
                } else if depth == 1 {
                    let id = fields.get("DatabaseID").and_then(|v| v.trim().parse::<u64>().ok());
                    match (section.as_slice(), id) {
                        (b"Game", Some(id)) => {
                            if let Some((platform, game)) = game_from_fields(id, &fields) {
                                let list = platforms.entry(platform.clone()).or_default();
                                where_is.insert(id, (platform, list.len()));
                                list.push(game);
                            }
                        }
                        (b"GameAlternateName", Some(id)) => {
                            if let Some(name) = fields.get("AlternateName").map(|s| s.trim()).filter(|s| !s.is_empty()) {
                                alts.push((id, name.to_string()));
                            }
                        }
                        (b"GameImage", Some(id)) => {
                            let kind = fields.get("Type").and_then(|t| media_type_for(t.trim()));
                            let file = fields.get("FileName").map(|f| f.trim().to_string()).filter(|f| !f.is_empty());
                            if let (Some(kind), Some(file)) = (kind, file) {
                                let region = fields.get("Region").map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
                                images.push((id, LbImage { media_type: kind.to_string(), file, region }));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("LaunchBox XML: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    for (id, name) in alts {
        if let Some((platform, index)) = where_is.get(&id) {
            let game = &mut platforms.get_mut(platform).unwrap()[*index];
            if !game.alt.contains(&name) && game.name != name {
                game.alt.push(name);
            }
        }
    }
    for (id, image) in images {
        if let Some((platform, index)) = where_is.get(&id) {
            platforms.get_mut(platform).unwrap()[*index].images.push(image);
        }
    }
    let count = where_is.len();
    Ok((platforms, count))
}

fn game_from_fields(id: u64, f: &HashMap<String, String>) -> Option<(String, LbGame)> {
    let get = |k: &str| f.get(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let platform = get("Platform")?;
    let name = get("Name")?;
    Some((
        platform,
        LbGame {
            id,
            name,
            alt: Vec::new(),
            desc: get("Overview"),
            date: get("ReleaseDate").map(|d| d.chars().take(10).collect()).or_else(|| get("ReleaseYear")),
            dev: get("Developer"),
            publ: get("Publisher"),
            genre: get("Genres").and_then(|g| g.split(';').next().map(|s| s.trim().to_string())).filter(|g| !g.is_empty()),
            players: get("MaxPlayers").filter(|p| p != "0"),
            rating: get("CommunityRating").and_then(|r| r.parse::<f32>().ok()),
            images: Vec::new(),
        },
    ))
}

/// Mame.xml: nombre corto de MAME (el del zip) → título del juego en LaunchBox
pub fn parse_mame<R: std::io::BufRead>(input: R) -> Result<HashMap<String, String>, String> {
    let mut reader = Reader::from_reader(input);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut field: Option<Vec<u8>> = None;
    let (mut file, mut name) = (String::new(), String::new());
    let mut out = HashMap::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if depth == 1 {
                    file.clear();
                    name.clear();
                } else if depth == 2 {
                    field = Some(e.name().as_ref().to_vec());
                }
                depth += 1;
            }
            Ok(Event::Text(t)) if depth == 3 => {
                let text = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                match field.as_deref() {
                    Some(b"FileName") => file.push_str(&text),
                    Some(b"Name") => name.push_str(&text),
                    _ => {}
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 2 {
                    field = None;
                } else if depth == 1 && !file.trim().is_empty() && !name.trim().is_empty() {
                    out.insert(file.trim().to_lowercase(), name.trim().to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("LaunchBox Mame.xml: {}", e)),
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

// ---------- Consultas ----------

/// Los juegos de una plataforma, con un índice por nombre normalizado (nombre y alternativos)
pub struct LbPlatform {
    pub name: String,
    pub games: Vec<LbGame>,
    by_name: HashMap<String, Vec<usize>>,
}

impl LbPlatform {
    pub fn new(name: &str, games: Vec<LbGame>) -> Self {
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, g) in games.iter().enumerate() {
            for n in std::iter::once(&g.name).chain(g.alt.iter()) {
                let entry = by_name.entry(normalize_title(n)).or_default();
                if !entry.contains(&i) {
                    entry.push(i);
                }
            }
        }
        LbPlatform { name: name.to_string(), games, by_name }
    }

    pub fn find(&self, title: &str) -> Option<&LbGame> {
        self.by_name.get(&normalize_title(title)).and_then(|ids| ids.first()).map(|&i| &self.games[i])
    }

    /// Los que contienen el texto (normalizado), primero los exactos y los que empiezan así
    pub fn search(&self, query: &str, limit: usize) -> Vec<&LbGame> {
        let q = normalize_title(query);
        if q.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<(u8, &LbGame)> = self
            .games
            .iter()
            .filter_map(|g| {
                std::iter::once(&g.name)
                    .chain(g.alt.iter())
                    .map(|n| {
                        let n = normalize_title(n);
                        if n == q {
                            Some(0)
                        } else if n.starts_with(&q) {
                            Some(1)
                        } else if n.contains(&q) {
                            Some(2)
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .min()
                    .map(|score| (score, g))
            })
            .collect();
        // A igual coincidencia, primero los que más imágenes tienen (los conocidos, no los
        // hacks con nombre corto) y luego los de nombre más corto
        found.sort_by(|a, b| {
            (a.0, std::cmp::Reverse(a.1.images.len()), a.1.name.len()).cmp(&(b.0, std::cmp::Reverse(b.1.images.len()), b.1.name.len()))
        });
        found.into_iter().take(limit).map(|(_, g)| g).collect()
    }
}

/// Plataformas ya cargadas en esta sesión (para no releer el JSON en cada juego de un lote)
static CACHE: Mutex<Option<HashMap<String, Arc<LbPlatform>>>> = Mutex::new(None);

fn clear_cache() {
    if let Ok(mut c) = CACHE.lock() {
        *c = None;
    }
}

fn load_platform(app: &tauri::AppHandle, platform: &str) -> Option<Arc<LbPlatform>> {
    let mut guard = CACHE.lock().ok()?;
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(p) = cache.get(platform) {
        return Some(p.clone());
    }
    let file = data_dir(app).ok()?.join(format!("{}.json", slug(platform)));
    let games: Vec<LbGame> = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
    let loaded = Arc::new(LbPlatform::new(platform, games));
    cache.insert(platform.to_string(), loaded.clone());
    Some(loaded)
}

/// Título de LaunchBox de un nombre corto de MAME
fn mame_title(app: &tauri::AppHandle, short_name: &str) -> Option<String> {
    let map: HashMap<String, String> = serde_json::from_slice(&fs::read(data_dir(app).ok()?.join("mame.json")).ok()?).ok()?;
    map.get(&short_name.to_lowercase()).cloned()
}

fn rom_stem(game: &Game) -> String {
    Path::new(&game.path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// La imagen de un tipo, en el orden de regiones de Ajustes (sin región, después; luego cualquiera)
pub fn pick_image<'a>(game: &'a LbGame, media_type: &str, regions: &[String]) -> Option<&'a LbImage> {
    let of_type: Vec<&LbImage> = game.images.iter().filter(|i| i.media_type == media_type).collect();
    regions
        .iter()
        .flat_map(|code| lb_regions(code).iter())
        .find_map(|name| of_type.iter().find(|i| i.region.as_deref() == Some(*name)).copied())
        .or_else(|| of_type.iter().find(|i| i.region.is_none()).copied())
        .or_else(|| of_type.first().copied())
}

/// El juego de LaunchBox de un juego de la gamelist (automático): en arcade, por el nombre del
/// zip (tabla de MAME); si no, por su título o por el nombre del fichero
fn find_for(app: &tauri::AppHandle, system: &SystemRef, game: &Game) -> Option<(Arc<LbPlatform>, usize)> {
    let platform = load_platform(app, lb_platform(&system.name)?)?;
    let stem = rom_stem(game);
    let mut titles: Vec<String> = Vec::new();
    if platform.name == "Arcade" || crate::commands::scraper_adb::is_arcade_system(&system.name) {
        titles.extend(mame_title(app, &stem));
    }
    titles.extend(game.name.clone());
    titles.push(stem);
    let found = titles.iter().find_map(|t| platform.find(t))?;
    let index = platform.games.iter().position(|g| g.id == found.id)?;
    Some((platform, index))
}

/// Datos y media de un juego de LaunchBox escritos en el juego de la gamelist
async fn apply_lb_game(client: &Client, lb: &LbGame, game: &Game, layout: &Layout, system: &SystemRef, settings: &AppSettings) -> Game {
    let data = ScrapedData {
        name: Some(lb.name.clone()),
        desc: lb.desc.clone(),
        genre: lb.genre.clone(),
        developer: lb.dev.clone(),
        publisher: lb.publ.clone(),
        players: lb.players.clone(),
        rating: lb.rating.map(|r| format!("{:.2}", (r / 5.0).clamp(0.0, 1.0))),
        releasedate: lb.date.as_deref().and_then(es_date),
    };
    let mut updated = apply_data(game, data, settings);
    let regions = settings.scrape_regions();
    let mut downloaded = Vec::new();
    for media_type in ["box-2D", "box-3D", "ss", "sstitle", "wheel", "marquee", "fanart"] {
        if let Some(image) = pick_image(lb, media_type, &regions) {
            let url = format!("{}{}", IMAGE_BASE, image.file);
            downloaded.extend(fetch_media(client, &url, layout, system, settings, game, media_type).await);
        }
    }
    apply_media(&mut updated, downloaded);
    updated
}

/// Automático: busca el juego en LaunchBox y lo aplica. Sin base descargada, «no encontrado».
pub async fn scrape_game_launchbox(
    app: &tauri::AppHandle,
    client: &Client,
    game: &Game,
    layout: &Layout,
    system: &SystemRef,
    settings: &AppSettings,
) -> Result<ScrapeResult, String> {
    let Some((platform, index)) = find_for(app, system, game) else {
        return Ok(ScrapeResult { game: game.clone(), found: false });
    };
    let lb = platform.games[index].clone();
    Ok(ScrapeResult { game: apply_lb_game(client, &lb, game, layout, system, settings).await, found: true })
}

/// Búsqueda manual: fichas de LaunchBox para un texto (en la plataforma del sistema, o en todas)
pub fn search_launchbox(app: &tauri::AppHandle, system: &SystemRef, query: &str, all_systems: bool, regions: &[String]) -> Vec<ScrapeCandidate> {
    let platforms: Vec<String> = if all_systems {
        // Cargar las 190 plataformas sería demasiado: se miran las más habituales
        let mut list: Vec<String> = lb_platform(&system.name).map(|p| vec![p.to_string()]).unwrap_or_default();
        for p in ALL_PLATFORMS_HINT {
            if !list.iter().any(|l| l == p) {
                list.push(p.to_string());
            }
        }
        list
    } else {
        lb_platform(&system.name).map(|p| vec![p.to_string()]).unwrap_or_default()
    };
    let mut out = Vec::new();
    for name in platforms {
        let Some(platform) = load_platform(app, &name) else { continue };
        for g in platform.search(query, 30) {
            out.push(ScrapeCandidate {
                source: "launchbox".into(),
                id: format!("{}/{}", slug(&platform.name), g.id),
                name: g.name.clone(),
                system: Some(platform.name.clone()),
                year: g.date.as_deref().map(|d| d.chars().take(4).collect()),
                developer: g.dev.clone(),
                thumbnail: pick_image(g, "box-2D", regions).map(|i| format!("{}{}", IMAGE_BASE, i.file)),
                rom_match: false,
            });
        }
        if out.len() >= 30 {
            break;
        }
    }
    out.truncate(30);
    out
}

/// Plataformas que se miran con «Todos los sistemas» (además de la del sistema): las más habituales
const ALL_PLATFORMS_HINT: &[&str] = &[
    "Arcade", "Nintendo Entertainment System", "Super Nintendo Entertainment System", "Sega Genesis", "Sega Master System",
    "Nintendo Game Boy", "Nintendo Game Boy Color", "Nintendo Game Boy Advance", "Nintendo 64", "Sony Playstation",
    "Sony Playstation 2", "Sony PSP", "Sega Saturn", "Sega Dreamcast", "Sega CD", "Sega 32X", "Sega Game Gear",
    "NEC TurboGrafx-16", "NEC TurboGrafx-CD", "SNK Neo Geo AES", "SNK Neo Geo CD", "Nintendo DS", "Nintendo GameCube",
    "Nintendo Wii", "Atari 2600", "Atari 7800", "Atari Lynx", "Commodore 64", "Commodore Amiga", "MS-DOS",
];

/// Aplica el resultado elegido en la búsqueda manual (id «plataforma/número»)
pub async fn apply_launchbox_by_id(
    app: &tauri::AppHandle,
    client: &Client,
    id: &str,
    game: &Game,
    layout: &Layout,
    system: &SystemRef,
    settings: &AppSettings,
) -> Result<ScrapeResult, String> {
    let (platform_slug, number) = id.rsplit_once('/').ok_or("Invalid LaunchBox id")?;
    let number: u64 = number.parse().map_err(|_| "Invalid LaunchBox id")?;
    // El nombre real de la plataforma: el del sistema si coincide; si no, el de la lista
    let meta = read_meta(&data_dir(app)?).ok_or("LaunchBox database not downloaded")?;
    let name = meta.names.into_iter().find(|p| slug(p) == platform_slug).ok_or("LaunchBox platform not found")?;
    let platform = load_platform(app, &name).ok_or("LaunchBox database not downloaded")?;
    let lb = platform.games.iter().find(|g| g.id == number).cloned().ok_or("LaunchBox: game not found")?;
    Ok(ScrapeResult { game: apply_lb_game(client, &lb, game, layout, system, settings).await, found: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" standalone="yes"?>
<LaunchBox>
  <GameImage>
    <DatabaseID>10</DatabaseID>
    <FileName>early.png</FileName>
    <Type>Clear Logo</Type>
  </GameImage>
  <Game>
    <Name>Legend of Zelda: A Link to the Past, The</Name>
    <ReleaseDate>1992-04-13T00:00:00-07:00</ReleaseDate>
    <Overview>Link &amp; Zelda.</Overview>
    <MaxPlayers>1</MaxPlayers>
    <DatabaseID>10</DatabaseID>
    <CommunityRating>4.5</CommunityRating>
    <Platform>Super Nintendo Entertainment System</Platform>
    <Genres>Action; Adventure</Genres>
    <Developer>Nintendo</Developer>
    <Publisher>Nintendo</Publisher>
  </Game>
  <Game>
    <Name>Sin plataforma</Name>
    <DatabaseID>11</DatabaseID>
  </Game>
  <GameAlternateName>
    <AlternateName>Zelda no Densetsu: Kamigami no Triforce</AlternateName>
    <DatabaseID>10</DatabaseID>
    <Region>Japan</Region>
  </GameAlternateName>
  <GameImage>
    <DatabaseID>10</DatabaseID>
    <FileName>us.jpg</FileName>
    <Type>Box - Front</Type>
    <Region>North America</Region>
  </GameImage>
  <GameImage>
    <DatabaseID>10</DatabaseID>
    <FileName>eu.jpg</FileName>
    <Type>Box - Front</Type>
    <Region>Europe</Region>
  </GameImage>
  <GameImage>
    <DatabaseID>10</DatabaseID>
    <FileName>flyer.jpg</FileName>
    <Type>Advertisement Flyer - Front</Type>
  </GameImage>
</LaunchBox>"#;

    #[test]
    fn reads_games_alternate_names_and_useful_images() {
        let (platforms, count) = parse_metadata(XML.as_bytes()).unwrap();
        assert_eq!(count, 1, "a game without platform is skipped");
        let snes = &platforms["Super Nintendo Entertainment System"];
        let zelda = &snes[0];
        assert_eq!(zelda.desc.as_deref(), Some("Link & Zelda."));
        assert_eq!(zelda.date.as_deref(), Some("1992-04-13"));
        assert_eq!(zelda.genre.as_deref(), Some("Action"));
        assert_eq!(zelda.alt, vec!["Zelda no Densetsu: Kamigami no Triforce".to_string()]);
        // El flyer no se usa; el logo que venía antes que el juego, sí
        let kinds: Vec<&str> = zelda.images.iter().map(|i| i.media_type.as_str()).collect();
        assert_eq!(kinds, vec!["wheel", "box-2D", "box-2D"]);

        let regions = vec!["eu".to_string(), "us".to_string()];
        assert_eq!(pick_image(zelda, "box-2D", &regions).unwrap().file, "eu.jpg");
        assert_eq!(pick_image(zelda, "box-2D", &["us".to_string()]).unwrap().file, "us.jpg");
        assert_eq!(pick_image(zelda, "wheel", &regions).unwrap().file, "early.png");
    }

    #[test]
    fn finds_by_title_alternate_name_or_search() {
        let (mut platforms, _) = parse_metadata(XML.as_bytes()).unwrap();
        let p = LbPlatform::new("Super Nintendo Entertainment System", platforms.remove("Super Nintendo Entertainment System").unwrap());
        assert!(p.find("The Legend of Zelda - A Link to the Past (USA)").is_some());
        assert!(p.find("Zelda no Densetsu - Kamigami no Triforce (Japan)").is_some());
        assert!(p.find("Zelda II").is_none());
        assert_eq!(p.search("link to", 30).len(), 1);
        assert!(p.search("", 30).is_empty());
    }

    #[test]
    fn reads_the_mame_table() {
        let xml = r#"<LaunchBox><MameFile><FileName>sf2</FileName><Name>Street Fighter II: The World Warrior</Name><Year>1991</Year></MameFile></LaunchBox>"#;
        let map = parse_mame(xml.as_bytes()).unwrap();
        assert_eq!(map["sf2"], "Street Fighter II: The World Warrior");
    }
}
