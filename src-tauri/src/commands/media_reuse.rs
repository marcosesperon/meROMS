//! Reaprovechar medios: sobrantes que en realidad son de un juego, pero con otro nombre (el
//! nombre corto de MAME que tenía la ROM antes de renombrarla, el título en vez del nombre de la
//! ROM…), y referencias rotas cuyo fichero sigue por ahí con otra extensión o en otra carpeta.
//! En vez de tirarlos y volver a descargarlos del scraper, se proponen para incorporarlos con el
//! nombre unificado del juego.
//!
//! De cada sobrante se saca el tipo (carpeta y sufijo antiguo, como Normalizar) y el juego, por
//! su nombre sin sufijo, si ese nombre lleva a **un solo** juego. Nombres que valen para un juego:
//! - `name`: su título o el nombre de su ROM (también con el artículo movido: «King of Dragons,
//!   The» = «The King of Dragons»)
//! - `old_name`: el nombre que tenía su ROM según las copias de la gamelist (`.backups`)
//! - `dat`: su nombre en el DAT del sistema (en arcade, el nombre corto de MAME)
//! - `mame`: el título que da LaunchBox para ese nombre corto de MAME
//!
//! Si el juego ya tiene un medio de ese tipo, se propone quedarse con el de más resolución.
//! Lo que no se queda lo quita el frontend (papelera o apartados) antes de incorporar.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::{Layout, SystemRef};
use crate::media::{self, MediaIndex, MediaKind};
use crate::models::Game;

use super::cleanup::BrokenMediaRef;
use super::normalize::{relative_to, title_key, NormalizeItem, NormalizeResult, NormalizeStatus, TITLE_SUFFIXES};
use super::progress::Progress;

