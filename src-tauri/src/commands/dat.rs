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
use crate::models::Game;

use super::progress::Progress;

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
    /// CRC32 de sus ROMs (hex en minúsculas), para reconocerlas por contenido. Solo en juegos
    /// de hasta MAX_CRC_ROMS ROMs (cartuchos, discos): los de arcade llevan decenas y se
    /// reconocen por nombre (es su identidad), así el DAT de MAME no se dispara de tamaño.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub crcs: Vec<String>,
}

/// Juegos con más ROMs que esto no guardan sus CRC (ver `DatGame::crcs`)
const MAX_CRC_ROMS: usize = 4;

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
    /// Trae CRC (asociado con esta versión): se puede comprobar por contenido.
    /// Los DAT asociados antes no los guardaban: hay que volver a elegirlos.
    pub has_crcs: bool,
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
    /// Ficheros reconocidos por contenido (CRC en caché que está en el DAT)
    pub content_matched: usize,
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
            // <rom name="…" size="…" crc="…"/>: su CRC, para reconocer el fichero por contenido
            Ok(Event::Empty(e)) if e.name().as_ref() == b"rom" => {
                if let Some(game) = current.as_mut() {
                    let crc = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"crc")
                        .and_then(|a| a.unescape_value().ok().map(|v| v.trim().to_lowercase()));
                    if let Some(crc) = crc.filter(|c| c.len() == 8) {
                        game.crcs.push(crc);
                    }
                }
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

    // Arcade y similares: demasiadas ROMs por juego para que el CRC sirva (y pesaría mucho)
    for game in &mut dat.games {
        if game.crcs.len() > MAX_CRC_ROMS {
            game.crcs.clear();
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
        has_crcs: dat.games.iter().any(|g| !g.crcs.is_empty()),
    }
}

