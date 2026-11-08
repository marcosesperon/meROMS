//! Comprobar las BIOS: qué ficheros de sistema necesita cada sistema y si están en la carpeta
//! de BIOS del perfil (con el MD5 correcto).
//!
//! La lista es el `System.dat` de libretro (RetroArch, licencia CC-BY-SA): no va dentro de la
//! app, se descarga desde Ajustes y se guarda leída en la carpeta de datos (`bios_list.json`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::models::{Profile, StructureKind};

const SYSTEM_DAT_URL: &str = "https://raw.githubusercontent.com/libretro/libretro-database/master/dat/System.dat";

/// Una BIOS de la lista
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiosFile {
    /// Ruta dentro de la carpeta de BIOS (puede llevar subcarpeta: «dc/dc_boot.bin»)
    pub name: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub md5: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BiosList {
    downloaded_at: i64,
    /// Sección de libretro («Sony - PlayStation») → sus BIOS
    sections: HashMap<String, Vec<BiosFile>>,
}

/// Sección de libretro de cada sistema (carpeta de ES-DE). Los que no salen, no tienen BIOS en la lista.
pub fn libretro_section(folder: &str) -> Option<&'static str> {
    Some(match folder.to_lowercase().as_str() {
        "3do" => "3DO Company, The - 3DO",
        "amstradcpc" | "gx4000" => "Amstrad - CPC",
        "atari800" | "atarixe" => "Atari - 400-800",
        "atari5200" => "Atari - 5200",
        "atari7800" => "Atari - 7800",
        "atarilynx" => "Atari - Lynx",
        "atarist" => "Atari - ST",
        "colecovision" => "Coleco - ColecoVision",
        "amiga" | "amiga600" | "amiga1200" | "amigacd32" | "cdtv" => "Commodore - Amiga",
        "dos" | "pc" => "DOS",
        "scv" => "EPOCH/YENO Super Cassette Vision",
        "channelf" => "Fairchild Channel F",
        "macintosh" => "MacII",
        "odyssey2" => "Magnavox - Odyssey2",
        "videopac" => "Phillips - Videopac+",
        "intellivision" => "Mattel - Intellivision",
        "msx" | "msx1" | "msx2" | "msxturbor" => "Microsoft - MSX",
        "pcengine" | "pcenginecd" | "tg16" | "tg-cd" | "supergrafx" => "NEC - PC Engine - TurboGrafx 16 - SuperGrafx",
        "pc98" => "NEC - PC-98",
        "pc88" => "NEC - PC-8801",
        "pcfx" => "NEC - PC-FX",
        "fds" => "Nintendo - Famicom Disk System",
        "gb" => "Nintendo - Gameboy",
        "gba" => "Nintendo - Game Boy Advance",
        "gbc" => "Nintendo - Gameboy Color",
        "gc" => "Nintendo - GameCube",
        "n64dd" => "Nintendo - Nintendo 64DD",
        "nds" => "Nintendo - Nintendo DS",
        "pokemini" => "Nintendo - Pokemon Mini",
        "satellaview" => "Nintendo - Satellaview",
        "sufami" => "Nintendo - SuFami Turbo",
        "sgb" => "Nintendo - Super Game Boy",
        "cdimono1" => "Philips - CD-i",
        "palm" => "Palm OS",
        "dreamcast" => "Sega - Dreamcast",
        "naomi" | "naomi2" | "naomigd" | "atomiswave" => "Sega - Dreamcast-based Arcade",
        "gamegear" => "Sega - Game Gear",
        "mastersystem" | "mark3" => "Sega - Master System - Mark III",
        "megacd" | "megacdjp" | "segacd" => "Sega - Mega CD - Sega CD",
        "saturn" | "saturnjp" => "Sega - Saturn",
        "x1" => "Sharp - X1",
        "x68000" => "Sharp - X68000",
        "zxspectrum" => "Sinclair - ZX Spectrum",
        "neogeocd" | "neogeocdjp" => "SNK - NeoGeo CD",
        "psx" => "Sony - PlayStation",
        "ps2" => "Sony - PlayStation 2",
        "psp" => "Sony - PlayStation Portable",
        "scummvm" => "ScummVM",
        "vircon32" => "Vircon32",
        _ => return None,
    })
}

/// Separa una línea en palabras, respetando las que van entre comillas
fn tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            out.push(chars.by_ref().take_while(|&d| d != '"').collect());
        } else {
            let mut word = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_whitespace() {
                    break;
                }
                word.push(d);
                chars.next();
            }
            out.push(word);
        }
    }
    out
}

/// Lee el System.dat (clrmamepro): cada «comment "…"» abre una sección y cada «rom ( … )» es una BIOS
pub fn parse_system_dat(text: &str) -> HashMap<String, Vec<BiosFile>> {
    let mut sections: HashMap<String, Vec<BiosFile>> = HashMap::new();
    let mut current: Option<String> = None;
    let mut in_game = false;
    for line in text.lines() {
        let t = tokens(line);
        match t.first().map(String::as_str) {
            Some("game") => in_game = true,
            Some("comment") if in_game => current = t.get(1).cloned(),
            Some("rom") => {
                let Some(section) = &current else { continue };
                let field = |k: &str| t.iter().position(|w| w == k).and_then(|i| t.get(i + 1)).cloned();
                let Some(name) = field("name") else { continue };
                sections.entry(section.clone()).or_default().push(BiosFile {
                    name,
                    size: field("size").and_then(|s| s.parse().ok()),
                    md5: field("md5").map(|m| m.to_lowercase()),
                });
            }
            _ => {}
        }
    }
    sections
}