/// Lo que el juego ya tiene de ese tipo
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentMedia {
    pub path: String,
    pub relative: String,
    pub size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Un fichero que se puede incorporar a un juego
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReusableMedia {
    pub path: String,
    pub relative: String,
    pub size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub game_path: String,
    pub game_name: String,
    /// Campo de la gamelist del tipo (image, fanart, video…)
    pub field: String,
    /// Por qué se sabe de qué juego es: name / old_name / dat / mame / broken_ref
    pub reason: String,
    /// Lo que el juego tiene ahora de ese tipo (si tiene)
    pub current: Option<CurrentMedia>,
    /// Propuesta: quedarse con este (true) o con el actual (false)
    pub keep_new: bool,
    /// Arregla una referencia rota de la gamelist (ese campo apunta a un fichero que no existe)
    pub fixes_broken: bool,
}

/// Lo elegido para incorporar
#[derive(Debug, Clone, Deserialize)]
pub struct ReuseChoice {
    pub path: String,
    pub game_path: String,
    pub field: String,
}

// ---------- Nombres de cada juego ----------

/// Artículos que se pueden poner al final («Punisher, The»)
const ARTICLES: &[&str] = &["the", "a", "an", "el", "la", "los", "las", "le", "les", "der", "die", "das", "il", "lo"];

/// El nombre con el artículo al otro lado: «King of Dragons, The» ↔ «The King of Dragons».
/// Solo la parte antes de las etiquetas. None si no lleva artículo.
fn article_swapped(name: &str) -> Option<String> {
    let cut = name.find(['(', '[']).unwrap_or(name.len());
    let (main, rest) = (name[..cut].trim(), &name[cut..]);
    if let Some((before, after)) = main.rsplit_once(", ") {
        if ARTICLES.contains(&after.trim().to_lowercase().as_str()) {
            return Some(format!("{} {} {}", after.trim(), before, rest));
        }
    }
    let (first, others) = main.split_once(' ')?;
    ARTICLES.contains(&first.to_lowercase().as_str()).then(|| format!("{}, {} {}", others, first, rest))
}

/// Claves de un nombre: la normal y, si lleva artículo, la del artículo al otro lado
fn keys_of_name(name: &str) -> Vec<String> {
    let mut keys = vec![title_key(name)];
    if let Some(swapped) = article_swapped(name) {
        keys.push(title_key(&swapped));
    }
    keys.retain(|k| !k.is_empty());
    keys.dedup();
    keys
}

fn file_name_of(path: &Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// Nombre → juego (solo los nombres que llevan a un único juego) y por qué
struct Owners {
    by_key: HashMap<String, (HashSet<String>, &'static str)>,
}

impl Owners {
    fn new(system: &SystemRef, games: &[Game]) -> Self {
        let mut owners = Owners { by_key: HashMap::new() };
        for game in games {
            let stem = file_name_of(&crate::layout::rom_media_stem(system, &game.path));
            for key in keys_of_name(&stem).into_iter().chain(game.name.as_deref().map(keys_of_name).unwrap_or_default()) {
                owners.add(key, &game.path, "name");
            }
        }
        owners
    }

    fn add(&mut self, key: String, game_path: &str, reason: &'static str) {
        if key.is_empty() {
            return;
        }
        let entry = self.by_key.entry(key).or_insert_with(|| (HashSet::new(), reason));
        entry.0.insert(game_path.to_string());
    }

    /// El juego de un nombre, si es de uno solo
    fn find(&self, key: &str) -> Option<(&str, &'static str)> {
        let (games, reason) = self.by_key.get(key)?;
        if games.len() != 1 {
            return None;
        }
        games.iter().next().map(|g| (g.as_str(), *reason))
    }
}

/// El juego de un valor en un índice que solo guarda los valores de un único juego
fn found<'a>(map: &HashMap<String, Option<&'a Game>>, value: Option<String>) -> Option<&'a Game> {
    value.and_then(|v| map.get(&v).copied().flatten())
}

/// Nombres antiguos de las ROMs según las copias de la gamelist: (nombre antiguo, juego de hoy).
/// El juego de hoy se reconoce por el id de ScreenScraper, el MD5 o el título (si llevan a uno solo).
fn old_names(layout: &Layout, system: &SystemRef, games: &[Game]) -> Vec<(String, String)> {
    let Ok(entries) = fs::read_dir(layout.backups_dir(system)) else { return Vec::new() };
    let current: HashSet<&str> = games.iter().map(|g| g.path.as_str()).collect();
    let unique = |pick: &dyn Fn(&Game) -> Option<String>| {
        let mut map: HashMap<String, Option<&Game>> = HashMap::new();
        for game in games {
            if let Some(v) = pick(game).filter(|v| !v.is_empty() && v != "0") {
                map.entry(v).and_modify(|e| *e = None).or_insert(Some(game));
            }
        }
        map
    };
    let by_id = unique(&|g| g.id.clone());
    let by_md5 = unique(&|g| g.md5.clone().map(|m| m.to_lowercase()));
    let by_title = unique(&|g| g.name.as_deref().map(title_key));

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = file_name_of(&path);
        if !name.starts_with("gamelist_") || !name.ends_with(".xml") {
            continue;
        }
        let Some(old) = fs::read_to_string(&path).ok().and_then(|xml| crate::parser::parse_gamelist(&xml).ok()) else { continue };
        for game in old.games {
            if current.contains(game.path.as_str()) || !seen.insert(game.path.clone()) {
                continue;
            }
            let now = found(&by_id, game.id.clone())
                .or_else(|| found(&by_md5, game.md5.clone().map(|m| m.to_lowercase())))
                .or_else(|| found(&by_title, game.name.as_deref().map(title_key)));
            if let Some(now) = now {
                out.push((file_name_of(&crate::layout::rom_media_stem(system, &game.path)), now.path.clone()));
            }
        }
    }
    out
}

// ---------- Búsqueda ----------

