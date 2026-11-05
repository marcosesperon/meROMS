//! «Convertir a CHD»: pasa los juegos de disco (.cue con sus .bin, .gdi con sus pistas, .iso, y
//! los discos de un .m3u) a CHD con `chdman`, la herramienta de MAME. `chdman` no viene con la
//! app (es GPL): lo instala el usuario y aquí solo se busca. Cada CHD se comprueba con
//! `chdman verify` antes de que el juego pase a usarlo; si algo falla, el juego queda como estaba.

use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::Emitter;

use crate::layout::{strip_dot_slash, Layout, SystemRef};
use crate::media;
use crate::models::system::rom_extensions_map;

use super::move_games::{companion_files, sheet_names};

/// Extensiones de disco que se convierten
const DISC_EXTS: &[&str] = &["cue", "gdi", "iso"];

/// Sistemas cuyas ISO son de DVD (`createdvd` en lugar de `createcd`)
const DVD_SYSTEMS: &[&str] = &["ps2", "psp"];

/// Fichero temporal mientras se crea el CHD: si se corta a medias, no parece un CHD bueno
const TMP_SUFFIX: &str = ".meroms-tmp.chd";

// ---------- Encontrar chdman ----------

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "chdman.exe"
    } else {
        "chdman"
    }
}

/// Sitios donde suele estar aunque no esté en el PATH (Homebrew, paquetes de Linux, MAME en Windows)
fn usual_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![PathBuf::from(r"C:\Program Files\MAME"), PathBuf::from(r"C:\MAME"), PathBuf::from(r"C:\Program Files (x86)\MAME")]
    } else {
        vec![PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin"), PathBuf::from("/usr/bin"), PathBuf::from("/usr/games")]
    }
}

/// `chdman`: el elegido en Ajustes y, si no, el primero del PATH o de los sitios habituales
pub fn find_chdman(configured: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = configured.map(str::trim).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let path_dirs = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
    path_dirs.into_iter().chain(usual_dirs()).map(|d| d.join(exe_name())).find(|p| p.is_file())
}