/// Asocia un DAT al sistema: lo lee y guarda la copia resumida.
#[tauri::command(async)]
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
#[tauri::command(async)]
pub fn clear_system_dat(app: tauri::AppHandle, system_path: String) -> Result<(), String> {
    let file = dat_file(&app, &SystemRef::from_rom_dir(&system_path).name)?;
    if file.exists() {
        fs::remove_file(&file).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Informe del DAT del sistema (None si no tiene).
#[tauri::command(async)]
pub fn dat_report(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<Option<DatReport>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    let Some(dat) = load_dat(&app, &system.name)? else { return Ok(None) };
    let games = super::gamelist::read_gamelist_with(&layout, &system, &progress)?.games;
    progress.check()?;
    progress.phase("scanning");
    let files = super::new_roms::rom_files(&system);
    progress.check()?;
    progress.phase("games");
    // Solo lo que ya está en caché: el informe nunca calcula (eso es «Comprobar por contenido»)
    let crcs = cached_crcs(&app, &files)?;
    Ok(Some(report(&system, &dat, &games, &files, &crcs)))
}

/// Huellas en caché de estos ficheros (sin calcular nada)
fn cached_crcs(app: &tauri::AppHandle, files: &[PathBuf]) -> Result<HashMap<PathBuf, String>, String> {
    let cache = crate::rom_hash::CrcCache::load(&crate::rom_hash::cache_file(app)?);
    Ok(files
        .iter()
        .filter_map(|f| cache.cached(f).flatten().map(|crc| (f.clone(), crc)))
        .collect())
}

#[derive(Debug, Clone, Serialize)]
struct CrcProgress {
    current: usize,
    total: usize,
    file: String,
}

/// «Comprobar por contenido»: calcula y guarda en caché las huellas de las ROMs del sistema
/// que falten (los comprimidos se leen de su índice; los sueltos, enteros la primera vez).
/// Emite `dat-crc-progress`. Devuelve cuántos ficheros reconoce el DAT por contenido.
#[tauri::command]
pub async fn dat_compute_crcs(app: tauri::AppHandle, system_path: String) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Emitter;
        let system = SystemRef::from_rom_dir(&system_path);
        let dat = load_dat(&app, &system.name)?.ok_or("This system has no DAT")?;
        let files = super::new_roms::rom_files(&system);
        let mut cache = crate::rom_hash::CrcCache::load(&crate::rom_hash::cache_file(&app)?);
        let mut crcs = HashMap::new();
        for (i, file) in files.iter().enumerate() {
            let _ = app.emit(
                "dat-crc-progress",
                CrcProgress {
                    current: i + 1,
                    total: files.len(),
                    file: file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                },
            );
            if let Some(crc) = cache.crc(file) {
                crcs.insert(file.clone(), crc);
            }
            // Cada 50 ficheros a disco: si se corta, lo calculado no se pierde
            if i % 50 == 49 {
                cache.save()?;
            }
        }
        cache.save()?;
        let by_crc = crc_index(&dat);
        Ok(crcs.values().filter(|crc| by_crc.contains_key(crc.as_str())).count())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Para Renombrar con el DAT: nombre del juego del DAT de cada ROM reconocida por contenido
/// (solo con lo que hay en caché), por ruta absoluta. Vacío si el sistema no tiene DAT.
pub fn names_by_content(app: &tauri::AppHandle, system: &SystemRef, rom_paths: &[PathBuf]) -> Result<HashMap<PathBuf, String>, String> {
    let Some(dat) = load_dat(app, &system.name)? else { return Ok(HashMap::new()) };
    let crcs = cached_crcs(app, rom_paths)?;
    let by_crc = crc_index(&dat);
    Ok(crcs
        .into_iter()
        .filter_map(|(path, crc)| {
            let key = match_key(&path.to_string_lossy());
            pick_by_crc(&by_crc, &crc, &key).map(|g| (path, g.name.clone()))
        })
        .collect())
}

/// Para «Mejor versión»: grupo de cada juego según el DAT (el original para los clones, el propio
/// nombre para los originales), en minúsculas. Por contenido si su huella está en caché; si no,
/// por nombre. Los juegos que el DAT no reconoce no salen. Vacío si el sistema no tiene DAT.
pub fn version_groups(app: &tauri::AppHandle, system: &SystemRef, games: &[Game]) -> Result<HashMap<String, String>, String> {
    let Some(dat) = load_dat(app, &system.name)? else { return Ok(HashMap::new()) };
    let roms: Vec<PathBuf> = games.iter().map(|g| system.rom_dir.join(g.path.strip_prefix("./").unwrap_or(&g.path))).collect();
    let crcs = cached_crcs(app, &roms)?;
    let by_name: HashMap<String, &DatGame> = dat.games.iter().map(|g| (g.name.to_lowercase(), g)).collect();
    let by_crc = crc_index(&dat);
    Ok(games
        .iter()
        .zip(roms.iter())
        .filter_map(|(game, rom)| {
            let key = match_key(&game.path);
            let entry = crcs.get(rom).and_then(|crc| pick_by_crc(&by_crc, crc, &key)).or_else(|| by_name.get(&key).copied())?;
            Some((game.path.clone(), entry.cloneof.clone().unwrap_or_else(|| entry.name.clone()).to_lowercase()))
        })
        .collect())
}

/// CRC → juegos del DAT que tienen una ROM con ese CRC
fn crc_index(dat: &Dat) -> HashMap<&str, Vec<&DatGame>> {
    let mut index: HashMap<&str, Vec<&DatGame>> = HashMap::new();
    for game in &dat.games {
        for crc in &game.crcs {
            index.entry(crc.as_str()).or_default().push(game);
        }
    }
    index
}

/// El juego de un CRC; si varios lo comparten (clones idénticos), el que coincide por nombre
fn pick_by_crc<'a>(index: &HashMap<&str, Vec<&'a DatGame>>, crc: &str, name_key: &str) -> Option<&'a DatGame> {
    let games = index.get(crc)?;
    games.iter().find(|g| g.name.to_lowercase() == name_key).or_else(|| games.first()).copied()
}