/// Lo que hace falta además de los juegos para reconocer los sobrantes
#[derive(Default)]
pub(super) struct Hints {
    /// (nombre, juego, motivo): nombres antiguos y del DAT
    pub aliases: Vec<(String, String, &'static str)>,
    /// LaunchBox: nombre corto de MAME → título
    pub mame_titles: HashMap<String, String>,
}

/// Tipo y nombre base de un sobrante: por su sufijo antiguo o, sin él, por su carpeta.
/// `None` en el tipo = `-boxart` (carátula o caja 3D, según la forma).
fn kind_and_base(path: &Path) -> Option<(Option<&'static MediaKind>, String)> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let lower = stem.to_lowercase();
    if let Some((suffix, folder)) = TITLE_SUFFIXES.iter().find(|(suffix, _)| lower.ends_with(suffix)) {
        let base = stem[..stem.len() - suffix.len()].to_string();
        let kind = if folder.is_empty() { None } else { Some(media::kind_for_folder(folder)?) };
        return Some((kind, base));
    }
    let folder = path.parent().map(|p| file_name_of(p).to_lowercase()).unwrap_or_default();
    Some((Some(media::kind_for_folder(&folder)?), stem))
}

fn image_size(path: &Path) -> (Option<u32>, Option<u32>) {
    match super::media_kinds::info(path) {
        Some(i) => (Some(i.width), Some(i.height)),
        None => (None, None),
    }
}

fn area(w: Option<u32>, h: Option<u32>) -> u64 {
    w.unwrap_or(0) as u64 * h.unwrap_or(0) as u64
}

pub(super) fn find_reusable(
    layout: &Layout,
    system: &SystemRef,
    games: &[Game],
    leftovers: &[PathBuf],
    broken: &[BrokenMediaRef],
    hints: &Hints,
) -> Vec<ReusableMedia> {
    let mut owners = Owners::new(system, games);
    for (name, game, reason) in &hints.aliases {
        for key in keys_of_name(name) {
            owners.add(key, game, reason);
        }
    }
    let index = MediaIndex::build(layout, system);
    let used: HashSet<PathBuf> = games
        .iter()
        .flat_map(|g| media::media_fields().filter_map(|f| media::get_field(g, f)).map(|v| media::resolve_media_path(system, v)))
        .collect();
    let by_path: HashMap<&str, &Game> = games.iter().map(|g| (g.path.as_str(), g)).collect();
    // Carátula habitual del sistema, para decidir los `-boxart` (se mide al hacer falta)
    let mut usual_cover: Option<Option<super::media_kinds::MediaProfile>> = None;

    let current_of = |game: &Game, kind: &MediaKind| -> Option<PathBuf> {
        media::get_field(game, kind.field)
            .map(|v| media::resolve_media_path(system, v))
            .filter(|p| p.is_file())
            .or_else(|| index.find(system, kind, &game.path).cloned())
    };
    let item = |path: &Path, game: &Game, kind: &MediaKind, reason: &str| -> Option<ReusableMedia> {
        let mut current = current_of(game, kind);
        if current.as_deref() == Some(path) {
            // Una referencia rota cuyo fichero ya tiene el nombre bueno: solo hay que enlazarlo
            if reason != "broken_ref" {
                return None;
            }
            current = None;
        }
        let (width, height) = image_size(path);
        let current = current.map(|c| {
            let (w, h) = image_size(&c);
            CurrentMedia {
                relative: relative_to(layout, system, &c),
                size: fs::metadata(&c).map(|m| m.len()).unwrap_or(0),
                path: c.to_string_lossy().to_string(),
                width: w,
                height: h,
            }
        });
        // Con uno actual, el de más píxeles (con empate o sin poder medir, el actual)
        let keep_new = current.as_ref().is_none_or(|c| area(width, height) > area(c.width, c.height));
        Some(ReusableMedia {
            path: path.to_string_lossy().to_string(),
            relative: relative_to(layout, system, path),
            size: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            width,
            height,
            game_path: game.path.clone(),
            game_name: game.name.clone().unwrap_or_else(|| game.path.clone()),
            field: kind.field.to_string(),
            reason: reason.to_string(),
            current,
            keep_new,
            fixes_broken: false,
        })
    };

    let mut items: Vec<ReusableMedia> = Vec::new();
    for path in leftovers {
        if used.contains(path) {
            continue;
        }
        let Some((kind, base)) = kind_and_base(path) else { continue };
        let kind = match kind {
            Some(kind) => kind,
            None => {
                let cover = *usual_cover.get_or_insert_with(|| super::media_kinds::usual_sizes(layout, system, games).0);
                let is_cover = match (cover, super::media_kinds::info(path)) {
                    (Some(c), Some(img)) => super::media_kinds::same_type(img.width, img.height, c.width, c.height),
                    _ => true,
                };
                match media::kind_for_folder(if is_cover { "covers" } else { "3dboxes" }) {
                    Some(k) => k,
                    None => continue,
                }
            }
        };
        // La extensión tiene que valer para ese tipo (un .mp4 no es una carátula)
        if !media::extension_lower(path).is_some_and(|e| kind.extensions().contains(&e.as_str())) {
            continue;
        }
        let owner = keys_of_name(&base).iter().find_map(|k| owners.find(k)).or_else(|| {
            // Nombre corto de MAME → título de LaunchBox → juego
            let title = hints.mame_titles.get(&base.to_lowercase())?;
            keys_of_name(title).iter().find_map(|k| owners.find(k)).map(|(g, _)| (g, "mame"))
        });
        let Some((game_path, reason)) = owner else { continue };
        let Some(game) = by_path.get(game_path) else { continue };
        if let Some(found) = item(path, game, kind, reason) {
            items.push(found);
        }
    }

    // Varios para el mismo juego y tipo: solo el mejor (más píxeles, luego más grande); los demás
    // siguen como sobrantes
    items.sort_by(|a, b| {
        (a.game_path.as_str(), a.field.as_str())
            .cmp(&(b.game_path.as_str(), b.field.as_str()))
            .then(area(b.width, b.height).cmp(&area(a.width, a.height)))
            .then(b.size.cmp(&a.size))
    });
    items.dedup_by(|b, a| a.game_path == b.game_path && a.field == b.field);

    // Referencias rotas: un sobrante ya propuesto para ese juego y tipo, o el mismo nombre con
    // otra extensión, o el mismo fichero en otra carpeta de medios
    for r in broken {
        if let Some(found) = items.iter_mut().find(|i| i.game_path == r.game_path && i.field == r.field) {
            found.fixes_broken = true;
            continue;
        }
        let (Some(kind), Some(game)) = (media::kind_for_field(&r.field), by_path.get(r.game_path.as_str())) else { continue };
        if let Some(path) = find_moved(system, kind, &media::resolve_media_path(system, &r.value), &used) {
            if let Some(mut found) = item(&path, game, kind, "broken_ref") {
                found.fixes_broken = true;
                found.keep_new = true;
                items.push(found);
            }
        }
    }
    items.sort_by(|a, b| a.game_name.to_lowercase().cmp(&b.game_name.to_lowercase()).then(a.field.cmp(&b.field)));
    items
}

/// El fichero de una referencia rota, si sigue por ahí: mismo nombre con otra extensión del tipo,
/// o el mismo nombre de fichero en otra carpeta de medios del sistema. Nunca uno que ya use otro.
fn find_moved(system: &SystemRef, kind: &MediaKind, missing: &Path, used: &HashSet<PathBuf>) -> Option<PathBuf> {
    let free = |p: &Path| p.is_file() && !used.contains(p);
    if let (Some(dir), Some(stem)) = (missing.parent(), missing.file_stem()) {
        for ext in kind.extensions() {
            let candidate = dir.join(format!("{}.{}", stem.to_string_lossy(), ext));
            if free(&candidate) {
                return Some(candidate);
            }
        }
    }
    let name = missing.file_name()?;
    super::cleanup::MEDIA_DIRS.iter().map(|d| system.rom_dir.join(d).join(name)).find(|p| free(p))
}

// ---------- Comandos ----------

/// Los sobrantes y referencias rotas que se pueden incorporar a un juego, con la propuesta
#[tauri::command(async)]
pub fn find_reusable_media(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<Vec<ReusableMedia>, String> {
    let (a, sp) = (app.clone(), system_path.clone());
    super::system_status::remember(&app, &system_path, "reuse", "", move || find_reusable_media_now(a, sp, task))
}

fn find_reusable_media_now(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<Vec<ReusableMedia>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task.clone());
    let games = super::gamelist::read_gamelist_with(&layout, &system, &progress)?.games;
    progress.check()?;
    let leftovers: Vec<PathBuf> = super::cleanup::find_unused_media(app.clone(), system_path.clone(), None)?
        .into_iter()
        .filter(|f| !f.recoverable)
        .map(|f| PathBuf::from(f.path))
        .collect();
    let broken = super::cleanup::find_broken_media_refs(app.clone(), system_path.clone(), None, None)?;
    progress.check()?;
    progress.phase("media");

    let mut hints = Hints::default();
    for (name, game) in old_names(&layout, &system, &games) {
        hints.aliases.push((name, game, "old_name"));
    }
    let roms: Vec<PathBuf> = games.iter().map(|g| system.rom_dir.join(crate::layout::strip_dot_slash(&g.path))).collect();
    let dat_names = super::dat::dat_names(&app, &system, &roms).unwrap_or_default();
    for (game, rom) in games.iter().zip(&roms) {
        if let Some(name) = dat_names.get(rom) {
            hints.aliases.push((name.clone(), game.path.clone(), "dat"));
        }
    }
    if super::scraper_adb::is_arcade_system(&system.name) {
        hints.mame_titles = super::scraper_launchbox::mame_titles(&app);
    }
    Ok(find_reusable(&layout, &system, &games, &leftovers, &broken, &hints))
}

/// Incorpora lo elegido: cada fichero al nombre unificado de su juego y tipo (lo que no se queda
/// ya lo ha quitado el frontend). Es lo mismo que aplicar Normalizar: se puede deshacer.
#[tauri::command(async)]
pub fn apply_media_reuse(app: tauri::AppHandle, system_path: String, choices: Vec<ReuseChoice>, task: Option<String>) -> Result<NormalizeResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let paths: Vec<String> = choices.iter().map(|c| c.path.clone()).collect();
    let (allowed, _) = super::cleanup::allowed_media_paths(&layout, &system, &paths);
    let allowed: HashSet<PathBuf> = allowed.into_iter().collect();
    let items: Vec<NormalizeItem> = choices
        .iter()
        .filter_map(|c| {
            let from = PathBuf::from(&c.path);
            if !allowed.contains(&from) {
                return None;
            }
            let kind = media::kind_for_field(&c.field)?;
            let ext = media::extension_lower(&from)?;
            let to = layout.media_file_path(&system, kind.folder, &c.game_path, &ext);
            Some(NormalizeItem {
                game_path: c.game_path.clone(),
                game_name: c.game_path.clone(),
                field: c.field.clone(),
                from_relative: relative_to(&layout, &system, &from),
                to_relative: relative_to(&layout, &system, &to),
                status: if from == to { NormalizeStatus::Link } else { NormalizeStatus::Move },
                from: c.path.clone(),
                to: to.to_string_lossy().to_string(),
            })
        })
        .collect();
    super::normalize::apply_media_normalization(app, system_path, items, task)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(path: &str, name: &str) -> Game {
        Game { path: path.into(), name: Some(name.into()), ..Default::default() }
    }

