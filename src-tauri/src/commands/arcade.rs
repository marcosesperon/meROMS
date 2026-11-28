//! «Emulador por juego» (arcade): descarga el DAT de cada núcleo, diagnostica con cuál funciona
//! cada set (`arcade::diagnose`) y propone el emulador de los juegos cuyo emulador actual no
//! tiene el set completo. Aplicarlo escribe `<altemulator>` en el gamelist (con copia y Deshacer).
//!
//! Núcleos: FinalBurn Neo, MAME 2003-Plus y MAME 2010 (DAT de sus repositorios de libretro) y
//! MAME actual (el `-listxml` del MAME instalado o, si no, el `mameXXXXlx.zip` de MAME oficial).
//! Flycast (Naomi, Atomiswave) se compara con el de MAME actual: orientativo.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::arcade::{self, ArcadeIndex, SetCheck, SetFiles, SetState};
use crate::layout::{strip_dot_slash, SystemRef};

use super::progress::Progress;

/// Un núcleo que se comprueba
pub struct Core {
    pub id: &'static str,
    /// Etiquetas de ES-DE (es_systems.xml) que usan este romset
    pub labels: &'static [&'static str],
    /// De dónde se baja su DAT (MAME actual: aparte)
    url: Option<&'static str>,
    /// Versión de MAME si el DAT no la dice
    default_version: &'static str,
}

pub const CORES: &[Core] = &[
    Core {
        id: "fbneo",
        labels: &["FinalBurn Neo"],
        url: Some("https://raw.githubusercontent.com/libretro/FBNeo/master/dats/FinalBurn%20Neo%20(ClrMame%20Pro%20XML%2C%20Arcade%20only).dat"),
        default_version: "",
    },
    Core {
        id: "mame",
        labels: &["MAME - Current", "MAME (Standalone)", "Flycast", "Flycast (Standalone)", "Flycast Dojo (Standalone)"],
        url: None,
        default_version: "",
    },
    Core {
        id: "mame2010",
        labels: &["MAME 2010"],
        url: Some("https://raw.githubusercontent.com/libretro/mame2010-libretro/master/metadata/mame2010.xml"),
        default_version: "0.139",
    },
    Core {
        id: "mame2003plus",
        labels: &["MAME 2003-Plus"],
        url: Some("https://raw.githubusercontent.com/libretro/mame2003-plus-libretro/master/metadata/mame2003-plus.xml"),
        default_version: "0.78",
    },
];

/// Orden de preferencia si nadie lo ha cambiado (a igualdad de estado del driver)
pub const DEFAULT_ORDER: &[&str] = &["fbneo", "mame", "mame2010", "mame2003plus"];

fn core(id: &str) -> Option<&'static Core> {
    CORES.iter().find(|c| c.id == id)
}

/// El núcleo de una etiqueta de ES-DE
pub fn core_of_label(label: &str) -> Option<&'static str> {
    CORES.iter().find(|c| c.labels.contains(&label)).map(|c| c.id)
}

/// ¿Sistema arcade con estos núcleos? Los de `is_arcade_system` salvo Model 2/3 (otros emuladores)
pub fn is_checked_system(folder: &str) -> bool {
    super::scraper_adb::is_arcade_system(folder) && !matches!(folder.to_lowercase().as_str(), "model2" | "model3")
}

// ---------- DAT guardados ----------

fn dats_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?.join("arcade_dats");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    Ok(dir)
}

fn index_file(app: &tauri::AppHandle, id: &str) -> Result<PathBuf, String> {
    Ok(dats_dir(app)?.join(format!("{}.json", id)))
}

fn meta_file(app: &tauri::AppHandle, id: &str) -> Result<PathBuf, String> {
    Ok(dats_dir(app)?.join(format!("{}.meta.json", id)))
}

/// Lo que se enseña de un DAT guardado (sin leer el índice entero)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatMeta {
    pub version: String,
    /// libretro | mamedev | standalone
    pub source: String,
    /// Fecha de la descarga (RFC 3339)
    pub updated: String,
    pub games: usize,
}

