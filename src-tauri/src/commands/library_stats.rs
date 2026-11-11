//! Estadísticas de toda la biblioteca (pantalla de inicio): juegos, actividad, estados y
//! cobertura de medios de cada sistema, y el espacio en disco de ROMs y medios. El espacio
//! tarda (recorre todas las carpetas), así que va aparte y se guarda en `library_space.json`
//! (carpeta de datos de la app), por perfil.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use super::cleanup::MEDIA_DIRS;
use super::progress::Progress;
use super::{game_status, gamelist, settings};
use crate::layout::{strip_dot_slash, Layout, SystemRef};
use crate::media;
use crate::models::Game;

/// Cuántos juegos van en «los más jugados» y «jugados recientemente»
const TOP: usize = 10;
/// Carátulas del mosaico de la cabecera de la pantalla de inicio
const MOSAIC: usize = 24;

/// Lo de un sistema
#[derive(Debug, Clone, Default, Serialize)]
pub struct SystemStats {
    pub path: String,
    pub name: String,
    pub games: usize,
    pub favorites: usize,
    pub hidden: usize,
    /// Juegos cuya ROM ya no está
    pub missing: usize,
    /// Juegos con alguna partida
    pub played: usize,
    pub playcount: u64,
    /// Segundos jugados
    pub gametime: u64,
    /// Juegos por estado de partida (incomplete, finished…) y por marca (playing, backlog)
    pub statuses: BTreeMap<String, usize>,
    /// Juegos que tienen cada tipo de medio (por campo de la gamelist: image, video…)
    pub media: BTreeMap<String, usize>,
}

/// Un juego de las listas de la biblioteca (más jugados, recientes)
#[derive(Debug, Clone, Serialize)]
pub struct TopGame {
    pub system_path: String,
    pub system_name: String,
    pub game_path: String,
    pub name: String,
    pub playcount: u64,
    pub gametime: u64,
    pub lastplayed: Option<String>,
    /// Su carátula (campo `image`: relativa a la carpeta del sistema o absoluta)
    pub cover: Option<String>,
}

