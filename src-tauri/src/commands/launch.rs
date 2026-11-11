//! Jugar: lanzar un juego con su emulador (configuración de ES-DE o comando propio del sistema),
//! esperar a que se cierre y apuntar partidas, última vez y tiempo jugado en el gamelist.
//! Solo un juego a la vez.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::Emitter;

use crate::commands::gamelist::{read_gamelist, write_gamelist_no_backup};
use crate::commands::{settings, system_context};
use crate::launch::command::{prepare, Missing};
use crate::launch::config::LaunchConfig;
use crate::launch::find::FindContext;
use crate::launch::{choose_command, es_path_for, game_alt_emulator, system_alt_emulator, with_system_alt_emulator, Chosen};
use crate::layout::{strip_dot_slash, Layout, SystemRef};
use crate::models::profile::system_key;
use crate::models::Game;

/// Una partida más corta que esto no cuenta (se abrió y se cerró sin querer)
const MIN_PLAY_SECS: u64 = 10;

/// El juego abierto ahora (y la señal para cerrarlo)
struct Running {
    system_path: String,
    game_path: String,
    stop: Arc<AtomicBool>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

/// Cómo se lanzaría un juego: el comando final o qué falta
#[derive(Debug, Clone, Serialize)]
pub struct LaunchPlan {
    /// Etiqueta del emulador de ES-DE elegido, o `None` con el comando propio
    pub label: Option<String>,
    pub custom: bool,
    /// La línea de comandos final (si se puede lanzar)
    pub command: Option<String>,
    pub start_dir: Option<String>,
    /// Lo que falta (emulador o core)
    pub missing: Option<Missing>,
    /// ES-DE no conoce el sistema y no hay comando propio
    pub no_config: bool,
}

/// Un emulador de ES-DE para un sistema y si está en este equipo
#[derive(Debug, Clone, Serialize)]
pub struct EmulatorOption {
    pub label: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaunchOptions {
    /// ES-DE conoce el sistema (si no, solo hay comando propio)
    pub known: bool,
    pub options: Vec<EmulatorOption>,
    /// El elegido en el gamelist (`<alternativeEmulator>`), si hay
    pub current: Option<String>,
}

/// Lo que se avisa al cerrarse el emulador
#[derive(Debug, Clone, Serialize)]
struct GameExited {
    system_path: String,
    game_path: String,
    seconds: u64,
    /// La partida contó (duró lo bastante y se apuntó)
    counted: bool,
    /// El juego con las estadísticas nuevas
    game: Option<Game>,
    error: Option<String>,
}

/// Todo lo que hace falta para elegir y preparar el comando de un sistema
struct Context {
    layout: Layout,
    system: SystemRef,
    config: LaunchConfig,
    find: FindContext,
    custom: Option<String>,
    gamelist_xml: Option<String>,
}

fn context(app: &tauri::AppHandle, system_path: &str) -> Result<Context, String> {
    let (layout, system) = system_context(app, system_path)?;
    let settings = settings::get_settings(app.clone())?;
    let custom = settings
        .active_profile()
        .and_then(|p| p.system_views.as_ref())
        .and_then(|views| views.get(&system_key(&system.name)))
        .and_then(|view| view.launch_command.clone())
        .filter(|c| !c.trim().is_empty());
    // Lo personalizado de ES-DE: el de la carpeta del perfil o, en los clásicos, ~/ES-DE
    let esde_home = match &layout {
        Layout::Esde(esde) => esde.home.clone(),
        Layout::Classic => crate::layout::esde_settings::home_dir().join("ES-DE"),
    };
    let custom_dir = esde_home.join("custom_systems");
    let config = LaunchConfig::load(custom_dir.is_dir().then_some(custom_dir.as_path()))?;
    let find = FindContext::current(es_path_for(&esde_home));
    let gamelist_xml = std::fs::read_to_string(layout.gamelist_path(&system)).ok();
    Ok(Context { layout, system, config, find, custom, gamelist_xml })
}

/// Ruta absoluta de la ROM de un juego (las del gamelist son relativas a la carpeta del sistema)
fn rom_path(system: &SystemRef, game_path: &str) -> PathBuf {
    let path = Path::new(game_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        system.rom_dir.join(strip_dot_slash(game_path))
    }
}

fn plan(ctx: &Context, game_path: &str) -> LaunchPlan {
    let xml = ctx.gamelist_xml.as_deref().unwrap_or("");
    let chosen = choose_command(
        &ctx.config,
        &ctx.system.name,
        ctx.custom.as_deref(),
        game_alt_emulator(xml, game_path).as_deref(),
        system_alt_emulator(xml).as_deref(),
    );
    let Some(chosen) = chosen else {
        return LaunchPlan { label: None, custom: false, command: None, start_dir: None, missing: None, no_config: true };
    };
    let (label, custom) = match &chosen {
        Chosen::Custom(_) => (None, true),
        Chosen::Esde { label, .. } => (Some(label.clone()), false),
    };
    let rom = rom_path(&ctx.system, game_path);
    let rom_root = ctx.system.rom_dir.parent().unwrap_or(&ctx.system.rom_dir);
    match prepare(chosen.command(), &ctx.config.rules, &ctx.find, &rom, rom_root) {
        Ok(p) => LaunchPlan {
            label,
            custom,
            command: Some(p.command),
            start_dir: p.start_dir.map(|d| d.to_string_lossy().to_string()),
            missing: None,
            no_config: false,
        },
        Err(missing) => LaunchPlan { label, custom, command: None, start_dir: None, missing: Some(missing), no_config: false },
    }
}

/// Cómo se lanzaría un juego (el botón «Probar» y el aviso de lo que falta)
#[tauri::command(async)]
pub fn resolve_launch(app: tauri::AppHandle, system_path: String, game_path: String) -> Result<LaunchPlan, String> {
    Ok(plan(&context(&app, &system_path)?, &game_path))
}

/// Los emuladores de ES-DE para un sistema, cuáles hay en este equipo y el elegido
#[tauri::command(async)]
pub fn launch_options(app: tauri::AppHandle, system_path: String) -> Result<LaunchOptions, String> {
    let ctx = context(&app, &system_path)?;
    let Some(def) = ctx.config.system(&ctx.system.name) else {
        return Ok(LaunchOptions { known: false, options: Vec::new(), current: None });
    };
    // Con una ROM inventada: solo importa si se encuentran el emulador y el core
    let rom = ctx.system.rom_dir.join("game.rom");
    let rom_root = ctx.system.rom_dir.parent().unwrap_or(&ctx.system.rom_dir);
    let options = def
        .commands
        .iter()
        .map(|c| EmulatorOption {
            label: c.label.clone(),
            available: prepare(&c.command, &ctx.config.rules, &ctx.find, &rom, rom_root).is_ok(),
        })
        .collect();
    let current = ctx.gamelist_xml.as_deref().and_then(system_alt_emulator);
    Ok(LaunchOptions { known: true, options, current })
}

/// Elige el emulador de ES-DE del sistema (`None` = el predeterminado). Se guarda en el gamelist
/// (`<alternativeEmulator>`), donde lo lee ES-DE, con copia de seguridad antes.
#[tauri::command(async)]
pub fn set_system_emulator(app: tauri::AppHandle, system_path: String, label: Option<String>) -> Result<(), String> {
    let (layout, system) = system_context(&app, &system_path)?;
    let path = layout.gamelist_path(&system);
    let original = std::fs::read_to_string(&path).ok();
    let updated = with_system_alt_emulator(original.as_deref(), label.as_deref());
    if original.as_deref() == Some(updated.as_str()) {
        return Ok(());
    }
    if original.is_some() {
        crate::commands::backup::backup_gamelist(&layout, &system)?;
    }
    std::fs::create_dir_all(layout.gamelist_dir(&system)).map_err(|e| e.to_string())?;
    // Por un temporal, como el resto de escrituras del gamelist
    let tmp = path.with_extension("xml.tmp");
    std::fs::write(&tmp, updated).map_err(|e| format!("Failed to write gamelist: {}", e))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("Failed to write gamelist: {}", e))
}

/// El ejecutable de un emulador elegido a mano: en macOS, si es una aplicación (.app), el de
/// dentro (`Contents/MacOS/<CFBundleExecutable>`); si no, el mismo fichero
#[tauri::command(async)]
pub fn app_executable(path: String) -> String {
    let app = Path::new(&path);
    if !(app.is_dir() && path.ends_with(".app")) {
        return path;
    }
    let plist = std::fs::read_to_string(app.join("Contents/Info.plist")).unwrap_or_default();
    let name = bundle_executable(&plist)
        .or_else(|| app.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_default();
    app.join("Contents/MacOS").join(name).to_string_lossy().to_string()
}

/// `CFBundleExecutable` de un Info.plist en XML
fn bundle_executable(plist: &str) -> Option<String> {
    let after = &plist[plist.find("<key>CFBundleExecutable</key>")? ..];
    let start = after.find("<string>")? + "<string>".len();
    let end = after[start..].find("</string>")? + start;
    Some(after[start..end].trim().to_string()).filter(|s| !s.is_empty())
}

/// El juego que está abierto, si hay (ruta del sistema y del juego)
#[tauri::command]
pub fn running_game() -> Option<(String, String)> {
    RUNNING.lock().unwrap().as_ref().map(|r| (r.system_path.clone(), r.game_path.clone()))
}

/// Cierra el emulador del juego abierto (si se ha colgado o no responde)
#[tauri::command]
pub fn stop_game() {
    if let Some(r) = RUNNING.lock().unwrap().as_ref() {
        r.stop.store(true, Ordering::SeqCst);
    }
}

/// Lanza un juego y vuelve en cuanto el emulador ha arrancado. Al cerrarse se emite
/// `game-exited` (con el juego actualizado si la partida contó).
#[tauri::command(async)]
pub fn launch_game(app: tauri::AppHandle, system_path: String, game_path: String) -> Result<(), String> {
    let mut running = RUNNING.lock().unwrap();
    if running.is_some() {
        return Err("A game is already running".into());
    }
    let ctx = context(&app, &system_path)?;
    let plan = plan(&ctx, &game_path);
    let Some(command) = plan.command else {
        return Err(match plan.missing {
            Some(m) => format!("Not found: {}", m.name),
            None => "No emulator configured for this system".into(),
        });
    };
    let child = spawn(&command, plan.start_dir.as_deref().map(Path::new))?;

    let stop = Arc::new(AtomicBool::new(false));
    *running = Some(Running { system_path: system_path.clone(), game_path: game_path.clone(), stop: stop.clone() });
    drop(running);

    let (layout, system) = (ctx.layout, ctx.system);
    std::thread::spawn(move || {
        let started = Instant::now();
        let error = wait(child, &stop).err();
        let seconds = started.elapsed().as_secs();
        let counted = seconds >= MIN_PLAY_SECS;
        let game = if counted { record_play(&layout, &system, &game_path, seconds).ok() } else { None };
        *RUNNING.lock().unwrap() = None;
        let _ = app.emit("game-exited", GameExited { system_path, game_path, seconds, counted: game.is_some(), game, error });
    });
    Ok(())
}

/// Arranca el comando como ES-DE: con el shell en macOS y Linux (`exec`, así el proceso es el
/// emulador y se puede cerrar), y la línea tal cual en Windows
fn spawn(command: &str, start_dir: Option<&Path>) -> Result<Child, String> {
    let mut cmd = shell_command(command);
    if let Some(dir) = start_dir.filter(|d| d.is_dir()) {
        cmd.current_dir(dir);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().map_err(|e| format!("Failed to start the emulator: {}", e))
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> Command {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg(format!("exec {}", command));
    cmd
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    use std::os::windows::process::CommandExt;
    // El programa es el primer trozo (entre comillas si las lleva); el resto va tal cual
    let command = command.trim_start();
    let (program, rest) = if let Some(quoted) = command.strip_prefix('"') {
        match quoted.find('"') {
            Some(end) => (&quoted[..end], &quoted[end + 1..]),
            None => (quoted, ""),
        }
    } else {
        command.split_once(' ').unwrap_or((command, ""))
    };
    let mut cmd = Command::new(program);
    cmd.raw_arg(rest.trim_start());
    cmd
}

/// Espera a que el emulador se cierre (o lo cierra si se pide)
fn wait(mut child: Child, stop: &AtomicBool) -> Result<(), String> {
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Ok(());
        }
        if stop.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Apunta la partida: una más, la fecha de ahora y el tiempo sumado (sin copia de seguridad:
/// sería una por partida)
fn record_play(layout: &Layout, system: &SystemRef, game_path: &str, seconds: u64) -> Result<Game, String> {
    let mut gamelist = read_gamelist(layout, system)?;
    let game = gamelist.games.iter_mut().find(|g| g.path == game_path).ok_or("Game not in the gamelist")?;
    add_play(game, seconds, &chrono::Local::now().format("%Y%m%dT%H%M%S").to_string());
    let updated = game.clone();
    write_gamelist_no_backup(layout, system, &gamelist)?;
    Ok(updated)
}

fn add_play(game: &mut Game, seconds: u64, now: &str) {
    let number = |v: &Option<String>| v.as_deref().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0);
    game.playcount = Some((number(&game.playcount) + 1).to_string());
    game.gametime = Some((number(&game.gametime) + seconds).to_string());
    game.lastplayed = Some(now.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suma_la_partida() {
        let mut game = Game { path: "./a.sfc".into(), playcount: Some("2".into()), gametime: Some("100".into()), ..Default::default() };
        add_play(&mut game, 65, "20261111T101500");
        assert_eq!(game.playcount.as_deref(), Some("3"));
        assert_eq!(game.gametime.as_deref(), Some("165"));
        assert_eq!(game.lastplayed.as_deref(), Some("20261111T101500"));
        // Sin estadísticas antes
        let mut fresh = Game { path: "./b.sfc".into(), ..Default::default() };
        add_play(&mut fresh, 30, "20261111T101500");
        assert_eq!((fresh.playcount.as_deref(), fresh.gametime.as_deref()), (Some("1"), Some("30")));
    }

    #[test]
    fn ejecutable_de_una_app_de_macos() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("Mi Emu.app");
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::write(
            app.join("Contents/Info.plist"),
            "<plist><dict><key>CFBundleName</key><string>X</string><key>CFBundleExecutable</key>\n\t<string>miemu</string></dict></plist>",
        )
        .unwrap();
        let path = app.to_string_lossy().to_string();
        assert_eq!(app_executable(path), app.join("Contents/MacOS/miemu").to_string_lossy());
        // Un fichero normal se queda como está
        assert_eq!(app_executable("/usr/bin/emu".into()), "/usr/bin/emu");
    }

    #[test]
    fn rom_relativa_o_absoluta() {
        let system = SystemRef::from_rom_dir("/roms/snes");
        assert_eq!(rom_path(&system, "./a.sfc"), PathBuf::from("/roms/snes/a.sfc"));
        assert_eq!(rom_path(&system, "/otra/b.sfc"), PathBuf::from("/otra/b.sfc"));
    }

    #[cfg(not(windows))]
    #[test]
    fn lanza_espera_y_se_puede_cerrar() {
        // Un «emulador» que dura poco y otro que hay que cerrar
        let child = spawn("sleep 0.2", None).unwrap();
        assert!(wait(child, &AtomicBool::new(false)).is_ok());
        let child = spawn("sleep 30", None).unwrap();
        let stop = AtomicBool::new(true);
        let started = Instant::now();
        assert!(wait(child, &stop).is_ok());
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