/// Índices ya leídos, por núcleo, con la fecha del fichero (si cambia se vuelven a leer)
type LoadedIndexes = HashMap<String, (u64, Arc<ArcadeIndex>)>;
static LOADED: Mutex<Option<LoadedIndexes>> = Mutex::new(None);

fn modified(path: &Path) -> u64 {
    fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as u64)
}

/// El índice de un núcleo (None si no se ha descargado)
fn load_index(app: &tauri::AppHandle, id: &str) -> Result<Option<Arc<ArcadeIndex>>, String> {
    let file = index_file(app, id)?;
    if !file.is_file() {
        return Ok(None);
    }
    let stamp = modified(&file);
    if let Some((s, ix)) = LOADED.lock().ok().and_then(|m| m.as_ref().and_then(|m| m.get(id).cloned())) {
        if s == stamp {
            return Ok(Some(ix));
        }
    }
    let reader = BufReader::new(File::open(&file).map_err(|e| e.to_string())?);
    let index: ArcadeIndex = serde_json::from_reader(reader).map_err(|e| format!("Failed to read the saved DAT: {}", e))?;
    let index = Arc::new(index);
    if let Ok(mut m) = LOADED.lock() {
        m.get_or_insert_with(HashMap::new).insert(id.to_string(), (stamp, index.clone()));
    }
    Ok(Some(index))
}

/// Guarda el índice y su ficha (por un temporal: una descarga fallida no estropea la anterior)
fn save_index(app: &tauri::AppHandle, id: &str, mut index: ArcadeIndex, source: &str) -> Result<DatMeta, String> {
    if index.version.is_empty() {
        index.version = core(id).map(|c| c.default_version.to_string()).unwrap_or_default();
    }
    let meta = DatMeta {
        version: index.version.clone(),
        source: source.to_string(),
        updated: chrono::Local::now().to_rfc3339(),
        games: index.machines.values().filter(|m| !m.bios && !m.device).count(),
    };
    let file = index_file(app, id)?;
    let tmp = file.with_extension("json.tmp");
    let mut out = std::io::BufWriter::new(File::create(&tmp).map_err(|e| e.to_string())?);
    serde_json::to_writer(&mut out, &index).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    drop(out);
    fs::rename(&tmp, &file).map_err(|e| e.to_string())?;
    fs::write(meta_file(app, id)?, serde_json::to_vec(&meta).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(meta)
}

fn read_meta(app: &tauri::AppHandle, id: &str) -> Option<DatMeta> {
    if !index_file(app, id).ok()?.is_file() {
        return None;
    }
    serde_json::from_slice(&fs::read(meta_file(app, id).ok()?).ok()?).ok()
}

// ---------- MAME instalado ----------

fn mame_exe_name() -> &'static str {
    if cfg!(windows) {
        "mame.exe"
    } else {
        "mame"
    }
}

fn usual_mame_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/usr/games", "C:\\Program Files\\MAME", "C:\\MAME"].iter().map(PathBuf::from).collect();
    let home = crate::layout::esde_settings::home_dir();
    dirs.push(home.join("Applications").join("mame"));
    dirs.push(PathBuf::from("/Applications/mame"));
    dirs
}

/// MAME standalone: el elegido en Ajustes o el primero del PATH o de los sitios habituales
pub fn find_mame(configured: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = configured.map(str::trim).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let path_dirs = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
    path_dirs.into_iter().chain(usual_mame_dirs()).map(|d| d.join(mame_exe_name())).find(|p| p.is_file())
}