fn list_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("bios_list.json"))
}

fn load_list(app: &tauri::AppHandle) -> Option<BiosList> {
    serde_json::from_slice(&fs::read(list_file(app).ok()?).ok()?).ok()
}

#[derive(Debug, Clone, Serialize)]
pub struct BiosListStatus {
    pub downloaded: bool,
    pub downloaded_at: Option<i64>,
    pub files: usize,
}

#[tauri::command(async)]
pub fn bios_status(app: tauri::AppHandle) -> BiosListStatus {
    let list = load_list(&app);
    BiosListStatus {
        downloaded: list.is_some(),
        downloaded_at: list.as_ref().map(|l| l.downloaded_at),
        files: list.as_ref().map_or(0, |l| l.sections.values().map(Vec::len).sum()),
    }
}

/// Descarga el System.dat de libretro y lo guarda leído
#[tauri::command]
pub async fn bios_download(app: tauri::AppHandle) -> Result<BiosListStatus, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.get(SYSTEM_DAT_URL).send().await.map_err(|e| format!("Download failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }
    let text = response.text().await.map_err(|e| e.to_string())?;
    let sections = parse_system_dat(&text);
    if sections.is_empty() {
        return Err("The BIOS list could not be read".into());
    }
    let list = BiosList { downloaded_at: chrono::Utc::now().timestamp(), sections };
    fs::write(list_file(&app)?, serde_json::to_vec(&list).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(bios_status(app))
}

#[tauri::command(async)]
pub fn bios_delete(app: tauri::AppHandle) -> Result<(), String> {
    let file = list_file(&app)?;
    if file.exists() {
        fs::remove_file(file).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Carpeta de BIOS del perfil: la elegida o, si no, la habitual junto a la carpeta de ROMs
/// (clásica: <ROMs>/../bios; ES-DE: <ES-DE>/../bios o <ROMs>/../bios, como EmuDeck)
pub fn bios_dir(profile: &Profile) -> Option<PathBuf> {
    if let Some(dir) = profile.bios_dir.as_deref().filter(|d| !d.trim().is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let next_to = |p: &str| Path::new(p).parent().map(|d| d.join("bios"));
    let mut candidates: Vec<PathBuf> = Vec::new();
    if profile.kind == StructureKind::Esde {
        candidates.extend(profile.esde_home_path.as_deref().and_then(next_to));
    }
    candidates.extend(next_to(&profile.rom_root_path));
    candidates.iter().find(|d| d.is_dir()).cloned().or_else(|| candidates.into_iter().next())
}

/// Carpeta que se propone si el perfil no tiene una (para enseñarla en Ajustes)
#[tauri::command(async)]
pub fn suggested_bios_dir(profile: Profile) -> Option<String> {
    let mut p = profile;
    p.bios_dir = None;
    bios_dir(&p).map(|d| d.to_string_lossy().to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct BiosCheck {
    pub name: String,
    /// ok | missing | mismatch
    pub status: String,
    pub expected_md5: Option<String>,
    /// Dónde se encontró (si se encontró)
    pub found: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemBios {
    pub system: String,
    pub section: String,
    pub ok: usize,
    pub total: usize,
    pub files: Vec<BiosCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BiosReport {
    pub bios_dir: Option<String>,
    pub systems: Vec<SystemBios>,
}

fn md5_of(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(format!("{:x}", Md5::digest(&bytes)))
}

/// Comprueba las BIOS de unos sistemas (carpetas de ES-DE) en una carpeta
pub fn check(dir: &Path, sections: &HashMap<String, Vec<BiosFile>>, systems: &[String]) -> Vec<SystemBios> {
    // Todos los ficheros de la carpeta de BIOS, por nombre en minúsculas (una sola pasada)
    let mut by_name: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for file in crate::media::list_files_recursive(dir) {
        if let Some(n) = file.file_name() {
            by_name.entry(n.to_string_lossy().to_lowercase()).or_default().push(file);
        }
    }
    let mut out = Vec::new();
    let mut seen_sections: Vec<&str> = Vec::new();
    for system in systems {
        let Some(section) = libretro_section(system) else { continue };
        let Some(files) = sections.get(section) else { continue };
        // Varias carpetas con la misma sección (megacd y segacd): se ve una vez
        if seen_sections.contains(&section) {
            continue;
        }
        seen_sections.push(section);
        let checks: Vec<BiosCheck> = files
            .iter()
            .map(|f| {
                let exact = dir.join(&f.name);
                let base = Path::new(&f.name).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                let mut candidates: Vec<PathBuf> = Vec::new();
                if exact.is_file() {
                    candidates.push(exact);
                }
                candidates.extend(by_name.get(&base).into_iter().flatten().cloned());
                // La que coincide (tamaño y luego MD5); si ninguna, la primera encontrada
                let matching = candidates.iter().find(|c| {
                    let size_ok = f.size.is_none_or(|s| fs::metadata(c).map(|m| m.len()).ok() == Some(s));
                    size_ok && f.md5.as_ref().is_none_or(|m| md5_of(c).as_deref() == Some(m.as_str()))
                });
                let (status, found) = match (matching, candidates.first()) {
                    (Some(c), _) => ("ok", Some(c)),
                    (None, Some(c)) => ("mismatch", Some(c)),
                    (None, None) => ("missing", None),
                };
                BiosCheck {
                    name: f.name.clone(),
                    status: status.into(),
                    expected_md5: f.md5.clone(),
                    found: found.map(|p| p.to_string_lossy().to_string()),
                }
            })
            .collect();
        out.push(SystemBios {
            system: system.clone(),
            section: section.to_string(),
            ok: checks.iter().filter(|c| c.status == "ok").count(),
            total: checks.len(),
            files: checks,
        });
    }
    out
}

/// Comprueba las BIOS de los sistemas indicados (o de todos los del perfil, si no se indican)
#[tauri::command]
pub async fn check_bios(app: tauri::AppHandle, systems: Vec<String>) -> Result<Option<BiosReport>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(list) = load_list(&app) else { return Ok(None) };
        let settings = super::settings::get_settings(app.clone())?;
        let profile = settings.active_profile().cloned().ok_or("No active profile")?;
        let dir = bios_dir(&profile);
        let systems = match dir.as_deref() {
            Some(d) => check(d, &list.sections, &systems),
            None => Vec::new(),
        };
        Ok(Some(BiosReport { bios_dir: dir.map(|d| d.to_string_lossy().to_string()), systems }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAT: &str = r#"clrmamepro (
	name "System"
	comment "System, firmware, and BIOS files used by libretro cores."
)

game (
	name "System"
	comment "Sony - PlayStation"
	rom ( name scph1001.bin size 4 crc 00000000 md5 81dc9bdb52d04dc20036dbd8313ed055 sha1 x )
	rom ( name scph5501.bin size 4 crc 00000000 md5 ffffffffffffffffffffffffffffffff sha1 x )
	comment "Sega - Dreamcast"
	rom ( name "dc/dc_boot.bin" size 4 md5 81DC9BDB52D04DC20036DBD8313ED055 )
	rom ( name "dc/dc nvmem.bin" size 4 md5 81dc9bdb52d04dc20036dbd8313ed055 )
)"#;

    #[test]
    fn reads_sections_names_with_folders_and_spaces() {
        let s = parse_system_dat(DAT);
        assert_eq!(s["Sony - PlayStation"].len(), 2);
        let dc = &s["Sega - Dreamcast"];
        assert_eq!(dc[0].name, "dc/dc_boot.bin");
        assert_eq!(dc[0].md5.as_deref(), Some("81dc9bdb52d04dc20036dbd8313ed055"), "lowercased");
        assert_eq!(dc[1].name, "dc/dc nvmem.bin");
        assert!(!s.contains_key("System, firmware, and BIOS files used by libretro cores."));
    }

    #[test]
    fn ok_missing_mismatch_and_found_in_a_subfolder() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        // «1234» → md5 81dc9bdb52d04dc20036dbd8313ed055
        fs::write(d.join("scph1001.bin"), "1234").unwrap();
        fs::create_dir_all(d.join("otra")).unwrap();
        fs::write(d.join("otra/dc_boot.bin"), "1234").unwrap(); // donde no toca, pero se encuentra
        fs::create_dir_all(d.join("dc")).unwrap();
        fs::write(d.join("dc/dc nvmem.bin"), "abcd").unwrap(); // MD5 distinto

        let sections = parse_system_dat(DAT);
        let report = check(d, &sections, &["psx".into(), "dreamcast".into(), "snes".into()]);
        assert_eq!(report.len(), 2, "snes has no BIOS in the list");
        let psx = &report[0];
        assert_eq!((psx.ok, psx.total), (1, 2));
        assert_eq!(psx.files[1].status, "missing");
        let dc = &report[1];
        assert_eq!(dc.files[0].status, "ok");
        assert_eq!(dc.files[1].status, "mismatch");
    }

    #[test]
    fn suggested_folder_is_next_to_the_roms() {
        let p = Profile {
            id: "x".into(),
            name: "x".into(),
            kind: StructureKind::Classic,
            rom_root_path: "/ficticio/Emulation/roms".into(),
            esde_home_path: None,
            media_dir: None,
            skip_structure_prompt: false,
            last_system: None,
            bios_dir: None,
            saves_dir: None,
            system_views: None,
        };
        assert_eq!(bios_dir(&p), Some(PathBuf::from("/ficticio/Emulation/bios")));
        let mine = Profile { bios_dir: Some("/mis/bios".into()), ..p };
        assert_eq!(bios_dir(&mine), Some(PathBuf::from("/mis/bios")));
    }
}