/// Una carátula del mosaico: de qué sistema es (para resolver las rutas relativas) y la ruta
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CoverRef {
    pub system_path: String,
    pub image: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct LibraryStats {
    pub systems: Vec<SystemStats>,
    pub most_played: Vec<TopGame>,
    pub recent: Vec<TopGame>,
    /// Carátulas para el mosaico: las de los más jugados y recientes y, si no llegan, al azar
    pub mosaic: Vec<CoverRef>,
}

/// Espacio de un sistema (bytes)
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SystemSpace {
    pub path: String,
    pub roms: u64,
    pub media: u64,
}

/// Lo medido para un perfil, con la fecha (segundos Unix)
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DiskUsage {
    pub measured_at: i64,
    pub systems: Vec<SystemSpace>,
}

/// Estadísticas de los sistemas dados (los del perfil activo con gamelist; los pasa el frontend)
#[tauri::command(async)]
pub fn library_stats(app: tauri::AppHandle, system_paths: Vec<String>, task: Option<String>) -> Result<LibraryStats, String> {
    let progress = Progress::new(&app, task);
    let layout = super::active_layout(&app)?;
    let mut stats = LibraryStats::default();
    let mut all: Vec<TopGame> = Vec::new();
    // Todas las carátulas de la biblioteca, para completar el mosaico al azar
    let mut covers: Vec<CoverRef> = Vec::new();
    for (i, path) in system_paths.iter().enumerate() {
        progress.check()?;
        let system = SystemRef::from_rom_dir(path);
        progress.step("reading", i + 1, system_paths.len(), &system.name);
        let games = gamelist::read_gamelist(&layout, &system)?.games;
        let statuses = game_status::statuses_of(&system.rom_dir, &games)?;
        stats.systems.push(system_stats(&system, &games, &statuses));
        all.extend(games.iter().filter(|g| number(&g.playcount) > 0 || g.lastplayed.is_some()).map(|g| top_game(&system, g)));
        covers.extend(games.iter().filter_map(|g| cover_of(g).map(|image| CoverRef { system_path: path.clone(), image })));
    }
    stats.most_played = top_by(&all, |a, b| b.playcount.cmp(&a.playcount).then(b.gametime.cmp(&a.gametime)), |g| g.playcount > 0);
    stats.recent = top_by(&all, |a, b| b.lastplayed.cmp(&a.lastplayed), |g| g.lastplayed.is_some());
    let seed = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(1) as u64;
    stats.mosaic = mosaic(&stats.most_played, &stats.recent, covers, seed);
    Ok(stats)
}

/// Un juego de la biblioteca para el sorteo de «¿A qué juego?» (solo lo que hace falta)
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LibraryGame {
    pub system_path: String,
    pub system_name: String,
    pub game_path: String,
    pub name: String,
    pub cover: Option<String>,
    /// Año (de la fecha de salida)
    pub year: Option<String>,
    pub genre: Option<String>,
    pub played: bool,
    pub favorite: bool,
}

/// Todos los juegos de los sistemas dados, sin los ocultos (para sortear en la pantalla de inicio)
#[tauri::command(async)]
pub fn library_games(app: tauri::AppHandle, system_paths: Vec<String>) -> Result<Vec<LibraryGame>, String> {
    let layout = super::active_layout(&app)?;
    let mut out = Vec::new();
    for path in &system_paths {
        let system = SystemRef::from_rom_dir(path);
        let games = gamelist::read_gamelist(&layout, &system)?.games;
        out.extend(games.iter().filter_map(|g| library_game(&system, g)));
    }
    Ok(out)
}

fn library_game(system: &SystemRef, game: &Game) -> Option<LibraryGame> {
    if game.hidden.as_deref() == Some("true") {
        return None;
    }
    let text = |v: &Option<String>| v.clone().filter(|s| !s.trim().is_empty());
    Some(LibraryGame {
        system_path: system.rom_dir.to_string_lossy().to_string(),
        system_name: system.name.clone(),
        game_path: game.path.clone(),
        name: text(&game.name).unwrap_or_else(|| game.path.clone()),
        cover: cover_of(game),
        year: game.releasedate.as_deref().filter(|d| d.len() >= 4).map(|d| d[..4].to_string()),
        genre: text(&game.genre),
        played: number(&game.playcount) > 0 || game.lastplayed.is_some(),
        favorite: game.favorite.as_deref() == Some("true"),
    })
}

/// Mide el espacio de ROMs y medios de los sistemas dados y lo guarda para el perfil activo
#[tauri::command(async)]
pub fn library_disk_usage(app: tauri::AppHandle, system_paths: Vec<String>, task: Option<String>) -> Result<DiskUsage, String> {
    let progress = Progress::new(&app, task);
    let layout = super::active_layout(&app)?;
    let mut usage = DiskUsage { measured_at: chrono::Utc::now().timestamp(), systems: Vec::new() };
    for (i, path) in system_paths.iter().enumerate() {
        progress.check()?;
        let system = SystemRef::from_rom_dir(path);
        progress.step("measuring", i + 1, system_paths.len(), &system.name);
        usage.systems.push(system_space(&layout, &system));
    }
    if let Some(id) = active_profile_id(&app) {
        let mut cache = read_cache(&app);
        cache.insert(id, usage.clone());
        write_cache(&app, &cache)?;
    }
    Ok(usage)
}

/// Lo último medido para el perfil activo (si se midió alguna vez)
#[tauri::command(async)]
pub fn library_disk_cached(app: tauri::AppHandle) -> Option<DiskUsage> {
    let id = active_profile_id(&app)?;
    read_cache(&app).remove(&id)
}

fn system_stats(system: &SystemRef, games: &[Game], statuses: &HashMap<String, game_status::GameStatus>) -> SystemStats {
    let mut stats = SystemStats {
        path: system.rom_dir.to_string_lossy().to_string(),
        name: system.name.clone(),
        games: games.len(),
        ..Default::default()
    };
    for game in games {
        stats.favorites += usize::from(game.favorite.as_deref() == Some("true"));
        stats.hidden += usize::from(game.hidden.as_deref() == Some("true"));
        stats.missing += usize::from(!rom_exists(system, &game.path));
        let plays = number(&game.playcount);
        stats.played += usize::from(plays > 0 || game.lastplayed.is_some());
        stats.playcount += plays;
        stats.gametime += number(&game.gametime);
        if let Some(status) = statuses.get(&game.path) {
            if let Some(s) = &status.status {
                *stats.statuses.entry(s.clone()).or_default() += 1;
            }
            if status.playing {
                *stats.statuses.entry("playing".into()).or_default() += 1;
            }
            if status.backlog {
                *stats.statuses.entry("backlog".into()).or_default() += 1;
            }
        }
        for field in media::media_fields() {
            if media::get_field(game, field).is_some_and(|v| !v.trim().is_empty()) {
                *stats.media.entry(field.to_string()).or_default() += 1;
            }
        }
    }
    stats
}

fn top_game(system: &SystemRef, game: &Game) -> TopGame {
    TopGame {
        system_path: system.rom_dir.to_string_lossy().to_string(),
        system_name: system.name.clone(),
        game_path: game.path.clone(),
        name: game.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| game.path.clone()),
        playcount: number(&game.playcount),
        gametime: number(&game.gametime),
        lastplayed: game.lastplayed.clone().filter(|l| !l.is_empty()),
        cover: cover_of(game),
    }
}