/// Un proceso de chdman sin ventana de consola (en Windows abriría una por cada disco)
fn command(exe: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// La versión, de la primera línea que imprime sin argumentos
/// («chdman - MAME Compressed Hunks of Data (CHD) manager 0.264 (mame0264)»)
fn chdman_version(exe: &Path) -> Option<String> {
    let out = command(exe).stdin(Stdio::null()).output().ok()?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let line = text.lines().find(|l| l.to_lowercase().contains("chdman"))?;
    Some(line.rsplit(" manager ").next().unwrap_or(line).trim().to_string())
}

fn configured_path(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    Ok(super::settings::get_settings(app.clone())?.chdman_path)
}

#[derive(Debug, Clone, Serialize)]
pub struct ChdStatus {
    /// Ruta de chdman, si se encontró
    pub path: Option<String>,
    pub version: Option<String>,
}

#[tauri::command]
pub fn chd_status(app: tauri::AppHandle) -> Result<ChdStatus, String> {
    let exe = find_chdman(configured_path(&app)?.as_deref());
    let version = exe.as_deref().and_then(chdman_version);
    Ok(ChdStatus { path: exe.map(|p| p.to_string_lossy().to_string()), version })
}

// ---------- Qué se puede convertir ----------

/// ¿Acepta CHD el emulador habitual del sistema? Lo dice su tabla de extensiones.
pub fn supports_chd(system: &str) -> bool {
    rom_extensions_map().get(system.to_lowercase().as_str()).is_some_and(|exts| exts.contains(&".chd"))
}

fn is_disc(path: &Path) -> bool {
    media::extension_lower(path).is_some_and(|e| DISC_EXTS.contains(&e.as_str()))
}

/// Los discos que hay que convertir de una ROM: ella misma si es .cue/.gdi/.iso; los de un .m3u
/// (que ya sean .chd no hace falta). None si no hay nada que convertir o el .m3u lista otra cosa.
fn discs_of(rom: &Path) -> Option<Vec<PathBuf>> {
    if is_disc(rom) {
        return Some(vec![rom.to_path_buf()]);
    }
    if media::extension_lower(rom).as_deref() != Some("m3u") {
        return None;
    }
    let dir = rom.parent()?;
    let mut discs = Vec::new();
    for name in sheet_names(rom) {
        let disc = dir.join(&name);
        if is_disc(&disc) && disc.is_file() {
            discs.push(disc);
        } else if media::extension_lower(&disc).as_deref() != Some("chd") {
            return None; // un disco de otro tipo o que no está: mejor no tocarlo
        }
    }
    (!discs.is_empty()).then_some(discs)
}

fn chd_path(disc: &Path) -> PathBuf {
    disc.with_extension("chd")
}

/// Lo que ocupa un disco con sus ficheros (.bin, pistas)
fn disc_files(disc: &Path) -> Vec<PathBuf> {
    std::iter::once(disc.to_path_buf()).chain(companion_files(disc)).collect()
}

fn size_of(files: &[PathBuf]) -> u64 {
    files.iter().filter_map(|f| fs::metadata(f).ok()).map(|m| m.len()).sum()
}

#[derive(Debug, Clone, Serialize)]
pub struct ChdCandidate {
    pub path: String,
    pub name: String,
    /// Discos que se convierten (rutas relativas a la carpeta del sistema)
    pub discs: Vec<String>,
    pub size: u64,
    /// «exists» = ya hay un .chd con ese nombre (no se pisa)
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChdCandidates {
    pub supported: bool,
    pub games: Vec<ChdCandidate>,
}

fn candidates(layout: &Layout, system: &SystemRef) -> Result<Vec<ChdCandidate>, String> {
    let list = super::gamelist::read_gamelist(layout, system)?;
    let mut out = Vec::new();
    for game in &list.games {
        let rom = system.rom_dir.join(strip_dot_slash(&game.path));
        if !rom.is_file() {
            continue;
        }
        let Some(discs) = discs_of(&rom) else { continue };
        let files: Vec<PathBuf> = discs.iter().flat_map(|d| disc_files(d)).collect();
        out.push(ChdCandidate {
            path: game.path.clone(),
            name: game.name.clone().unwrap_or_else(|| game.path.clone()),
            discs: discs.iter().filter_map(|d| d.strip_prefix(&system.rom_dir).ok()).map(|d| d.to_string_lossy().replace('\\', "/")).collect(),
            size: size_of(&files),
            blocked: discs.iter().any(|d| chd_path(d).exists()).then(|| "exists".to_string()),
        });
    }
    out.sort_by_key(|c| c.name.to_lowercase());
    Ok(out)
}

#[tauri::command]
pub fn list_chd_candidates(app: tauri::AppHandle, system_path: String) -> Result<ChdCandidates, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    if !supports_chd(&system.name) {
        return Ok(ChdCandidates { supported: false, games: Vec::new() });
    }
    Ok(ChdCandidates { supported: true, games: candidates(&layout, &system)? })
}

// ---------- Convertir ----------

static CANCEL: AtomicBool = AtomicBool::new(false);

/// Para la conversión: corta el chdman que esté en marcha (su CHD a medias se borra)
#[tauri::command]
pub fn cancel_chd() {
    CANCEL.store(true, Ordering::SeqCst);
}

const STOPPED: &str = "stopped";

/// El porcentaje de una línea de chdman («Compressing, 45.3% complete... (ratio=60.1%)»)
fn parse_percent(line: &str) -> Option<f32> {
    let end = line.find("% complete")?;
    let start = line[..end].rfind(|c: char| !(c.is_ascii_digit() || c == '.')).map_or(0, |i| i + 1);
    line[start..end].parse().ok()
}

/// Lanza chdman y va pasando el porcentaje. Error = sus últimas líneas (o «stopped» si se paró).
fn run_chdman(exe: &Path, args: &[OsString], on_percent: &mut dyn FnMut(f32)) -> Result<(), String> {
    let mut child = command(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("chdman: {}", e))?;
    let mut stderr = child.stderr.take().ok_or("chdman: no output")?;

    // chdman reescribe la línea del progreso con \r: se separa por \r y \n
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut buf = [0u8; 4096];
    loop {
        if CANCEL.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(STOPPED.into());
        }
        let n = match stderr.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for c in String::from_utf8_lossy(&buf[..n]).chars() {
            if c == '\r' || c == '\n' {
                if let Some(p) = parse_percent(&current) {
                    on_percent(p);
                } else if !current.trim().is_empty() {
                    lines.push(current.trim().to_string());
                }
                current.clear();
            } else {
                current.push(c);
            }
        }
    }
    if !current.trim().is_empty() {
        lines.push(current.trim().to_string());
    }
    let status = child.wait().map_err(|e| format!("chdman: {}", e))?;
    if status.success() {
        return Ok(());
    }
    let tail: Vec<String> = lines.iter().rev().take(3).rev().cloned().collect();
    Err(if tail.is_empty() { format!("chdman failed ({})", status) } else { tail.join(" · ") })
}

