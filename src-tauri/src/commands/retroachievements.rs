//! RetroAchievements con la cuenta del usuario (usuario + clave de API web en Ajustes): perfil,
//! logros y progreso de un juego, y la comprobación de un sistema entero (qué ROMs son versiones
//! reconocidas). La huella de cada ROM la calcula `crate::retroachievements`.
//!
//! Se guardan en la carpeta de datos de la app: `ra/<consola>.json` (juegos de la consola con sus
//! huellas; se renueva a los 7 días), `ra_hashes.json` (huellas ya calculadas, por ruta + tamaño +
//! fecha) y `ra_marks.json` (el resultado de «Comprobar logros» de cada sistema).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::Manager;

use super::progress::Progress;
use super::scrape_common::normalize_title;
use super::{gamelist, settings, system_context};
use crate::layout::{strip_dot_slash, SystemRef};
use crate::retroachievements::{console_for, rom_hash, HashMethod};

const API: &str = "https://retroachievements.org/API";
const MEDIA: &str = "https://media.retroachievements.org";
/// La lista de juegos de una consola se vuelve a pedir pasado este tiempo
const LIST_MAX_AGE: i64 = 7 * 24 * 3600;
/// El perfil se guarda en memoria este rato (la pantalla de inicio se abre a menudo)
const PROFILE_TTL: Duration = Duration::from_secs(300);

// ---------- Tipos que ve la app ----------