fn cover_of(game: &Game) -> Option<String> {
    game.image.clone().filter(|i| !i.trim().is_empty())
}

/// Carátulas del mosaico: primero las de los más jugados y recientes (sin repetir) y luego, si no
/// llegan a `MOSAIC`, otras de la biblioteca al azar (`seed` cambia cada vez)
fn mosaic(most: &[TopGame], recent: &[TopGame], mut covers: Vec<CoverRef>, seed: u64) -> Vec<CoverRef> {
    let mut out: Vec<CoverRef> = Vec::new();
    let mut push = |c: CoverRef, out: &mut Vec<CoverRef>| {
        if out.len() < MOSAIC && !out.contains(&c) {
            out.push(c);
        }
    };
    for g in most.iter().chain(recent) {
        if let Some(image) = &g.cover {
            push(CoverRef { system_path: g.system_path.clone(), image: image.clone() }, &mut out);
        }
    }
    shuffle(&mut covers, seed);
    for c in covers {
        push(c, &mut out);
    }
    out
}

/// Barajado sencillo (xorshift): para el mosaico basta, sin otra dependencia
fn shuffle<T>(items: &mut [T], seed: u64) {
    let mut state = seed | 1;
    for i in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        items.swap(i, (state % (i as u64 + 1)) as usize);
    }
}

/// Los primeros `TOP` que cumplen `keep`, en el orden de `cmp`
fn top_by(all: &[TopGame], cmp: impl Fn(&TopGame, &TopGame) -> std::cmp::Ordering, keep: impl Fn(&TopGame) -> bool) -> Vec<TopGame> {
    let mut list: Vec<TopGame> = all.iter().filter(|g| keep(g)).cloned().collect();
    list.sort_by(cmp);
    list.truncate(TOP);
    list
}