fn tmp_path(disc: &Path) -> PathBuf {
    let stem = disc.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    disc.with_file_name(format!("{}{}", stem, TMP_SUFFIX))
}

/// createdvd para las ISO de PS2/PSP; createcd para todo lo demás
fn create_verb(system: &str, disc: &Path) -> &'static str {
    let iso = media::extension_lower(disc).as_deref() == Some("iso");
    if iso && DVD_SYSTEMS.contains(&system.to_lowercase().as_str()) {
        "createdvd"
    } else {
        "createcd"
    }
}

/// En qué va la conversión (evento `chd-progress`)
#[derive(Debug, Clone, Serialize)]
pub struct ChdProgress {
    pub game: usize,
    pub total: usize,
    pub name: String,
    pub disc: usize,
    pub discs: usize,
    /// «convert» | «verify»
    pub step: String,
    pub percent: f32,
}

/// Convierte los discos de un juego y los comprueba. Si todo va bien, deja cada CHD con su
/// nombre final y devuelve (disco, chd); si no, borra lo creado y no toca nada más.
fn convert_discs(exe: &Path, system: &str, discs: &[PathBuf], report: &mut dyn FnMut(usize, &str, f32)) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut made: Vec<PathBuf> = Vec::new();
    let result = (|| {
        for (i, disc) in discs.iter().enumerate() {
            let tmp = tmp_path(disc);
            let _ = fs::remove_file(&tmp); // uno viejo de una vez que se cortó
            made.push(tmp.clone());
            let create = [create_verb(system, disc).into(), "-i".into(), disc.as_os_str().to_owned(), "-o".into(), tmp.as_os_str().to_owned()];
            run_chdman(exe, &create, &mut |p| report(i, "convert", p))?;
            run_chdman(exe, &["verify".into(), "-i".into(), tmp.as_os_str().to_owned()], &mut |p| report(i, "verify", p))?;
        }
        // Todos bien: a su nombre final (sin pisar nada)
        let mut done = Vec::new();
        for (disc, tmp) in discs.iter().zip(&made) {
            let chd = chd_path(disc);
            if chd.exists() {
                return Err(format!("Already exists: {}", chd.display()));
            }
            fs::rename(tmp, &chd).map_err(|e| format!("{} ({})", chd.display(), e))?;
            done.push((disc.clone(), chd));
        }
        Ok(done)
    })();
    if result.is_err() {
        for tmp in &made {
            let _ = fs::remove_file(tmp);
        }
    }
    result
}

/// Qué se hace con los ficheros originales de un juego convertido
#[derive(Debug, Clone, PartialEq)]
pub enum Originals {
    Trash,
    Keep,
    /// Movidos a esta subcarpeta de la del sistema, con su posición relativa
    Subfolder(String),
}

impl Originals {
    fn parse(mode: &str, subfolder: Option<String>) -> Result<Self, String> {
        match mode {
            "trash" => Ok(Originals::Trash),
            "keep" => Ok(Originals::Keep),
            "subfolder" => {
                let sub = subfolder.unwrap_or_default().trim().trim_matches(['/', '\\']).to_string();
                let bad = sub.is_empty() || Path::new(&sub).is_absolute() || sub.split(['/', '\\']).any(|p| p == "..");
                if bad {
                    return Err(format!("Invalid subfolder: {}", sub));
                }
                Ok(Originals::Subfolder(sub))
            }
            other => Err(format!("Unknown option: {}", other)),
        }
    }
}