#[derive(Debug, Clone, Serialize)]
pub struct RaProfile {
    pub user: String,
    pub user_pic: String,
    pub points: u64,
    pub true_points: u64,
    pub rank: Option<u64>,
    pub total_ranked: Option<u64>,
    pub recent_games: Vec<RaRecentGame>,
    pub recent_achievements: Vec<RaRecentAchievement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RaRecentGame {
    pub game_id: u64,
    pub title: String,
    pub console: String,
    pub icon: String,
    pub last_played: String,
    pub achievements_total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RaRecentAchievement {
    pub title: String,
    pub description: String,
    pub points: u64,
    pub badge: String,
    pub game_title: String,
    pub date: String,
    pub hardcore: bool,
}

/// Lo que se sabe de un juego: si la consola se puede comprobar, la huella, cómo se encontró en
/// RetroAchievements y, si se encontró, sus logros con el progreso del usuario
#[derive(Debug, Clone, Serialize)]
pub struct RaGameReport {
    /// La consola existe en RetroAchievements
    pub known_console: bool,
    /// Su huella se sabe calcular (no es un disco)
    pub hashable: bool,
    pub hash: Option<String>,
    /// "hash" (ROM compatible) | "name" (el juego tiene logros, pero esta versión no) | "none"
    pub matched: String,
    pub game: Option<RaGameDetail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RaGameDetail {
    pub id: u64,
    pub title: String,
    pub icon: String,
    pub points: u64,
    pub num_achievements: u64,
    pub awarded: u64,
    pub awarded_hardcore: u64,
    pub achievements: Vec<RaAchievement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RaAchievement {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub points: u64,
    pub badge: String,
    pub badge_locked: String,
    pub earned: Option<String>,
    pub earned_hardcore: Option<String>,
    pub order: u64,
}

/// La marca de un juego tras «Comprobar logros» (insignia y filtro)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaMark {
    /// "compatible" | "other" (tiene logros, pero esta versión no) | "none" | "unsupported"
    pub status: String,
    pub ra_id: Option<u64>,
    pub achievements: u64,
    /// Logros conseguidos por el usuario (normal; los hardcore cuentan también)
    pub awarded: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RaCheckSummary {
    pub total: usize,
    pub compatible: usize,
    pub other: usize,
    pub none: usize,
    pub unsupported: usize,
}

// ---------- Comandos ----------

/// Comprueba la cuenta: devuelve el perfil si el usuario y la clave valen
#[tauri::command]
pub async fn ra_check_account(app: tauri::AppHandle) -> Result<RaProfile, String> {
    *PROFILE.lock().unwrap() = None;
    ra_profile(app).await
}

/// Perfil del usuario para la pantalla de inicio (unos minutos en memoria)
#[tauri::command]
pub async fn ra_profile(app: tauri::AppHandle) -> Result<RaProfile, String> {
    if let Some((at, profile)) = PROFILE.lock().unwrap().as_ref() {
        if at.elapsed() < PROFILE_TTL {
            return Ok(profile.clone());
        }
    }
    let creds = credentials(&app)?;
    let value = get(&creds, "API_GetUserSummary.php", &[("u", &creds.user), ("g", "5"), ("a", "6")]).await?;
    let profile = parse_profile(&value, &creds.user);
    *PROFILE.lock().unwrap() = Some((Instant::now(), profile.clone()));
    Ok(profile)
}

/// Logros de un juego: su huella, cómo se encuentra en RetroAchievements y el progreso del usuario
#[tauri::command]
pub async fn ra_game(app: tauri::AppHandle, system_path: String, game_path: String) -> Result<RaGameReport, String> {
    let creds = credentials(&app)?;
    let (_, system) = system_context(&app, &system_path)?;
    let Some(console) = console_for(&system.name) else {
        return Ok(RaGameReport { known_console: false, hashable: false, hash: None, matched: "none".into(), game: None });
    };
    let rom = rom_path(&system, &game_path);
    let hashable = console.method != HashMethod::Unsupported;
    let hash = if hashable {
        let app2 = app.clone();
        let rom2 = rom.clone();
        tauri::async_runtime::spawn_blocking(move || HashCache::load(&app2).hash(&rom2, console.method, true))
            .await
            .map_err(|e| e.to_string())?
    } else {
        None
    };
    let list = console_games(&app, &creds, console.id).await?;
    // El nombre del juego en el gamelist, para buscarlo si la huella no está
    let name = gamelist::read_gamelist(&super::active_layout(&app)?, &system)?
        .games
        .into_iter()
        .find(|g| g.path == game_path)
        .and_then(|g| g.name)
        .unwrap_or_else(|| rom.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    let (matched, id) = match_game(&list, hash.as_deref(), &name);
    let game = match id {
        Some(id) => Some(game_detail(&creds, id).await?),
        None => None,
    };
    Ok(RaGameReport { known_console: true, hashable, hash, matched: matched.into(), game })
}

/// «Comprobar logros»: las huellas de todas las ROMs del sistema contra RetroAchievements y el
/// progreso del usuario; se guarda para la insignia y el filtro
#[tauri::command]
pub async fn ra_check_system(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<RaCheckSummary, String> {
    let creds = credentials(&app)?;
    let (layout, system) = system_context(&app, &system_path)?;
    let console = console_for(&system.name).ok_or("RetroAchievements doesn't support this system")?;
    let progress = Progress::new(&app, task);
    progress.phase("raList");
    let list = console_games(&app, &creds, console.id).await?;
    progress.phase("raProgress");
    let awarded = user_progress(&creds).await.unwrap_or_default();

    // Las huellas, en un hilo (leen ficheros)
    let app2 = app.clone();
    let games = gamelist::read_gamelist(&layout, &system)?.games;
    let marks = tauri::async_runtime::spawn_blocking(move || -> Result<HashMap<String, RaMark>, String> {
        let mut cache = HashCache::load(&app2);
        let mut marks = HashMap::new();
        for (i, game) in games.iter().enumerate() {
            if progress.cancelled() {
                break;
            }
            progress.step("raHashing", i + 1, games.len(), game.name.as_deref().unwrap_or(&game.path));
            let hash = if console.method == HashMethod::Unsupported {
                None
            } else {
                cache.hash(&rom_path(&system, &game.path), console.method, false)
            };
            let name = game.name.clone().unwrap_or_else(|| game.path.clone());
            let (matched, id) = match_game(&list, hash.as_deref(), &name);
            let entry = id.and_then(|id| list.games.iter().find(|g| g.id == id));
            let status = match (console.method == HashMethod::Unsupported, matched) {
                (_, "hash") => "compatible",
                (_, "name") => "other",
                (true, _) => "unsupported",
                _ => "none",
            };
            marks.insert(
                game.path.clone(),
                RaMark {
                    status: status.into(),
                    ra_id: id,
                    achievements: entry.map_or(0, |e| e.num_achievements),
                    awarded: id.and_then(|id| awarded.get(&id).copied()).unwrap_or(0),
                },
            );
        }
        cache.save();
        Ok(marks)
    })
    .await
    .map_err(|e| e.to_string())??;

    let mut summary = RaCheckSummary { total: marks.len(), ..Default::default() };
    for m in marks.values() {
        match m.status.as_str() {
            "compatible" => summary.compatible += 1,
            "other" => summary.other += 1,
            "unsupported" => summary.unsupported += 1,
            _ => summary.none += 1,
        }
    }
    let mut all = read_json::<HashMap<String, HashMap<String, RaMark>>>(&data_file(&app, "ra_marks.json")?).unwrap_or_default();
    all.insert(system_path, marks);
    write_json(&data_file(&app, "ra_marks.json")?, &all)?;
    Ok(summary)
}

/// Lo que dejó «Comprobar logros» en un sistema (vacío si no se ha comprobado)
#[tauri::command(async)]
pub fn ra_system_marks(app: tauri::AppHandle, system_path: String) -> Result<HashMap<String, RaMark>, String> {
    let all = read_json::<HashMap<String, HashMap<String, RaMark>>>(&data_file(&app, "ra_marks.json")?).unwrap_or_default();
    Ok(all.get(&system_path).cloned().unwrap_or_default())
}

// ---------- Cuenta y peticiones ----------

static PROFILE: Mutex<Option<(Instant, RaProfile)>> = Mutex::new(None);

struct Credentials {
    user: String,
    key: String,
}

fn credentials(app: &tauri::AppHandle) -> Result<Credentials, String> {
    let s = settings::get_settings(app.clone())?;
    let user = s.ra_user.unwrap_or_default().trim().to_string();
    let key = s.ra_apikey.unwrap_or_default().trim().to_string();
    if user.is_empty() || key.is_empty() {
        return Err("RetroAchievements: missing user or web API key (Settings → General)".into());
    }
    Ok(Credentials { user, key })
}

/// Una llamada a la API web. Los errores nunca llevan la clave (va en la URL).
async fn get(creds: &Credentials, endpoint: &str, params: &[(&str, &str)]) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("meROMS/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let mut query: Vec<(&str, &str)> = vec![("y", &creds.key)];
    query.extend_from_slice(params);
    let response = client
        .get(format!("{}/{}", API, endpoint))
        .query(&query)
        .send()
        .await
        .map_err(|e| format!("RetroAchievements: {}", e.without_url()))?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("RetroAchievements: the user or the web API key is not valid".into());
    }
    if !status.is_success() {
        return Err(format!("RetroAchievements: HTTP {}", status));
    }
    response.json::<Value>().await.map_err(|e| format!("RetroAchievements: {}", e.without_url()))
}

/// Ficha del juego con el progreso del usuario
async fn game_detail(creds: &Credentials, id: u64) -> Result<RaGameDetail, String> {
    let value = get(creds, "API_GetGameInfoAndUserProgress.php", &[("g", &id.to_string()), ("u", &creds.user)]).await?;
    Ok(parse_game_detail(&value, id))
}

/// Logros conseguidos por el usuario en cada juego (por id), de todas sus páginas
async fn user_progress(creds: &Credentials) -> Result<HashMap<u64, u64>, String> {
    let mut out = HashMap::new();
    let mut offset = 0usize;
    loop {
        let value = get(creds, "API_GetUserCompletionProgress.php", &[("u", &creds.user), ("c", "500"), ("o", &offset.to_string())]).await?;
        let results = value.get("Results").and_then(Value::as_array).cloned().unwrap_or_default();
        for r in &results {
            let id = num(r.get("GameID"));
            let awarded = num(r.get("NumAwarded")).max(num(r.get("NumAwardedHardcore")));
            out.insert(id, awarded);
        }
        offset += results.len();
        let total = num(value.get("Total")) as usize;
        if results.is_empty() || offset >= total {
            break;
        }
    }
    Ok(out)
}

// ---------- Juegos de una consola (con sus huellas) ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConsoleList {
    fetched_at: i64,
    games: Vec<ListGame>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ListGame {
    id: u64,
    title: String,
    num_achievements: u64,
    hashes: Vec<String>,
}

/// La lista guardada si es reciente; si no, se pide (con huellas, solo juegos con logros)
async fn console_games(app: &tauri::AppHandle, creds: &Credentials, console: u32) -> Result<ConsoleList, String> {
    let file = data_file(app, &format!("ra/{}.json", console))?;
    let now = chrono::Utc::now().timestamp();
    if let Some(list) = read_json::<ConsoleList>(&file) {
        if now - list.fetched_at < LIST_MAX_AGE {
            return Ok(list);
        }
    }
    let value = get(creds, "API_GetGameList.php", &[("i", &console.to_string()), ("h", "1"), ("f", "1")]).await?;
    let games = value
        .as_array()
        .map(|list| {
            list.iter()
                .map(|g| ListGame {
                    id: num(g.get("ID")),
                    title: text(g.get("Title")),
                    num_achievements: num(g.get("NumAchievements")),
                    hashes: g
                        .get("Hashes")
                        .and_then(Value::as_array)
                        .map(|h| h.iter().filter_map(Value::as_str).map(str::to_lowercase).collect())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    let list = ConsoleList { fetched_at: now, games };
    write_json(&file, &list)?;
    Ok(list)
}

/// Busca el juego: por huella (versión compatible) o, si no, por el nombre (tiene logros, pero
/// para otra versión). Devuelve cómo se encontró y su id.
fn match_game(list: &ConsoleList, hash: Option<&str>, name: &str) -> (&'static str, Option<u64>) {
    if let Some(hash) = hash.map(str::to_lowercase) {
        if let Some(g) = list.games.iter().find(|g| g.hashes.contains(&hash)) {
            return ("hash", Some(g.id));
        }
    }
    let wanted = normalize_title(name);
    if !wanted.is_empty() {
        if let Some(g) = list.games.iter().find(|g| normalize_title(&g.title) == wanted) {
            return ("name", Some(g.id));
        }
    }
    ("none", None)
}

// ---------- Huellas ya calculadas ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HashEntry {
    size: u64,
    modified: u64,
    hash: Option<String>,
}

struct HashCache {
    file: Option<PathBuf>,
    entries: HashMap<String, HashEntry>,
    dirty: bool,
}

impl HashCache {
    fn load(app: &tauri::AppHandle) -> Self {
        let file = data_file(app, "ra_hashes.json").ok();
        let entries = file.as_deref().and_then(read_json).unwrap_or_default();
        HashCache { file, entries, dirty: false }
    }

    /// La huella de una ROM: guardada si el fichero no ha cambiado; si no, calculada.
    /// Con `save_now`, se guarda al momento (una sola ROM).
    fn hash(&mut self, rom: &Path, method: HashMethod, save_now: bool) -> Option<String> {
        // Arcade: es el nombre, no hace falta leer nada
        if method == HashMethod::Arcade {
            return rom_hash(rom, method);
        }
        let key = rom.to_string_lossy().to_string();
        let (size, modified) = stamp(rom)?;
        if let Some(e) = self.entries.get(&key) {
            if e.size == size && e.modified == modified {
                return e.hash.clone();
            }
        }
        let hash = rom_hash(rom, method);
        self.entries.insert(key, HashEntry { size, modified, hash: hash.clone() });
        self.dirty = true;
        if save_now {
            self.save();
        }
        hash
    }

    fn save(&mut self) {
        if let (Some(file), true) = (&self.file, self.dirty) {
            let _ = write_json(file, &self.entries);
            self.dirty = false;
        }
    }
}

fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some((meta.len(), modified))
}

// ---------- Lectura de respuestas ----------

/// Un número que la API puede mandar como número o como texto
fn num(value: Option<&Value>) -> u64 {
    match value {
        Some(Value::Number(n)) => n.as_u64().or_else(|| n.as_f64().map(|f| f as u64)).unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        Some(Value::Bool(b)) => u64::from(*b),
        _ => 0,
    }
}

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Una ruta de imagen de la API (/Images/x.png, /UserPic/x.png) como URL completa
fn media_url(path: &str) -> String {
    if path.is_empty() || path.starts_with("http") {
        path.to_string()
    } else {
        format!("{}/{}", MEDIA, path.trim_start_matches('/'))
    }
}

fn badge_url(badge: &str, locked: bool) -> String {
    format!("{}/Badge/{}{}.png", MEDIA, badge, if locked { "_lock" } else { "" })
}

fn parse_profile(v: &Value, user: &str) -> RaProfile {
    let recent_games = v
        .get("RecentlyPlayed")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|g| RaRecentGame {
                    game_id: num(g.get("GameID")),
                    title: text(g.get("Title")),
                    console: text(g.get("ConsoleName")),
                    icon: media_url(&text(g.get("ImageIcon"))),
                    last_played: text(g.get("LastPlayed")),
                    achievements_total: num(g.get("AchievementsTotal")),
                })
                .collect()
        })
        .unwrap_or_default();
    // Los logros recientes vienen por juego y por logro; los más nuevos primero
    let mut recent_achievements: Vec<RaRecentAchievement> = v
        .get("RecentAchievements")
        .and_then(Value::as_object)
        .map(|games| {
            games
                .values()
                .filter_map(Value::as_object)
                .flat_map(|achs| achs.values())
                .map(|a| RaRecentAchievement {
                    title: text(a.get("Title")),
                    description: text(a.get("Description")),
                    points: num(a.get("Points")),
                    badge: badge_url(&text(a.get("BadgeName")), false),
                    game_title: text(a.get("GameTitle")),
                    date: text(a.get("DateAwarded")),
                    hardcore: num(a.get("HardcoreAchieved")) > 0,
                })
                .collect()
        })
        .unwrap_or_default();
    recent_achievements.sort_by(|a, b| b.date.cmp(&a.date));
    let rank = num(v.get("Rank"));
    let total_ranked = num(v.get("TotalRanked"));
    RaProfile {
        user: Some(text(v.get("User"))).filter(|u| !u.is_empty()).unwrap_or_else(|| user.to_string()),
        user_pic: media_url(&text(v.get("UserPic"))),
        points: num(v.get("TotalPoints")),
        true_points: num(v.get("TotalTruePoints")),
        rank: (rank > 0).then_some(rank),
        total_ranked: (total_ranked > 0).then_some(total_ranked),
        recent_games,
        recent_achievements,
    }
}

fn parse_game_detail(v: &Value, id: u64) -> RaGameDetail {
    let mut achievements: Vec<RaAchievement> = v
        .get("Achievements")
        .and_then(Value::as_object)
        .map(|achs| {
            achs.values()
                .map(|a| {
                    let badge = text(a.get("BadgeName"));
                    let date = |key: &str| Some(text(a.get(key))).filter(|d| !d.is_empty());
                    RaAchievement {
                        id: num(a.get("ID")),
                        title: text(a.get("Title")),
                        description: text(a.get("Description")),
                        points: num(a.get("Points")),
                        badge: badge_url(&badge, false),
                        badge_locked: badge_url(&badge, true),
                        earned: date("DateEarned"),
                        earned_hardcore: date("DateEarnedHardcore"),
                        order: num(a.get("DisplayOrder")),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    achievements.sort_by_key(|a| (a.order, a.id));
    let points = achievements.iter().map(|a| a.points).sum();
    RaGameDetail {
        id,
        title: text(v.get("Title")),
        icon: media_url(&text(v.get("ImageIcon"))),
        points,
        num_achievements: num(v.get("NumAchievements")).max(achievements.len() as u64),
        awarded: num(v.get("NumAwardedToUser")),
        awarded_hardcore: num(v.get("NumAwardedToUserHardcore")),
        achievements,
    }
}

// ---------- Ficheros ----------

fn rom_path(system: &SystemRef, game_path: &str) -> PathBuf {
    let path = Path::new(game_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        system.rom_dir.join(strip_dot_slash(game_path))
    }
}

fn data_file(app: &tauri::AppHandle, name: &str) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join(name))
}

fn read_json<T: for<'de> Deserialize<'de>>(file: &Path) -> Option<T> {
    serde_json::from_str(&fs::read_to_string(file).ok()?).ok()
}

fn write_json<T: Serialize>(file: &Path, value: &T) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(file, serde_json::to_string(value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn list() -> ConsoleList {
        ConsoleList {
            fetched_at: 0,
            games: vec![
                ListGame { id: 1, title: "Alex Kidd in Miracle World".into(), num_achievements: 30, hashes: vec!["aaa".into()] },
                ListGame { id: 2, title: "Sonic the Hedgehog".into(), num_achievements: 25, hashes: vec!["bbb".into(), "ccc".into()] },
            ],
        }
    }

    #[test]
    fn por_huella_por_nombre_o_nada() {
        let l = list();
        assert_eq!(match_game(&l, Some("CCC"), "lo que sea"), ("hash", Some(2)));
        // Huella desconocida, pero el nombre (sin región) coincide: tiene logros, otra versión
        assert_eq!(match_game(&l, Some("zzz"), "Sonic The Hedgehog (Europe)"), ("name", Some(2)));
        assert_eq!(match_game(&l, None, "Alex Kidd in Miracle World [!]"), ("name", Some(1)));
        assert_eq!(match_game(&l, Some("zzz"), "Otro juego"), ("none", None));
    }

    #[test]
    fn numeros_como_texto_o_numero() {
        assert_eq!(num(Some(&json!("12"))), 12);
        assert_eq!(num(Some(&json!(7))), 7);
        assert_eq!(num(Some(&json!(true))), 1);
        assert_eq!(num(None), 0);
    }

    #[test]
    fn ficha_de_un_juego_con_progreso() {
        let v = json!({
            "Title": "Alex Kidd", "ImageIcon": "/Images/000001.png", "NumAchievements": 2,
            "NumAwardedToUser": "1", "NumAwardedToUserHardcore": 0,
            "Achievements": {
                "20": {"ID": 20, "Title": "B", "Description": "d", "Points": 10, "BadgeName": "222", "DisplayOrder": 2},
                "10": {"ID": 10, "Title": "A", "Description": "d", "Points": "5", "BadgeName": "111", "DisplayOrder": 1, "DateEarned": "2026-11-01 10:00:00"}
            }
        });
        let d = parse_game_detail(&v, 1);
        assert_eq!(d.icon, "https://media.retroachievements.org/Images/000001.png");
        assert_eq!((d.awarded, d.points, d.num_achievements), (1, 15, 2));
        assert_eq!(d.achievements[0].title, "A");
        assert_eq!(d.achievements[0].earned.as_deref(), Some("2026-11-01 10:00:00"));
        assert_eq!(d.achievements[1].badge_locked, "https://media.retroachievements.org/Badge/222_lock.png");
        assert_eq!(d.achievements[1].earned, None);
    }

    #[test]
    fn perfil_con_juegos_y_logros_recientes() {
        let v = json!({
            "User": "Fulano", "UserPic": "/UserPic/Fulano.png", "TotalPoints": 1200, "TotalTruePoints": "3000",
            "Rank": 500, "TotalRanked": 10000,
            "RecentlyPlayed": [{"GameID": 1, "Title": "Alex Kidd", "ConsoleName": "Master System", "ImageIcon": "/Images/1.png", "LastPlayed": "2026-11-10 20:00:00", "AchievementsTotal": 30}],
            "RecentAchievements": {"1": {
                "5": {"Title": "Viejo", "Points": 5, "BadgeName": "9", "GameTitle": "Alex Kidd", "DateAwarded": "2026-11-01 10:00:00", "HardcoreAchieved": 0},
                "6": {"Title": "Nuevo", "Points": 10, "BadgeName": "8", "GameTitle": "Alex Kidd", "DateAwarded": "2026-11-10 20:00:00", "HardcoreAchieved": 1}
            }}
        });
        let p = parse_profile(&v, "fulano");
        assert_eq!((p.user.as_str(), p.points, p.true_points, p.rank), ("Fulano", 1200, 3000, Some(500)));
        assert_eq!(p.user_pic, "https://media.retroachievements.org/UserPic/Fulano.png");
        assert_eq!(p.recent_games[0].console, "Master System");
        assert_eq!(p.recent_achievements[0].title, "Nuevo");
        assert!(p.recent_achievements[0].hardcore);
    }
}