    fn touch(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    /// PNG mínimo: solo la cabecera con el tamaño (es lo único que se lee)
    fn png(path: &Path, width: u32, height: u32) {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        touch(path, &bytes);
    }

    #[test]
    fn articulo_al_otro_lado() {
        assert_eq!(title_key(&article_swapped("King of Dragons, The").unwrap()), title_key("The King of Dragons"));
        assert_eq!(title_key(&article_swapped("The Punisher (USA)").unwrap()), title_key("Punisher, The"));
        assert!(article_swapped("Strider").is_none());
    }

    #[test]
    fn reconoce_el_nombre_antiguo_y_propone_el_de_mas_resolucion() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("cps1"));
        let mut three = game("./Three Wonders.zip", "Three Wonders");
        // Ya tiene carátula, más pequeña que la sobrante
        png(&system.rom_dir.join("covers/Three Wonders.png"), 100, 140);
        three.image = Some("./covers/Three Wonders.png".into());
        png(&system.rom_dir.join("images/3wonders-image.png"), 300, 420);
        touch(&system.rom_dir.join("images/3wonders-fanart.jpg"), b"f");
        // Un sobrante de nadie
        touch(&system.rom_dir.join("images/nadie-fanart.jpg"), b"n");
        let leftovers = vec![
            system.rom_dir.join("images/3wonders-image.png"),
            system.rom_dir.join("images/3wonders-fanart.jpg"),
            system.rom_dir.join("images/nadie-fanart.jpg"),
        ];
        let hints = Hints { aliases: vec![("3wonders".into(), "./Three Wonders.zip".into(), "old_name")], ..Default::default() };