/// Un proceso de MAME sin ventana de consola en Windows
fn mame_command(exe: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// La versión de un MAME instalado (`mame -version` → «0.289 (mame0289)»)
fn mame_version(exe: &Path) -> Option<String> {
    let out = mame_command(exe).arg("-version").stdin(Stdio::null()).output().ok()?;
    String::from_utf8_lossy(&out.stdout).split_whitespace().next().map(String::from)
}

// ---------- Estado y descarga ----------

#[derive(Debug, Clone, Serialize)]
pub struct CoreStatus {
    pub id: String,
    pub dat: Option<DatMeta>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArcadeStatus {
    pub cores: Vec<CoreStatus>,
    /// Orden de preferencia (ids)
    pub order: Vec<String>,
    /// MAME standalone encontrado y su versión
    pub mame_path: Option<String>,
    pub mame_version: Option<String>,
    /// Versión de MAME elegida para el DAT de MAME actual (None = la última / la del instalado)
    pub mame_version_setting: Option<String>,
}

fn core_order(settings: &crate::models::settings::AppSettings) -> Vec<String> {
    let mut order: Vec<String> = settings.arcade_core_order.clone().unwrap_or_default().into_iter().filter(|id| core(id).is_some()).collect();
    // Los que falten, al final en el orden de siempre
    for id in DEFAULT_ORDER {
        if !order.iter().any(|o| o == id) {
            order.push(id.to_string());
        }
    }
    order
}

#[tauri::command(async)]
pub fn arcade_status(app: tauri::AppHandle) -> Result<ArcadeStatus, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let mame = find_mame(settings.mame_path.as_deref());
    Ok(ArcadeStatus {
        cores: CORES.iter().map(|c| CoreStatus { id: c.id.to_string(), dat: read_meta(&app, c.id) }).collect(),
        order: core_order(&settings),
        mame_version: mame.as_deref().and_then(mame_version),
        mame_path: mame.map(|p| p.to_string_lossy().to_string()),
        mame_version_setting: settings.arcade_mame_version.clone(),
    })
}

fn client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .user_agent(format!("meROMS/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

/// Versiones de MAME publicadas (para el selector): «0.289», «0.288»…
#[tauri::command]
pub async fn mame_versions() -> Result<Vec<String>, String> {
    let url = "https://api.github.com/repos/mamedev/mame/releases?per_page=40";
    let releases: Vec<serde_json::Value> = client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Could not ask GitHub: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Could not ask GitHub: {}", e))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(releases.iter().filter_map(|r| r["tag_name"].as_str()).filter_map(version_of_tag).collect())
}

/// «mame0289» → «0.289»
fn version_of_tag(tag: &str) -> Option<String> {
    let digits = tag.strip_prefix("mame")?;
    (digits.len() >= 4 && digits.chars().all(|c| c.is_ascii_digit())).then(|| format!("{}.{}", &digits[..1], &digits[1..]))
}

/// «0.289» → «mame0289»
fn tag_of_version(version: &str) -> String {
    format!("mame{}", version.replace('.', ""))
}

/// Descarga a un fichero con progreso (en MB) y cancelable
async fn download(url: &str, to: &Path, progress: &Progress) -> Result<(), String> {
    let mut response = client()?.get(url).send().await.map_err(|e| format!("Download failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }
    let total = response.content_length().unwrap_or(0);
    let mut file = File::create(to).map_err(|e| e.to_string())?;
    let mut done = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(|e| format!("Download failed: {}", e))? {
        progress.check()?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        progress.step("downloading", (done >> 20) as usize, (total >> 20) as usize, "MB");
    }
    Ok(())
}

/// Descarga (o saca del MAME instalado) el DAT de un núcleo y lo guarda resumido
#[tauri::command]
pub async fn arcade_download(app: tauri::AppHandle, core_id: String, task: Option<String>) -> Result<DatMeta, String> {
    let def = core(&core_id).ok_or("Unknown core")?;
    let progress = Progress::new(&app, task);
    let settings = super::settings::get_settings(app.clone())?;
    let tmp = dats_dir(&app)?.join(format!("{}.download", def.id));

    let result = if let Some(url) = def.url {
        download(url, &tmp, &progress).await?;
        progress.phase("readingDat");
        let (a, path) = (app.clone(), tmp.clone());
        tauri::async_runtime::spawn_blocking(move || {
            let index = arcade::parse_index(BufReader::with_capacity(1 << 20, File::open(&path).map_err(|e| e.to_string())?))?;
            save_index(&a, def.id, index, "libretro")
        })
        .await
        .map_err(|e| e.to_string())?
    } else {
        // MAME actual: el instalado manda, salvo que se haya elegido una versión a mano
        let installed = find_mame(settings.mame_path.as_deref()).filter(|_| settings.arcade_mame_version.is_none());
        if let Some(exe) = installed {
            progress.phase("readingDat");
            let a = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let mut child = mame_command(&exe).arg("-listxml").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| format!("Could not run MAME: {}", e))?;
                let out = child.stdout.take().ok_or("Could not run MAME")?;
                let index = arcade::parse_index(BufReader::with_capacity(1 << 20, out));
                let _ = child.wait();
                save_index(&a, "mame", index?, "standalone")
            })
            .await
            .map_err(|e| e.to_string())?
        } else {
            let version = match settings.arcade_mame_version.clone() {
                Some(v) => v,
                None => mame_versions().await?.into_iter().next().ok_or("No MAME release found")?,
            };
            let tag = tag_of_version(&version);
            let url = format!("https://github.com/mamedev/mame/releases/download/{tag}/{tag}lx.zip");
            download(&url, &tmp, &progress).await?;
            progress.phase("readingDat");
            let (a, path) = (app.clone(), tmp.clone());
            tauri::async_runtime::spawn_blocking(move || {
                let mut zip = zip::ZipArchive::new(File::open(&path).map_err(|e| e.to_string())?).map_err(|e| format!("Not a valid zip: {}", e))?;
                let entry = zip.by_index(0).map_err(|e| e.to_string())?;
                let index = arcade::parse_index(BufReader::with_capacity(1 << 20, entry))?;
                save_index(&a, "mame", index, "mamedev")
            })
            .await
            .map_err(|e| e.to_string())?
        }
    };
    let _ = fs::remove_file(&tmp);
    result
}

#[tauri::command(async)]
pub fn arcade_delete(app: tauri::AppHandle, core_id: String) -> Result<(), String> {
    for file in [index_file(&app, &core_id)?, meta_file(&app, &core_id)?] {
        if file.exists() {
            fs::remove_file(&file).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ---------- Diagnóstico ----------

/// Los zips de alrededor de un set: la carpeta del sistema y la de BIOS (por nombre, leídos una vez)
struct FolderFiles {
    dirs: Vec<PathBuf>,
    read: RefCell<HashMap<String, Option<HashSet<u32>>>>,
}

impl FolderFiles {
    fn new(dirs: Vec<PathBuf>) -> Self {
        FolderFiles { dirs, read: RefCell::new(HashMap::new()) }
    }
}

impl SetFiles for FolderFiles {
    fn crcs(&self, set_name: &str) -> Option<HashSet<u32>> {
        if let Some(known) = self.read.borrow().get(set_name) {
            return known.clone();
        }
        let found = self
            .dirs
            .iter()
            .flat_map(|d| ["zip", "7z"].map(|ext| d.join(format!("{}.{}", set_name, ext))))
            .find(|p| p.is_file())
            .and_then(|p| crate::rom_hash::archive_entries(&p))
            .map(|entries| entries.iter().filter_map(|e| u32::from_str_radix(&e.crc, 16).ok()).collect());
        self.read.borrow_mut().insert(set_name.to_string(), found.clone());
        found
    }

    fn has_chd(&self, folder: &str, disk: &str) -> bool {
        self.dirs.first().is_some_and(|d| d.join(folder).join(format!("{}.chd", disk)).is_file())
    }
}

/// Un juego del sistema: cómo está su set en cada núcleo y qué emulador se le propone
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeGame {
    pub path: String,
    pub name: Option<String>,
    /// La etiqueta que usaría Jugar ahora (None: comando propio o sistema que ES-DE no conoce)
    pub current: Option<String>,
    /// Su núcleo, si es uno de los comprobados
    pub current_core: Option<String>,
    /// Tiene `<altemulator>` propio
    pub own_emulator: bool,
    /// Por núcleo (solo los que tienen DAT)
    pub checks: HashMap<String, SetCheck>,
    /// La etiqueta propuesta y su núcleo (None: el actual vale, o ninguno lo tiene completo)
    pub proposal: Option<String>,
    pub proposal_core: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeReport {
    /// Núcleos con DAT, en el orden de preferencia
    pub cores: Vec<String>,
    /// Emuladores de ES-DE del sistema y si están en este equipo
    pub labels: Vec<(String, bool)>,
    /// El sistema tiene comando propio: no se propone nada
    pub custom: bool,
    pub games: Vec<ArcadeGame>,
}

fn driver_rank(driver: &str) -> u8 {
    match driver {
        "good" => 0,
        "imperfect" => 1,
        "preliminary" => 2,
        _ => 3,
    }
}

/// La etiqueta de un núcleo para este sistema: la primera de las suyas que tenga el sistema,
/// mejor si está en este equipo
fn label_for(core_id: &str, labels: &[(String, bool)]) -> Option<String> {
    let def = core(core_id)?;
    let has = |l: &&str, available: bool| labels.iter().any(|(x, a)| x == l && (*a || !available));
    def.labels.iter().find(|l| has(l, true)).or_else(|| def.labels.iter().find(|l| has(l, false))).map(|l| l.to_string())
}

/// El emulador que se propone: nada si el actual tiene el set completo (o no se puede saber);
/// si no, el núcleo con el set completo y mejor driver y, a igualdad, el primero del orden
pub fn propose(checks: &HashMap<String, SetCheck>, current_core: Option<&str>, order: &[String], labels: &[(String, bool)]) -> Option<(String, String)> {
    let current = current_core?;
    let state = &checks.get(current)?.state;
    if *state == SetState::Ok || *state == SetState::NotAGame {
        return None;
    }
    order
        .iter()
        .filter(|id| id.as_str() != current)
        .filter_map(|id| checks.get(id).filter(|c| c.state == SetState::Ok).map(|c| (id, c)))
        .min_by_key(|(id, c)| (driver_rank(&c.driver), order.iter().position(|o| o == *id)))
        .and_then(|(id, _)| label_for(id, labels).map(|l| (id.clone(), l)))
}

fn check_now(app: &tauri::AppHandle, system_path: &str, game_paths: Option<&[String]>, progress: &Progress) -> Result<ArcadeReport, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(app, system_path)?;
    let order = core_order(&settings);
    let mut indexes: Vec<(String, Arc<ArcadeIndex>)> = Vec::new();
    for id in &order {
        if let Some(ix) = load_index(app, id)? {
            indexes.push((id.clone(), ix));
        }
    }
    let choices = super::launch::emulator_choices(app, system_path)?;
    let labels: Vec<(String, bool)> = choices.options.iter().map(|o| (o.label.clone(), o.available)).collect();

    let mut dirs = vec![system.rom_dir.clone()];
    if let Some(bios) = settings.active_profile().and_then(super::bios::bios_dir) {
        dirs.push(bios);
    }
    let files = FolderFiles::new(dirs);

    let list = super::gamelist::read_gamelist_with(&layout, &system, progress)?;
    let wanted: Option<HashSet<&str>> = game_paths.map(|p| p.iter().map(String::as_str).collect());
    let games: Vec<_> = list.games.iter().filter(|g| wanted.as_ref().is_none_or(|w| w.contains(g.path.as_str()))).collect();
    let total = games.len();
    let mut out = Vec::new();
    for (i, game) in games.into_iter().enumerate() {
        progress.check()?;
        let rom = system.rom_dir.join(strip_dot_slash(&game.path));
        let stem = rom.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        progress.step("checking", i + 1, total, &stem);
        // Solo sets en zip/7z directamente en la carpeta del sistema
        if !crate::rom_hash::is_archive(&rom) || rom.parent() != Some(system.rom_dir.as_path()) {
            continue;
        }
        let checks: HashMap<String, SetCheck> = indexes.iter().map(|(id, ix)| (id.clone(), arcade::diagnose(ix, &stem, &files))).collect();
        let own = game.altemulator.as_deref().map(str::trim).filter(|a| !a.is_empty());
        let current = choices.chosen(own);
        let current_core = current.as_deref().and_then(core_of_label).map(String::from);
        let proposal = if choices.custom { None } else { propose(&checks, current_core.as_deref(), &order, &labels) };
        out.push(ArcadeGame {
            path: game.path.clone(),
            name: game.name.clone(),
            current,
            current_core,
            own_emulator: own.is_some(),
            checks,
            proposal_core: proposal.as_ref().map(|(c, _)| c.clone()),
            proposal: proposal.map(|(_, l)| l),
        });
    }
    Ok(ArcadeReport { cores: indexes.into_iter().map(|(id, _)| id).collect(), labels, custom: choices.custom, games: out })
}

/// Lo que cambia el diagnóstico sin tocar la carpeta del sistema: los DAT guardados y la carpeta de BIOS
fn outside_signature(app: &tauri::AppHandle) -> String {
    let mut parts: Vec<String> = CORES.iter().map(|c| index_file(app, c.id).map(|f| modified(&f)).unwrap_or(0).to_string()).collect();
    if let Some(bios) = super::settings::get_settings(app.clone()).ok().and_then(|s| s.active_profile().and_then(super::bios::bios_dir)) {
        parts.push(modified(&bios).to_string());
    }
    parts.join(",")
}

/// Diagnóstico de los juegos del sistema (o de unos cuantos: la ficha, Jugar). El del sistema
/// entero se guarda con el estado del sistema.
#[tauri::command(async)]
pub fn arcade_check(app: tauri::AppHandle, system_path: String, game_paths: Option<Vec<String>>, task: Option<String>) -> Result<ArcadeReport, String> {
    let progress = Progress::new(&app, task);
    if !is_checked_system(&SystemRef::from_rom_dir(&system_path).name) {
        return Ok(ArcadeReport { cores: Vec::new(), labels: Vec::new(), custom: false, games: Vec::new() });
    }
    if game_paths.is_some() {
        return check_now(&app, &system_path, game_paths.as_deref(), &progress);
    }
    let extra = outside_signature(&app);
    let (a, sp) = (app.clone(), system_path.clone());
    super::system_status::remember(&app, &system_path, "arcade", &extra, move || check_now(&a, &sp, None, &progress))
}

/// Un cambio de emulador: la etiqueta, o None para quitar el suyo (vuelve al del sistema)
#[derive(Debug, Clone, Deserialize)]
pub struct EmulatorChange {
    pub path: String,
    pub label: Option<String>,
}

/// Escribe `<altemulator>` de esos juegos (con copia del gamelist y Deshacer). Devuelve cuántos cambió.
#[tauri::command(async)]
pub fn apply_emulators(app: tauri::AppHandle, system_path: String, changes: Vec<EmulatorChange>) -> Result<usize, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let before = super::gamelist::read_gamelist(&layout, &system)?;
    let mut after = before.clone();
    let wanted: HashMap<&str, Option<&str>> = changes.iter().map(|c| (c.path.as_str(), c.label.as_deref().map(str::trim).filter(|l| !l.is_empty()))).collect();
    let mut undo = Vec::new();
    for (game, old) in after.games.iter_mut().zip(before.games.iter()) {
        let Some(label) = wanted.get(game.path.as_str()) else { continue };
        let label = label.map(String::from);
        if game.altemulator != label {
            game.altemulator = label;
            undo.push(super::undo::GameChange { before_sys: system_path.clone(), before: old.clone(), after_sys: system_path.clone(), after: game.clone() });
        }
    }
    if undo.is_empty() {
        return Ok(0);
    }
    super::gamelist::write_gamelist(&layout, &system, &after)?;
    let count = undo.len();
    super::undo::record(super::undo::UndoOp { kind: "emulator".into(), system: system_path, games: undo, ..Default::default() });
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(state: SetState, driver: &str) -> SetCheck {
        SetCheck { state, driver: driver.into(), bios: None, parent: None, devices: vec![], chd: vec![], bad_chips: 0, chips: 1, missing_chips: vec![] }
    }

    fn labels() -> Vec<(String, bool)> {
        ["MAME - Current", "MAME 2010", "MAME 2003-Plus", "MAME (Standalone)", "FinalBurn Neo", "Flycast"].iter().map(|l| (l.to_string(), *l != "MAME - Current")).collect()
    }

    fn order() -> Vec<String> {
        DEFAULT_ORDER.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn nothing_is_proposed_when_the_current_core_runs_it() {
        let checks: HashMap<String, SetCheck> = [("mame2003plus".to_string(), check(SetState::Ok, "good")), ("fbneo".to_string(), check(SetState::Ok, "good"))].into();
        assert_eq!(propose(&checks, Some("mame2003plus"), &order(), &labels()), None);
        // Sin núcleo conocido (comando propio, Geolith…) tampoco
        assert_eq!(propose(&checks, None, &order(), &labels()), None);
    }

    #[test]
    fn the_best_driver_wins_then_the_order() {
        let checks: HashMap<String, SetCheck> = [
            ("mame2003plus".to_string(), check(SetState::Wrong, "good")),
            ("fbneo".to_string(), check(SetState::Ok, "imperfect")),
            ("mame".to_string(), check(SetState::Ok, "good")),
            ("mame2010".to_string(), check(SetState::Ok, "good")),
        ]
        .into();
        // MAME actual y 2010 van «bien»; MAME actual va antes en el orden. Su etiqueta: la que
        // está en este equipo (MAME - Current no lo está)
        assert_eq!(propose(&checks, Some("mame2003plus"), &order(), &labels()), Some(("mame".into(), "MAME (Standalone)".into())));
        // Con FBNeo «bien» gana FBNeo (primero del orden)
        let mut checks = checks;
        checks.insert("fbneo".into(), check(SetState::Ok, "good"));
        assert_eq!(propose(&checks, Some("mame2003plus"), &order(), &labels()), Some(("fbneo".into(), "FinalBurn Neo".into())));
        // Si ninguno lo tiene completo, nada
        let none: HashMap<String, SetCheck> = [("mame2003plus".to_string(), check(SetState::Wrong, "good")), ("fbneo".to_string(), check(SetState::Needs, "good"))].into();
        assert_eq!(propose(&none, Some("mame2003plus"), &order(), &labels()), None);
    }

    #[test]
    fn labels_cores_and_versions() {
        assert_eq!(core_of_label("Flycast"), Some("mame"));
        assert_eq!(core_of_label("Geolith"), None);
        assert_eq!(version_of_tag("mame0289").as_deref(), Some("0.289"));
        assert_eq!(tag_of_version("0.289"), "mame0289");
        assert!(is_checked_system("naomi") && is_checked_system("Arcade") && !is_checked_system("model3") && !is_checked_system("snes"));
    }

    #[test]
    fn folder_files_read_each_zip_once_and_find_chds() {
        let tmp = tempfile::tempdir().unwrap();
        let rom_dir = tmp.path().join("arcade");
        let bios_dir = tmp.path().join("bios");
        fs::create_dir_all(rom_dir.join("kinst")).unwrap();
        fs::create_dir_all(&bios_dir).unwrap();
        let mut zip = zip::ZipWriter::new(File::create(bios_dir.join("neogeo.zip")).unwrap());
        zip.start_file("sfix.sfx", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"abc").unwrap();
        zip.finish().unwrap();
        fs::write(rom_dir.join("kinst").join("kinst.chd"), b"x").unwrap();

        let files = FolderFiles::new(vec![rom_dir, bios_dir]);
        let crcs = files.crcs("neogeo").unwrap();
        assert!(crcs.contains(&crc32fast::hash(b"abc")));
        assert!(files.crcs("nada").is_none());
        assert!(files.has_chd("kinst", "kinst") && !files.has_chd("kinst", "otro"));
    }
}

