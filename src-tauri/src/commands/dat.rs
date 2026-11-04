//! Ficheros DAT (MAME `-listxml`, Logiqx/clrmamepro XML de FBNeo y No-Intro, No-Intro
//! Parent-Clone): se asocian a un sistema y sirven para ver qué juegos faltan, qué ficheros
//! no conoce el DAT, qué juegos son clones o BIOS, y para poner nombres a juegos sin scrapear.
//!
//! El DAT original puede ser enorme (el de MAME ocupa cientos de MB): al asociarlo se guarda
//! una copia resumida en JSON en la carpeta de datos de la app y se lee esa.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::layout::SystemRef;
use crate::media;
use crate::models::Game;

/// Un juego del DAT, solo con lo que usa la app
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DatGame {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloneof: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bios: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub device: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub year: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub manufacturer: String,
}

/// El DAT resumido que se guarda por sistema
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Dat {
    /// Nombre del DAT (cabecera) o del fichero
    pub name: String,
    pub games: Vec<DatGame>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatSummary {
    pub name: String,
    pub games: usize,
    pub clones: usize,
    pub bios: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatMissing {
    pub name: String,
    pub description: String,
    pub cloneof: Option<String>,
    pub year: String,
    pub manufacturer: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatCloneMark {
    pub game_path: String,
    /// «clone» | «bios» | «device»
    pub kind: String,
    /// Original del clon (su descripción si el DAT la tiene)
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatNameProposal {
    pub game_path: String,
    pub current: Option<String>,
    pub proposed: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatReport {
    pub summary: DatSummary,
    pub missing: Vec<DatMissing>,
    /// Ficheros de ROM de la carpeta que el DAT no conoce (ruta «./sub/x.zip»)
    pub unknown_files: Vec<String>,
    pub marks: Vec<DatCloneMark>,
    pub names: Vec<DatNameProposal>,
}

// ---------- Lectura del XML ----------

/// Lee un DAT en cualquiera de los formatos XML habituales.
pub fn parse_dat(xml: &str, fallback_name: &str) -> Result<Dat, String> {
    let mut reader = Reader::from_str(xml);
    let mut dat = Dat { name: String::new(), games: Vec::new() };
    // No-Intro Parent-Clone: los clones apuntan al id del original, no a su nombre
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut clone_ids: Vec<(usize, String)> = Vec::new();

    let mut current: Option<DatGame> = None;
    let mut in_header = false;
    let mut text_target: Option<&'static str> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"header" => in_header = true,
                b"game" | b"machine" => {
                    let game = game_from_attrs(&e, &mut ids, dat.games.len(), &mut clone_ids);
                    current = Some(game);
                }
                b"description" => text_target = Some("description"),
                b"year" => text_target = Some("year"),
                b"manufacturer" => text_target = Some("manufacturer"),
                b"name" if in_header => text_target = Some("datname"),
                _ => {}
            },
            Ok(Event::Empty(e)) if matches!(e.name().as_ref(), b"game" | b"machine") => {
                let game = game_from_attrs(&e, &mut ids, dat.games.len(), &mut clone_ids);
                dat.games.push(game);
            }
            Ok(Event::Text(t)) => {
                if let Some(target) = text_target {
                    let text = t.unescape().map(|c| c.trim().to_string()).unwrap_or_default();
                    match (target, current.as_mut()) {
                        ("datname", _) if dat.name.is_empty() => dat.name = text,
                        ("description", Some(g)) => g.description = text,
                        ("year", Some(g)) => g.year = text,
                        ("manufacturer", Some(g)) => g.manufacturer = text,
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"header" => in_header = false,
                b"game" | b"machine" => {
                    if let Some(g) = current.take() {
                        dat.games.push(g);
                    }
                }
                _ => text_target = None,
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Not a valid DAT (XML): {}", e)),
            _ => {}
        }
    }

    for (index, id) in clone_ids {
        if let Some(parent) = ids.get(&id) {
            dat.games[index].cloneof = Some(parent.clone());
        }
    }
    if dat.games.is_empty() {
        return Err("The DAT has no games (supported: MAME -listxml, Logiqx/clrmamepro XML, No-Intro)".into());
    }
    if dat.name.is_empty() {
        dat.name = fallback_name.to_string();
    }
    Ok(dat)
}

fn game_from_attrs(e: &BytesStart, ids: &mut HashMap<String, String>, index: usize, clone_ids: &mut Vec<(usize, String)>) -> DatGame {
    let mut game = DatGame::default();
    let mut id = None;
    for attr in e.attributes().flatten() {
        let value = attr.unescape_value().map(|v| v.to_string()).unwrap_or_default();
        match attr.key.as_ref() {
            b"name" => game.name = value,
            b"cloneof" if !value.is_empty() => game.cloneof = Some(value),
            b"isbios" => game.bios = value == "yes",
            b"isdevice" => game.device = value == "yes",
            b"id" => id = Some(value),
            b"cloneofid" if !value.is_empty() => clone_ids.push((index, value)),
            _ => {}
        }
    }
    if let Some(id) = id {
        ids.insert(id, game.name.clone());
    }
    game
}

// ---------- Guardar por sistema ----------

fn dats_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?.join("dats");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    Ok(dir)
}

/// Fichero de la copia resumida de un sistema (por el nombre de su carpeta)
fn dat_file(app: &tauri::AppHandle, system_name: &str) -> Result<PathBuf, String> {
    let key: String = system_name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    Ok(dats_dir(app)?.join(format!("{}.json", key)))
}

fn load_dat(app: &tauri::AppHandle, system_name: &str) -> Result<Option<Dat>, String> {
    let file = dat_file(app, system_name)?;
    if !file.is_file() {
        return Ok(None);
    }
    let json = fs::read_to_string(&file).map_err(|e| format!("Failed to read {}: {}", file.display(), e))?;
    serde_json::from_str(&json).map(Some).map_err(|e| format!("Failed to read the saved DAT: {}", e))
}

fn summary(dat: &Dat) -> DatSummary {
    DatSummary {
        name: dat.name.clone(),
        games: dat.games.iter().filter(|g| !g.bios && !g.device).count(),
        clones: dat.games.iter().filter(|g| g.cloneof.is_some()).count(),
        bios: dat.games.iter().filter(|g| g.bios).count(),
    }
}

/// Asocia un DAT al sistema: lo lee y guarda la copia resumida.
#[tauri::command]
pub fn set_system_dat(app: tauri::AppHandle, system_path: String, dat_path: String) -> Result<DatSummary, String> {
    let system = SystemRef::from_rom_dir(&system_path);
    let xml = fs::read_to_string(&dat_path).map_err(|e| format!("Failed to read {}: {}", dat_path, e))?;
    let fallback = Path::new(&dat_path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let dat = parse_dat(&xml, &fallback)?;
    let json = serde_json::to_string(&dat).map_err(|e| e.to_string())?;
    fs::write(dat_file(&app, &system.name)?, json).map_err(|e| format!("Failed to save the DAT: {}", e))?;
    Ok(summary(&dat))
}

/// Quita el DAT del sistema.
#[tauri::command]
pub fn clear_system_dat(app: tauri::AppHandle, system_path: String) -> Result<(), String> {
    let file = dat_file(&app, &SystemRef::from_rom_dir(&system_path).name)?;
    if file.exists() {
        fs::remove_file(&file).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Informe del DAT del sistema (None si no tiene).
#[tauri::command]
pub fn dat_report(app: tauri::AppHandle, system_path: String) -> Result<Option<DatReport>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let Some(dat) = load_dat(&app, &system.name)? else { return Ok(None) };
    let games = super::gamelist::read_gamelist(&layout, &system)?.games;
    let files = super::new_roms::rom_files(&system);
    Ok(Some(report(&system, &dat, &games, &files)))
}

// ---------- Informe ----------

/// Nombre con el que un fichero o juego se empareja con el DAT: el del fichero sin extensión,
/// en minúsculas (arcade: «sf2.zip» → «sf2»; No-Intro: el nombre completo)
fn match_key(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn report(system: &SystemRef, dat: &Dat, games: &[Game], files: &[PathBuf]) -> DatReport {
    let by_name: HashMap<String, &DatGame> = dat.games.iter().map(|g| (g.name.to_lowercase(), g)).collect();
    let present: HashSet<String> = files.iter().map(|f| match_key(&f.to_string_lossy())).collect();
    let description_of = |name: &str| {
        by_name.get(&name.to_lowercase()).map(|g| if g.description.is_empty() { g.name.clone() } else { g.description.clone() })
    };

    let missing = dat
        .games
        .iter()
        .filter(|g| !g.bios && !g.device && !present.contains(&g.name.to_lowercase()))
        .map(|g| DatMissing {
            name: g.name.clone(),
            description: g.description.clone(),
            cloneof: g.cloneof.clone(),
            year: g.year.clone(),
            manufacturer: g.manufacturer.clone(),
        })
        .collect();

    let mut unknown_files: Vec<String> = files
        .iter()
        .filter(|f| !by_name.contains_key(&match_key(&f.to_string_lossy())))
        .filter_map(|f| f.strip_prefix(&system.rom_dir).ok())
        .map(|rel| format!("./{}", rel.to_string_lossy().replace('\\', "/")))
        .collect();
    unknown_files.sort_by_key(|p| p.to_lowercase());

    let mut marks = Vec::new();
    let mut names = Vec::new();
    for game in games {
        let Some(entry) = by_name.get(&match_key(&game.path)) else { continue };
        let kind = if entry.bios {
            Some("bios")
        } else if entry.device {
            Some("device")
        } else if entry.cloneof.is_some() {
            Some("clone")
        } else {
            None
        };
        if let Some(kind) = kind {
            marks.push(DatCloneMark {
                game_path: game.path.clone(),
                kind: kind.to_string(),
                parent: entry.cloneof.as_deref().map(|p| description_of(p).unwrap_or_else(|| p.to_string())),
            });
        }
        // Nombre propuesto: solo si no tiene o si su nombre es el del fichero (sin scrapear)
        let current = game.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
        let unnamed = current.is_none_or(|n| n.to_lowercase() == match_key(&game.path));
        if unnamed && !entry.description.is_empty() && current != Some(entry.description.as_str()) {
            names.push(DatNameProposal {
                game_path: game.path.clone(),
                current: current.map(String::from),
                proposed: entry.description.clone(),
            });
        }
    }

    DatReport { summary: summary(dat), missing, unknown_files, marks, names }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAME: &str = r#"<?xml version="1.0"?>
<mame build="0.270">
	<machine name="neogeo" isbios="yes"><description>Neo-Geo MV-6F</description></machine>
	<machine name="sf2" romof="sf2"><description>Street Fighter II: The World Warrior (World 910522)</description><year>1991</year><manufacturer>Capcom</manufacturer></machine>
	<machine name="sf2ua" cloneof="sf2" romof="sf2"><description>Street Fighter II: The World Warrior (USA 910206)</description><year>1991</year><manufacturer>Capcom</manufacturer></machine>
	<machine name="z80" isdevice="yes"><description>Zilog Z80</description></machine>
	<machine name="ffight"><description>Final Fight (World, set 1)</description><year>1989</year><manufacturer>Capcom</manufacturer></machine>
</mame>"#;

    const LOGIQX: &str = r#"<?xml version="1.0"?>
<datafile>
	<header><name>FinalBurn Neo - Arcade Games</name></header>
	<game name="3wonders"><description>Three Wonders (World 910520)</description></game>
	<game name="3wondersu" cloneof="3wonders" romof="3wonders"><description>Three Wonders (USA 910520)</description></game>
</datafile>"#;

    const NOINTRO_PC: &str = r#"<?xml version="1.0"?>
<datafile>
	<header><name>Sega - Master System - Mark III (Parent-Clone)</name></header>
	<game name="Alex Kidd in Miracle World (USA, Europe)" id="0010"><description>Alex Kidd in Miracle World (USA, Europe)</description></game>
	<game name="Alex Kidd in Miracle World (Brazil)" id="0011" cloneofid="0010"><description>Alex Kidd in Miracle World (Brazil)</description></game>
</datafile>"#;

    #[test]
    fn reads_the_three_formats() {
        let mame = parse_dat(MAME, "mame").unwrap();
        assert_eq!(mame.name, "mame");
        assert_eq!(mame.games.len(), 5);
        assert!(mame.games[0].bios);
        assert_eq!(mame.games[2].cloneof.as_deref(), Some("sf2"));
        assert_eq!(mame.games[1].year, "1991");
        assert!(mame.games[3].device);

        let fbneo = parse_dat(LOGIQX, "x").unwrap();
        assert_eq!(fbneo.name, "FinalBurn Neo - Arcade Games");
        assert_eq!(fbneo.games[1].cloneof.as_deref(), Some("3wonders"));

        let ni = parse_dat(NOINTRO_PC, "x").unwrap();
        assert_eq!(ni.games[1].cloneof.as_deref(), Some("Alex Kidd in Miracle World (USA, Europe)"));

        assert!(parse_dat("<datafile></datafile>", "x").is_err());
    }

    fn game(path: &str, name: Option<&str>) -> Game {
        Game { path: path.into(), name: name.map(String::from), ..Default::default() }
    }

    #[test]
    fn report_missing_unknown_clones_and_names() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("arcade"));
        let dat = parse_dat(MAME, "mame").unwrap();
        let files: Vec<PathBuf> = ["sf2ua.zip", "neogeo.zip", "pacman.zip"].iter().map(|f| system.rom_dir.join(f)).collect();
        let games = vec![
            game("./sf2ua.zip", Some("sf2ua")),          // clon, sin scrapear
            game("./neogeo.zip", Some("Neo Geo BIOS")), // BIOS, ya con nombre
        ];

        let r = report(&system, &dat, &games, &files);
        // Faltan: sf2 y ffight (ni la BIOS ni el dispositivo)
        let missing: Vec<_> = r.missing.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(missing, vec!["sf2", "ffight"]);
        assert_eq!(r.unknown_files, vec!["./pacman.zip"]);
        let clone = r.marks.iter().find(|m| m.game_path == "./sf2ua.zip").unwrap();
        assert_eq!(clone.kind, "clone");
        assert_eq!(clone.parent.as_deref(), Some("Street Fighter II: The World Warrior (World 910522)"));
        assert!(r.marks.iter().any(|m| m.game_path == "./neogeo.zip" && m.kind == "bios"));
        // Solo se propone nombre al que no estaba scrapeado
        assert_eq!(r.names.len(), 1);
        assert_eq!(r.names[0].game_path, "./sf2ua.zip");
        assert_eq!(r.names[0].proposed, "Street Fighter II: The World Warrior (USA 910206)");
        assert_eq!(r.summary.games, 3);
        assert_eq!(r.summary.bios, 1);
    }
}