        let found = find_reusable(&Layout::Classic, &system, &[three], &leftovers, &[], &hints);

        assert_eq!(found.len(), 2);
        let cover = found.iter().find(|i| i.field == "image").unwrap();
        assert_eq!(cover.reason, "old_name");
        assert!(cover.keep_new);
        assert_eq!(cover.current.as_ref().unwrap().width, Some(100));
        let fanart = found.iter().find(|i| i.field == "fanart").unwrap();
        assert!(fanart.current.is_none() && fanart.keep_new);
    }

    #[test]
    fn un_nombre_de_dos_juegos_no_se_propone() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("nes"));
        let games = vec![game("./Tetris (USA).nes", "Tetris"), game("./Tetris (Japan).nes", "Tetris")];
        touch(&system.rom_dir.join("images/Tetris-image.jpg"), b"t");
        let found = find_reusable(&Layout::Classic, &system, &games, &[system.rom_dir.join("images/Tetris-image.jpg")], &[], &Hints::default());
        assert!(found.is_empty());
    }

    #[test]
    fn mame_por_launchbox() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("cps1"));
        let games = vec![game("./King of Dragons, The.zip", "King of Dragons, The")];
        touch(&system.rom_dir.join("videos/kod-video.mp4"), b"v");
        let hints = Hints { mame_titles: HashMap::from([("kod".to_string(), "The King of Dragons".to_string())]), ..Default::default() };
        let found = find_reusable(&Layout::Classic, &system, &games, &[system.rom_dir.join("videos/kod-video.mp4")], &[], &hints);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].field.as_str(), found[0].reason.as_str()), ("video", "mame"));
    }

    #[test]
    fn referencia_rota_con_otra_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        let mut g = game("./Mario.sfc", "Mario");
        g.image = Some("./covers/Mario.png".into());
        touch(&system.rom_dir.join("covers/Mario.jpg"), b"j");
        let broken = vec![BrokenMediaRef { game_path: "./Mario.sfc".into(), game_name: "Mario".into(), field: "image".into(), value: "./covers/Mario.png".into() }];
        let found = find_reusable(&Layout::Classic, &system, &[g], &[], &broken, &Hints::default());
        assert_eq!(found.len(), 1);
        assert!(found[0].fixes_broken && found[0].keep_new);
        assert!(found[0].path.ends_with("covers/Mario.jpg"));
    }
}