/// Aparta los originales según lo elegido. Devuelve los que no se pudieron.
fn handle_originals(system: &SystemRef, files: &[PathBuf], originals: &Originals) -> Vec<String> {
    match originals {
        Originals::Keep => Vec::new(),
        Originals::Trash => media::trash_files(files).into_iter().map(|(f, e)| format!("{} ({})", f.display(), e)).collect(),
        Originals::Subfolder(sub) => files
            .iter()
            .filter_map(|f| {
                let rel = f.strip_prefix(&system.rom_dir).ok()?;
                media::move_file(f, &system.rom_dir.join(sub).join(rel)).err()
            })
            .collect(),
    }
}

/// «./Juego.cue» → «./Juego.chd» (respeta el «./» y las subcarpetas de la ruta)
fn with_chd_ext(rom_path: &str) -> String {
    match rom_path.rfind('.') {
        Some(dot) if !rom_path[dot..].contains('/') => format!("{}.chd", &rom_path[..dot]),
        _ => format!("{}.chd", rom_path),
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ChdResult {
    pub converted: usize,
    /// Juegos saltados porque ya había un .chd con ese nombre
    pub skipped: Vec<String>,
    pub failed: Vec<String>,
    /// Originales que no se pudieron mandar a la papelera o mover
    pub originals_failed: Vec<String>,
    /// Lo que ocupaban los discos convertidos y lo que ocupan sus CHD
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub stopped: bool,
}

/// Convierte los juegos de un sistema. La gamelist se guarda después de cada juego (la primera
/// vez con copia de seguridad), así un corte nunca deja un juego apuntando a lo que ya no está.
fn convert_games(
    exe: &Path,
    layout: &Layout,
    system: &SystemRef,
    game_paths: &[String],
    originals: &Originals,
    emit: &mut dyn FnMut(ChdProgress),
) -> Result<ChdResult, String> {
    let mut list = super::gamelist::read_gamelist(layout, system)?;
    let mut result = ChdResult::default();
    let mut backed_up = false;

    for (i, game_path) in game_paths.iter().enumerate() {
        if CANCEL.load(Ordering::SeqCst) {
            result.stopped = true;
            break;
        }
        let Some(game) = list.games.iter().find(|g| &g.path == game_path) else { continue };
        let name = game.name.clone().unwrap_or_else(|| game_path.clone());
        let rom = system.rom_dir.join(strip_dot_slash(game_path));
        let Some(discs) = discs_of(&rom) else { continue };
        if discs.iter().any(|d| chd_path(d).exists()) {
            result.skipped.push(name);
            continue;
        }

        let total = game_paths.len();
        let mut report = |disc: usize, step: &str, percent: f32| {
            emit(ChdProgress { game: i + 1, total, name: name.clone(), disc: disc + 1, discs: discs.len(), step: step.into(), percent });
        };
        report(0, "convert", 0.0);
        let done = match convert_discs(exe, &system.name, &discs, &mut report) {
            Ok(done) => done,
            Err(e) if e == STOPPED => {
                result.stopped = true;
                break;
            }
            Err(e) => {
                result.failed.push(format!("{}: {}", name, e));
                continue;
            }
        };

        // El juego (y cada disco que tenga su propia entrada) pasa a su .chd
        let renamed: Vec<(String, String)> = done
            .iter()
            .filter_map(|(disc, _)| disc.strip_prefix(&system.rom_dir).ok())
            .map(|rel| {
                let old = format!("./{}", rel.to_string_lossy().replace('\\', "/"));
                (old.clone(), with_chd_ext(&old))
            })
            .collect();
        for g in &mut list.games {
            if let Some((_, new)) = renamed.iter().find(|(old, _)| strip_dot_slash(old) == strip_dot_slash(&g.path)) {
                g.path = new.clone();
            }
        }
        super::rename::update_references(layout, system, &renamed);
        let write = if backed_up { super::gamelist::write_gamelist_no_backup(layout, system, &list) } else { super::gamelist::write_gamelist(layout, system, &list) };
        if let Err(e) = write {
            result.failed.push(format!("{}: {}", name, e));
            break; // sin gamelist guardada no se apartan los originales
        }
        backed_up = true;

        let files: Vec<PathBuf> = discs.iter().flat_map(|d| disc_files(d)).collect();
        result.bytes_before += size_of(&files);
        result.bytes_after += size_of(&done.iter().map(|(_, chd)| chd.clone()).collect::<Vec<_>>());
        result.originals_failed.extend(handle_originals(system, &files, originals));
        result.converted += 1;
    }
    Ok(result)
}

/// Convierte los juegos elegidos. Emite `chd-progress`; `cancel_chd` lo para.
#[tauri::command]
pub async fn convert_to_chd(app: tauri::AppHandle, system_path: String, game_paths: Vec<String>, originals: String, subfolder: Option<String>) -> Result<ChdResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        CANCEL.store(false, Ordering::SeqCst);
        let originals = Originals::parse(&originals, subfolder)?;
        let exe = find_chdman(configured_path(&app)?.as_deref()).ok_or("chdman not found")?;
        let (layout, system) = super::system_context(&app, &system_path)?;
        if !supports_chd(&system.name) {
            return Err(format!("{} doesn't use CHD", system.name));
        }
        convert_games(&exe, &layout, &system, &game_paths, &originals, &mut |p| {
            let _ = app.emit("chd-progress", p);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Game, GameList};

    fn touch(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn write_list(system: &SystemRef, games: Vec<Game>) {
        super::super::gamelist::write_gamelist_no_backup(&Layout::Classic, system, &GameList { games, ..Default::default() }).unwrap();
    }

    fn game(path: &str) -> Game {
        Game { path: path.into(), name: Some(path.into()), ..Default::default() }
    }

    #[test]
    fn systems_and_verbs() {
        assert!(supports_chd("psx"));
        assert!(supports_chd("Dreamcast"));
        assert!(!supports_chd("snes"));
        assert!(!supports_chd("unknown-system"));
        assert_eq!(create_verb("ps2", Path::new("a.iso")), "createdvd");
        assert_eq!(create_verb("psx", Path::new("a.iso")), "createcd");
        assert_eq!(create_verb("ps2", Path::new("a.cue")), "createcd");
    }

    #[test]
    fn percent_and_paths() {
        assert_eq!(parse_percent("Compressing, 45.3% complete... (ratio=60.1%)"), Some(45.3));
        assert_eq!(parse_percent("Verifying, 100% complete..."), Some(100.0));
        assert_eq!(parse_percent("Output CHD:   x.chd"), None);
        assert_eq!(with_chd_ext("./sub/Juego (USA).cue"), "./sub/Juego (USA).chd");
        assert_eq!(with_chd_ext("./v1.0/Juego"), "./v1.0/Juego.chd");
        assert!(Originals::parse("subfolder", Some("../x".into())).is_err());
        assert_eq!(Originals::parse("subfolder", Some(".originales/".into())).unwrap(), Originals::Subfolder(".originales".into()));
    }

    #[test]
    fn candidates_cue_gdi_iso_m3u_and_existing_chd() {
        let tmp = tempfile::tempdir().unwrap();
        let sys = SystemRef::from_rom_dir(tmp.path().join("psx"));
        touch(&sys.rom_dir.join("A.cue"), "FILE \"A.bin\" BINARY\n");
        touch(&sys.rom_dir.join("A.bin"), "1234");
        touch(&sys.rom_dir.join("B.iso"), "iso");
        touch(&sys.rom_dir.join("B.chd"), "old");
        touch(&sys.rom_dir.join("C.m3u"), "C1.cue\nC2.chd\n");
        touch(&sys.rom_dir.join("C1.cue"), "FILE \"C1.bin\" BINARY\n");
        touch(&sys.rom_dir.join("C1.bin"), "x");
        touch(&sys.rom_dir.join("C2.chd"), "y");
        touch(&sys.rom_dir.join("D.pbp"), "z");
        write_list(&sys, vec![game("./A.cue"), game("./B.iso"), game("./C.m3u"), game("./D.pbp")]);

        let found = candidates(&Layout::Classic, &sys).unwrap();
        let summary: Vec<(&str, Vec<&str>, Option<&str>)> = found.iter().map(|c| (c.path.as_str(), c.discs.iter().map(String::as_str).collect(), c.blocked.as_deref())).collect();
        assert_eq!(summary, vec![("./A.cue", vec!["A.cue"], None), ("./B.iso", vec!["B.iso"], Some("exists")), ("./C.m3u", vec!["C1.cue"], None)]);
        assert_eq!(found[0].size, (b"FILE \"A.bin\" BINARY\n".len() + 4) as u64);
    }

    /// Un chdman de mentira: «crea» copiando la entrada y falla con los discos que se llaman «Malo»
    #[cfg(unix)]
    fn fake_chdman(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let exe = dir.join("chdman");
        let script = r#"#!/bin/sh
case "$1" in
  createcd|createdvd)
    case "$3" in *Malo*) echo "Error: bad image" >&2; exit 1;; esac
    printf 'Compressing, 50.0%% complete...\r' >&2
    cp "$3" "$5"; exit 0;;
  verify) exit 0;;
esac
exit 1
"#;
        fs::write(&exe, script).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        exe
    }

    #[cfg(unix)]
    #[test]
    fn converting_updates_gamelist_m3u_and_trashes_nothing_when_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_chdman(tmp.path());
        let sys = SystemRef::from_rom_dir(tmp.path().join("psx"));
        touch(&sys.rom_dir.join("A.cue"), "FILE \"A.bin\" BINARY\n");
        touch(&sys.rom_dir.join("A.bin"), "1234");
        touch(&sys.rom_dir.join("C.m3u"), "C1.cue\n");
        touch(&sys.rom_dir.join("C1.cue"), "FILE \"C1.bin\" BINARY\n");
        touch(&sys.rom_dir.join("C1.bin"), "x");
        write_list(&sys, vec![game("./A.cue"), game("./C.m3u"), game("./C1.cue")]);

        let mut percents = Vec::new();
        let r = convert_games(&exe, &Layout::Classic, &sys, &["./A.cue".into(), "./C.m3u".into()], &Originals::Subfolder(".originales".into()), &mut |p| percents.push(p.percent)).unwrap();
        assert_eq!((r.converted, r.failed.len()), (2, 0), "{:?}", r.failed);
        assert!(percents.contains(&50.0));

        let list = super::super::gamelist::read_gamelist(&Layout::Classic, &sys).unwrap();
        let paths: Vec<&str> = list.games.iter().map(|g| g.path.as_str()).collect();
        assert_eq!(paths, vec!["./A.chd", "./C.m3u", "./C1.chd"], "a disc with its own entry also moves to its CHD");
        assert_eq!(fs::read_to_string(sys.rom_dir.join("C.m3u")).unwrap().trim(), "C1.chd");
        assert!(sys.rom_dir.join("A.chd").is_file());
        assert!(!sys.rom_dir.join("A.bin").exists());
        assert!(sys.rom_dir.join(".originales/A.bin").is_file());
        assert!(sys.rom_dir.join(".originales/C1.cue").is_file());
        assert!(!sys.rom_dir.join(format!("A{}", TMP_SUFFIX)).exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_disc_leaves_the_game_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_chdman(tmp.path());
        let sys = SystemRef::from_rom_dir(tmp.path().join("saturn"));
        touch(&sys.rom_dir.join("J.m3u"), "J1.cue\nMalo.cue\n");
        touch(&sys.rom_dir.join("J1.cue"), "FILE \"J1.bin\" BINARY\n");
        touch(&sys.rom_dir.join("J1.bin"), "x");
        touch(&sys.rom_dir.join("Malo.cue"), "FILE \"Malo.bin\" BINARY\n");
        touch(&sys.rom_dir.join("Malo.bin"), "y");
        write_list(&sys, vec![game("./J.m3u")]);

        let r = convert_games(&exe, &Layout::Classic, &sys, &["./J.m3u".into()], &Originals::Keep, &mut |_| {}).unwrap();
        assert_eq!(r.converted, 0);
        assert!(r.failed[0].contains("bad image"), "{:?}", r.failed);
        assert!(!sys.rom_dir.join("J1.chd").exists(), "the first disc's CHD is removed too");
        assert!(!sys.rom_dir.join(format!("J1{}", TMP_SUFFIX)).exists());
        assert_eq!(fs::read_to_string(sys.rom_dir.join("J.m3u")).unwrap(), "J1.cue\nMalo.cue\n");
    }
}