// ---------- Informe ----------

/// Nombre con el que un fichero o juego se empareja con el DAT: el del fichero sin extensión,
/// en minúsculas (arcade: «sf2.zip» → «sf2»; No-Intro: el nombre completo)
fn match_key(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Lo que dice el DAT de una ROM (pestaña Metadatos de la ficha a pantalla completa)
#[derive(Debug, Clone, Serialize)]
pub struct DatMatch {
    /// Nombre del DAT
    pub dat: String,
    /// "content" (por su CRC), "name" (por el nombre del fichero) o None si no está
    pub by: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub cloneof: Option<String>,
    pub bios: bool,
}

/// El juego del DAT de una ROM, con el mismo criterio que el informe: primero por contenido
/// (si su CRC está en caché), luego por nombre. None si el sistema no tiene DAT.
pub(crate) fn match_rom(app: &tauri::AppHandle, system: &SystemRef, rom: &Path) -> Result<Option<DatMatch>, String> {
    let Some(dat) = load_dat(app, &system.name)? else { return Ok(None) };
    let crcs = cached_crcs(app, &[rom.to_path_buf()])?;
    Ok(Some(match_in(&dat, rom, crcs.get(rom).map(String::as_str))))
}

fn match_in(dat: &Dat, rom: &Path, crc: Option<&str>) -> DatMatch {
    let key = match_key(&rom.to_string_lossy());
    let by_crc = crc_index(dat);
    let by_content = crc.and_then(|c| pick_by_crc(&by_crc, c, &key));
    let by_name = || dat.games.iter().find(|g| g.name.to_lowercase() == key);
    let (by, game) = match by_content {
        Some(g) => (Some("content"), Some(g)),
        None => match by_name() {
            Some(g) => (Some("name"), Some(g)),
            None => (None, None),
        },
    };
    DatMatch {
        dat: dat.name.clone(),
        by: by.map(String::from),
        name: game.map(|g| g.name.clone()),
        description: game.map(|g| g.description.clone()).filter(|d| !d.is_empty()),
        cloneof: game.and_then(|g| g.cloneof.clone()),
        bios: game.is_some_and(|g| g.bios),
    }
}

fn report(system: &SystemRef, dat: &Dat, games: &[Game], files: &[PathBuf], crcs: &HashMap<PathBuf, String>) -> DatReport {
    let by_name: HashMap<String, &DatGame> = dat.games.iter().map(|g| (g.name.to_lowercase(), g)).collect();
    let by_crc = crc_index(dat);
    // El juego del DAT de un fichero: primero por contenido (si su huella está en caché), luego por nombre
    let mut content_matched = 0;
    let mut entry_of_file: HashMap<&PathBuf, &DatGame> = HashMap::new();
    for file in files {
        let key = match_key(&file.to_string_lossy());
        let by_content = crcs.get(file).and_then(|crc| pick_by_crc(&by_crc, crc, &key));
        if by_content.is_some() {
            content_matched += 1;
        }
        if let Some(entry) = by_content.or_else(|| by_name.get(&key).copied()) {
            entry_of_file.insert(file, entry);
        }
    }
    let present: HashSet<String> = entry_of_file.values().map(|g| g.name.to_lowercase()).collect();
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
        .filter(|f| !entry_of_file.contains_key(f))
        .filter_map(|f| f.strip_prefix(&system.rom_dir).ok())
        .map(|rel| format!("./{}", rel.to_string_lossy().replace('\\', "/")))
        .collect();
    unknown_files.sort_by_key(|p| p.to_lowercase());

    let mut marks = Vec::new();
    let mut names = Vec::new();
    for game in games {
        let rom = system.rom_dir.join(game.path.strip_prefix("./").unwrap_or(&game.path));
        let Some(entry) = entry_of_file.get(&rom).copied().or_else(|| by_name.get(&match_key(&game.path)).copied()) else {
            continue;
        };
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

    DatReport { summary: summary(dat), missing, unknown_files, marks, names, content_matched }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_rom_por_contenido_o_por_nombre() {
        let dat = Dat {
            name: "No-Intro".into(),
            games: vec![
                DatGame { name: "Zelda (USA)".into(), crcs: vec!["abcd1234".into()], ..Default::default() },
                DatGame { name: "Zelda (Europe)".into(), cloneof: Some("Zelda (USA)".into()), ..Default::default() },
            ],
        };
        let m = match_in(&dat, Path::new("/r/snes/zelda.sfc"), Some("abcd1234"));
        assert_eq!(m.by.as_deref(), Some("content"));
        assert_eq!(m.name.as_deref(), Some("Zelda (USA)"));
        let m = match_in(&dat, Path::new("/r/snes/Zelda (Europe).zip"), None);
        assert_eq!(m.by.as_deref(), Some("name"));
        assert_eq!(m.cloneof.as_deref(), Some("Zelda (USA)"));
        let m = match_in(&dat, Path::new("/r/snes/otro.sfc"), None);
        assert!(m.by.is_none() && m.name.is_none());
    }

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

        let r = report(&system, &dat, &games, &files, &HashMap::new());
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

    const NOINTRO_CRC: &str = r#"<?xml version="1.0"?>
<datafile>
	<header><name>Nintendo - Super Nintendo Entertainment System</name></header>
	<game name="Super Mario World (USA)"><description>Super Mario World (USA)</description><rom name="Super Mario World (USA).sfc" size="524288" crc="B19ED489"/></game>
	<game name="Zelda (USA)"><description>Zelda (USA)</description><rom name="Zelda (USA).sfc" size="1048576" crc="777aac2f"/></game>
	<game name="Muchas ROMs"><rom name="a" crc="00000001"/><rom name="b" crc="00000002"/><rom name="c" crc="00000003"/><rom name="d" crc="00000004"/><rom name="e" crc="00000005"/></game>
</datafile>"#;

    #[test]
    fn keeps_the_crcs_of_small_games_only() {
        let dat = parse_dat(NOINTRO_CRC, "x").unwrap();
        assert_eq!(dat.games[0].crcs, vec!["b19ed489".to_string()]);
        assert!(dat.games[2].crcs.is_empty(), "games with many ROMs (arcade) are matched by name");
        assert!(summary(&dat).has_crcs);
        assert!(!summary(&parse_dat(MAME, "mame").unwrap()).has_crcs);
    }

    #[test]
    fn a_rom_with_another_name_is_recognised_by_its_crc() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        let dat = parse_dat(NOINTRO_CRC, "x").unwrap();
        let smw = system.rom_dir.join("smw.sfc");
        let files = vec![smw.clone(), system.rom_dir.join("otro.sfc")];
        let crcs: HashMap<PathBuf, String> = [(smw.clone(), "b19ed489".to_string())].into_iter().collect();
        let games = vec![game("./smw.sfc", Some("smw"))];

        let r = report(&system, &dat, &games, &files, &crcs);
        assert_eq!(r.content_matched, 1);
        // smw.sfc es Super Mario World: no falta ni está «fuera del DAT»; otro.sfc sí lo está
        let missing: Vec<_> = r.missing.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(missing, vec!["Zelda (USA)", "Muchas ROMs"]);
        assert_eq!(r.unknown_files, vec!["./otro.sfc"]);
        // Y se le propone el nombre del DAT (no estaba scrapeado)
        assert_eq!(r.names[0].proposed, "Super Mario World (USA)");

        // Sin huella en caché: por nombre, smw.sfc no está en el DAT
        let r = report(&system, &dat, &games, &files, &HashMap::new());
        assert_eq!(r.content_matched, 0);
        assert_eq!(r.unknown_files.len(), 2);
    }
}