fn number(value: &Option<String>) -> u64 {
    value.as_deref().and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

fn rom_exists(system: &SystemRef, game_path: &str) -> bool {
    let path = Path::new(game_path);
    if path.is_absolute() {
        path.exists()
    } else {
        system.rom_dir.join(strip_dot_slash(game_path)).exists()
    }
}

/// Espacio de un sistema: ROMs = su carpeta, sin sus carpetas de medios ni las ocultas como
/// `.backups`; medios = sus carpetas de medios. En ES-DE, la de medios del sistema y, además,
/// las carpetas de medios de la estructura clásica que sigan en la de ROMs (restos sin migrar)
fn system_space(layout: &Layout, system: &SystemRef) -> SystemSpace {
    let mut media_dirs: Vec<PathBuf> = MEDIA_DIRS.iter().map(|d| system.rom_dir.join(d)).collect();
    if let Layout::Esde(esde) = layout {
        media_dirs.push(esde.media_root.join(&system.name));
    }
    let roms = dir_size(&system.rom_dir, &|p: &Path| media_dirs.iter().any(|m| m == p));
    let media = media_dirs.iter().map(|d| dir_size(d, &|_: &Path| false)).sum();
    SystemSpace { path: system.rom_dir.to_string_lossy().to_string(), roms, media }
}

/// Tamaño de una carpeta y lo que tiene dentro, sin las carpetas que diga `skip` ni las ocultas;
/// los enlaces simbólicos no se siguen
fn dir_size(dir: &Path, skip: &dyn Fn(&Path) -> bool) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    let mut total = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() {
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            if !hidden && !skip(&path) {
                total += dir_size(&path, skip);
            }
        } else if meta.is_file() {
            total += meta.len();
        }
    }
    total
}

fn active_profile_id(app: &tauri::AppHandle) -> Option<String> {
    settings::get_settings(app.clone()).ok()?.active_profile_id
}

fn cache_file(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("library_space.json"))
}

fn read_cache(app: &tauri::AppHandle) -> HashMap<String, DiskUsage> {
    cache_file(app)
        .and_then(|f| fs::read_to_string(f).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_cache(app: &tauri::AppHandle, cache: &HashMap<String, DiskUsage>) -> Result<(), String> {
    let file = cache_file(app).ok_or("No app data dir")?;
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(cache).map_err(|e| e.to_string())?;
    fs::write(file, text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::EsdePaths;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    fn game(path: &str) -> Game {
        Game { path: path.into(), name: Some(path.trim_start_matches("./").into()), ..Default::default() }
    }

    #[test]
    fn totales_de_un_sistema() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        write(&system.rom_dir.join("a.sfc"), 1);
        write(&system.rom_dir.join("b.sfc"), 1);
        let mut a = game("./a.sfc");
        a.favorite = Some("true".into());
        a.playcount = Some("3".into());
        a.gametime = Some("600".into());
        a.lastplayed = Some("20261111T100000".into());
        a.image = Some("./covers/a.png".into());
        a.video = Some("./videos/a.mp4".into());
        let mut b = game("./b.sfc");
        b.hidden = Some("true".into());
        b.image = Some(" ".into());
        let c = game("./c.sfc"); // sin ROM
        let statuses = HashMap::from([
            ("./a.sfc".to_string(), game_status::GameStatus { status: Some("finished".into()), playing: false, backlog: false }),
            ("./b.sfc".to_string(), game_status::GameStatus { status: None, playing: true, backlog: true }),
        ]);
        let stats = system_stats(&system, &[a, b, c], &statuses);
        assert_eq!((stats.games, stats.favorites, stats.hidden, stats.missing), (3, 1, 1, 1));
        assert_eq!((stats.played, stats.playcount, stats.gametime), (1, 3, 600));
        assert_eq!(stats.statuses.get("finished"), Some(&1));
        assert_eq!(stats.statuses.get("playing"), Some(&1));
        assert_eq!(stats.statuses.get("backlog"), Some(&1));
        // Un campo de medio en blanco no cuenta
        assert_eq!(stats.media.get("image"), Some(&1));
        assert_eq!(stats.media.get("video"), Some(&1));
        assert_eq!(stats.media.get("manual"), None);
    }

    #[test]
    fn listas_de_mas_jugados_y_recientes() {
        let system = SystemRef::from_rom_dir("/roms/snes");
        let mut games = Vec::new();
        for i in 0..12u64 {
            let mut g = game(&format!("./g{i}.sfc"));
            g.playcount = Some(i.to_string());
            g.lastplayed = Some(format!("202611{:02}T100000", i + 1));
            games.push(top_game(&system, &g));
        }
        let most = top_by(&games, |a, b| b.playcount.cmp(&a.playcount), |g| g.playcount > 0);
        assert_eq!(most.len(), TOP);
        assert_eq!(most[0].playcount, 11);
        let recent = top_by(&games, |a, b| b.lastplayed.cmp(&a.lastplayed), |g| g.lastplayed.is_some());
        assert_eq!(recent[0].lastplayed.as_deref(), Some("20261112T100000"));
    }

    #[test]
    fn juegos_para_sortear() {
        let system = SystemRef::from_rom_dir("/roms/snes");
        let mut hidden = game("./h.sfc");
        hidden.hidden = Some("true".into());
        assert_eq!(library_game(&system, &hidden), None);
        let mut g = game("./a.sfc");
        g.favorite = Some("true".into());
        g.releasedate = Some("19921121T000000".into());
        g.genre = Some("Platform".into());
        g.lastplayed = Some("20261111T100000".into());
        let entry = library_game(&system, &g).unwrap();
        assert_eq!((entry.year.as_deref(), entry.genre.as_deref()), (Some("1992"), Some("Platform")));
        assert!(entry.played && entry.favorite);
        let fresh = library_game(&system, &game("./b.sfc")).unwrap();
        assert!(!fresh.played && !fresh.favorite && fresh.year.is_none());
    }

    #[test]
    fn mosaico_primero_lo_jugado_y_sin_repetir() {
        let system = SystemRef::from_rom_dir("/roms/snes");
        let mut played = game("./p.sfc");
        played.playcount = Some("5".into());
        played.lastplayed = Some("20261111T100000".into());
        played.image = Some("./covers/p.png".into());
        let top = top_game(&system, &played);
        let library: Vec<CoverRef> = (0..40)
            .map(|i| CoverRef { system_path: "/roms/snes".into(), image: format!("./covers/{i}.png") })
            .chain([CoverRef { system_path: "/roms/snes".into(), image: "./covers/p.png".into() }])
            .collect();
        // Está en «más jugados» y en «recientes»: sale una vez, la primera
        let result = mosaic(&[top.clone()], &[top], library, 42);
        assert_eq!(result.len(), MOSAIC);
        assert_eq!(result[0].image, "./covers/p.png");
        assert_eq!(result.iter().filter(|c| c.image == "./covers/p.png").count(), 1);
        // Con pocas carátulas, las que haya
        let few = mosaic(&[], &[], vec![CoverRef { system_path: "/s".into(), image: "a.png".into() }], 7);
        assert_eq!(few.len(), 1);
    }

    #[test]
    fn espacio_en_la_clasica_sin_contar_medios_como_roms() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        write(&system.rom_dir.join("a.sfc"), 1000);
        write(&system.rom_dir.join("sub/b.sfc"), 500);
        write(&system.rom_dir.join("covers/a.png"), 200);
        write(&system.rom_dir.join("images/a-image.jpg"), 50);
        write(&system.rom_dir.join(".backups/gamelist.xml"), 70);
        let space = system_space(&Layout::Classic, &system);
        assert_eq!((space.roms, space.media), (1500, 250));
    }

    #[test]
    fn espacio_en_es_de() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/snes"));
        write(&system.rom_dir.join("a.sfc"), 1000);
        let media_root = tmp.path().join("media");
        write(&media_root.join("snes/covers/a.png"), 300);
        write(&media_root.join("snes/videos/a.mp4"), 700);
        write(&media_root.join("n64/covers/x.png"), 999); // otro sistema
        // Restos de la estructura clásica en la carpeta de ROMs: son medios, no ROMs
        write(&system.rom_dir.join("images/a-image.png"), 40);
        let layout = Layout::Esde(EsdePaths { home: tmp.path().join("ES-DE"), media_root });
        let space = system_space(&layout, &system);
        assert_eq!((space.roms, space.media), (1000, 1040));
    }
}
