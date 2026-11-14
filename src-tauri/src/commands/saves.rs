//! Datos guardados de un juego (pestaña «Datos guardados» de la ficha a pantalla completa):
//! partidas (`.srm`, `.sav`) y estados (`.state`, `.state1`…, con su miniatura `.png`) que
//! RetroArch y compañía guardan con el nombre de la ROM.
//!
//! Viven fuera de la carpeta de ROMs: la carpeta de saves del perfil (`saves_dir`) o, si no
//! tiene, la habitual de Batocera/RetroBat (`<ROMs>/../saves`, en ES-DE también junto a la
//! carpeta de ES-DE). Dentro se miran la subcarpeta del sistema y la raíz.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::layout::strip_dot_slash;
use crate::models::{Profile, StructureKind};

/// Extensiones de partida guardada (la memoria del cartucho)
const SAVE_EXTS: &[&str] = &["srm", "sav"];

#[derive(Debug, Clone, Serialize)]
pub struct SaveFile {
    pub path: String,
    pub name: String,
    /// "save" (partida) | "state" (estado, con su número de ranura si lo tiene)
    pub kind: String,
    /// Ranura del estado: 0 para `.state`, N para `.stateN`, -1 para `.state.auto`
    pub slot: Option<i32>,
    pub size: u64,
    /// Fecha de modificación (segundos unix)
    pub modified: i64,
    /// Miniatura del estado (`<fichero>.png`), si la hay
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameSaves {
    /// Carpeta de saves que se ha usado (None = el perfil no tiene y la habitual no existe)
    pub dir: Option<String>,
    pub files: Vec<SaveFile>,
}

/// Carpeta de saves del perfil: la elegida o, si no hay, la habitual que exista
pub fn saves_dir(profile: &Profile) -> Option<PathBuf> {
    if let Some(dir) = profile.saves_dir.as_deref().filter(|d| !d.trim().is_empty()) {
        return Some(PathBuf::from(dir));
    }
    usual_dirs(profile).into_iter().find(|d| d.is_dir())
}

/// Las carpetas habituales, en orden de preferencia
fn usual_dirs(profile: &Profile) -> Vec<PathBuf> {
    let next_to = |p: &str| Path::new(p).parent().map(|d| d.join("saves"));
    let mut out: Vec<PathBuf> = Vec::new();
    if profile.kind == StructureKind::Esde {
        out.extend(profile.esde_home_path.as_deref().and_then(next_to));
    }
    out.extend(next_to(&profile.rom_root_path));
    out
}

/// Carpeta que se propone en Ajustes si el perfil no tiene una
#[tauri::command(async)]
pub fn suggested_saves_dir(profile: Profile) -> Option<String> {
    let usual = usual_dirs(&profile);
    usual.iter().find(|d| d.is_dir()).or(usual.first()).map(|d| d.to_string_lossy().to_string())
}

/// Nombre de la ROM sin extensión (lo que comparten sus partidas y estados)
fn rom_stem(game_path: &str) -> String {
    let rel = strip_dot_slash(game_path);
    Path::new(rel).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// ¿Es `file_name` una partida o un estado de `stem`? Devuelve (tipo, ranura)
fn classify(file_name: &str, stem: &str) -> Option<(&'static str, Option<i32>)> {
    let rest = file_name.strip_prefix(stem)?.strip_prefix('.')?;
    let lower = rest.to_lowercase();
    if SAVE_EXTS.contains(&lower.as_str()) {
        return Some(("save", None));
    }
    if lower == "state" {
        return Some(("state", Some(0)));
    }
    if lower == "state.auto" {
        return Some(("state", Some(-1)));
    }
    let slot = lower.strip_prefix("state")?.parse::<i32>().ok()?;
    Some(("state", Some(slot)))
}

/// Las carpetas donde se miran los saves de un sistema: su subcarpeta y la raíz
fn search_dirs(dir: &Path, system_name: &str) -> Vec<PathBuf> {
    vec![dir.join(system_name), dir.to_path_buf()]
}

fn find_saves(dir: &Path, system_name: &str, game_path: &str) -> Vec<SaveFile> {
    let stem = rom_stem(game_path);
    let mut out = Vec::new();
    if stem.is_empty() {
        return out;
    }
    for folder in search_dirs(dir, system_name) {
        let Ok(entries) = fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let Some((kind, slot)) = classify(&name, &stem) else { continue };
            let meta = entry.metadata().ok();
            let png = PathBuf::from(format!("{}.png", path.to_string_lossy()));
            out.push(SaveFile {
                path: path.to_string_lossy().to_string(),
                name,
                kind: kind.into(),
                slot,
                size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                modified: meta
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
                thumbnail: png.is_file().then(|| png.to_string_lossy().to_string()),
            });
        }
    }
    // Partidas primero, después los estados por ranura
    out.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.slot.cmp(&b.slot)).then(a.name.cmp(&b.name)));
    out
}

/// La última partida guardada de un juego (para «Seguir jugando»)
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LastSave {
    /// Fecha del fichero más reciente, partida o estado (segundos unix)
    pub modified: i64,
    /// Miniatura del estado más reciente que tenga una
    pub thumbnail: Option<String>,
}

/// Lo último que se guardó de un juego, si hay algo
pub fn last_save(dir: &Path, system_name: &str, game_path: &str) -> Option<LastSave> {
    let files = find_saves(dir, system_name, game_path);
    let modified = files.iter().map(|f| f.modified).max()?;
    let thumbnail = files.iter().filter(|f| f.thumbnail.is_some()).max_by_key(|f| f.modified).and_then(|f| f.thumbnail.clone());
    Some(LastSave { modified, thumbnail })
}

/// Perfil activo y nombre del sistema (su carpeta)
fn context(app: &tauri::AppHandle, system_path: &str) -> Result<(Profile, String), String> {
    let settings = super::settings::get_settings(app.clone())?;
    let profile = settings.active_profile().cloned().ok_or("No active profile")?;
    let system = Path::new(system_path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    Ok((profile, system))
}

#[tauri::command(async)]
pub fn list_saves(app: tauri::AppHandle, system_path: String, game_path: String) -> Result<GameSaves, String> {
    let (profile, system) = context(&app, &system_path)?;
    let Some(dir) = saves_dir(&profile) else { return Ok(GameSaves { dir: None, files: Vec::new() }) };
    let files = find_saves(&dir, &system, &game_path);
    Ok(GameSaves { dir: Some(dir.to_string_lossy().to_string()), files })
}

/// ¿Está `file` dentro de la carpeta de saves? (nada de borrar fuera de ella)
fn inside(dir: &Path, file: &Path) -> bool {
    match (dir.canonicalize(), file.canonicalize()) {
        (Ok(d), Ok(f)) => f.starts_with(d),
        _ => false,
    }
}

/// Manda a la papelera una partida o un estado (y la miniatura del estado)
#[tauri::command(async)]
pub fn trash_save(app: tauri::AppHandle, file: String) -> Result<(), String> {
    let settings = super::settings::get_settings(app.clone())?;
    let profile = settings.active_profile().cloned().ok_or("No active profile")?;
    let dir = saves_dir(&profile).ok_or("No saves folder")?;
    let path = PathBuf::from(&file);
    if !inside(&dir, &path) {
        return Err(format!("Not in the saves folder: {}", file));
    }
    let png = PathBuf::from(format!("{}.png", file));
    let mut targets = vec![path];
    if png.is_file() {
        targets.push(png);
    }
    trash::delete_all(&targets).map_err(|e| e.to_string())
}

/// Copia de seguridad de todos los datos guardados del juego en un .zip dentro de `dest`.
/// Devuelve la ruta del .zip.
#[tauri::command(async)]
pub fn backup_saves(app: tauri::AppHandle, system_path: String, game_path: String, dest: String) -> Result<String, String> {
    let (profile, system) = context(&app, &system_path)?;
    let dir = saves_dir(&profile).ok_or("No saves folder")?;
    let files = find_saves(&dir, &system, &game_path);
    if files.is_empty() {
        return Err("No saves".into());
    }
    let date = chrono::Local::now().format("%Y-%m-%d %H%M%S");
    let zip_path = Path::new(&dest).join(format!("{} saves {}.zip", rom_stem(&game_path), date));
    write_zip(&zip_path, &files)?;
    Ok(zip_path.to_string_lossy().to_string())
}

fn write_zip(zip_path: &Path, files: &[SaveFile]) -> Result<(), String> {
    let out = fs::File::create(zip_path).map_err(|e| format!("{}: {}", zip_path.display(), e))?;
    let mut zip = zip::ZipWriter::new(out);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for f in files {
        let mut entries = vec![PathBuf::from(&f.path)];
        entries.extend(f.thumbnail.as_ref().map(PathBuf::from));
        for path in entries {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let bytes = fs::read(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Trae una partida o un estado de fuera con el nombre del juego (misma extensión). Va a la
/// subcarpeta del sistema si existe; si no, a la raíz. Nunca sobrescribe.
#[tauri::command(async)]
pub fn import_save(app: tauri::AppHandle, system_path: String, game_path: String, file: String) -> Result<String, String> {
    let (profile, system) = context(&app, &system_path)?;
    let dir = saves_dir(&profile).ok_or("No saves folder")?;
    import_into(&dir, &system, &game_path, Path::new(&file))
}

fn import_into(dir: &Path, system: &str, game_path: &str, src: &Path) -> Result<String, String> {
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    // Lo que va después del nombre: «.srm», «.state3», «.state.auto»…
    let tail = name.find('.').map(|i| name[i..].to_string()).unwrap_or_default();
    let stem = rom_stem(game_path);
    let new_name = format!("{}{}", stem, tail);
    if classify(&new_name, &stem).is_none() {
        return Err(format!("Not a save or state file: {}", name));
    }
    let system_dir = dir.join(system);
    let folder = if system_dir.is_dir() { system_dir } else { dir.to_path_buf() };
    let dest = folder.join(&new_name);
    if dest.exists() {
        return Err(format!("Already exists: {}", dest.display()));
    }
    fs::copy(src, &dest).map_err(|e| format!("{}: {}", dest.display(), e))?;
    Ok(dest.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconoce_partidas_y_estados() {
        let s = "Sonic (USA)";
        assert_eq!(classify("Sonic (USA).srm", s), Some(("save", None)));
        assert_eq!(classify("Sonic (USA).SAV", s), Some(("save", None)));
        assert_eq!(classify("Sonic (USA).state", s), Some(("state", Some(0))));
        assert_eq!(classify("Sonic (USA).state3", s), Some(("state", Some(3))));
        assert_eq!(classify("Sonic (USA).state.auto", s), Some(("state", Some(-1))));
        assert_eq!(classify("Sonic (USA).state3.png", s), None);
        assert_eq!(classify("Sonic (USA) 2.srm", s), None);
        assert_eq!(classify("Sonic (USA).zip", s), None);
    }

    #[test]
    fn ultima_partida_con_la_miniatura_mas_reciente() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("snes")).unwrap();
        let at = |name: &str, secs: u64| {
            let path = dir.join("snes").join(name);
            fs::write(&path, b"x").unwrap();
            let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
            fs::File::options().write(true).open(&path).unwrap().set_modified(time).unwrap();
        };
        assert_eq!(last_save(dir, "snes", "./Mario.sfc"), None);
        at("Mario.state1", 100);
        at("Mario.state1.png", 100);
        at("Mario.state2", 200);
        at("Mario.state2.png", 200);
        at("Mario.srm", 300);
        let last = last_save(dir, "snes", "./Mario.sfc").unwrap();
        assert_eq!(last.modified, 300);
        assert!(last.thumbnail.unwrap().ends_with("Mario.state2.png"));
    }

    #[test]
    fn busca_en_el_sistema_y_en_la_raiz() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("megadrive")).unwrap();
        fs::write(dir.join("megadrive/Sonic.srm"), b"save").unwrap();
        fs::write(dir.join("megadrive/Sonic.state1"), b"st").unwrap();
        fs::write(dir.join("megadrive/Sonic.state1.png"), b"png").unwrap();
        fs::write(dir.join("Sonic.state"), b"root").unwrap();
        fs::write(dir.join("megadrive/Sonic 2.srm"), b"otro").unwrap();

        let files = find_saves(dir, "megadrive", "./Sonic.md");
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["Sonic.srm", "Sonic.state", "Sonic.state1"]);
        assert!(files[2].thumbnail.is_some());
        assert!(files[1].thumbnail.is_none());
    }

    #[test]
    fn importa_con_el_nombre_del_juego_sin_pisar() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("saves");
        fs::create_dir_all(dir.join("snes")).unwrap();
        let src = tmp.path().join("descarga.srm");
        fs::write(&src, b"x").unwrap();
        let dest = import_into(&dir, "snes", "./Zelda (USA).sfc", &src).unwrap();
        assert!(dest.ends_with("snes/Zelda (USA).srm") || dest.ends_with("snes\\Zelda (USA).srm"));
        assert!(import_into(&dir, "snes", "./Zelda (USA).sfc", &src).is_err());
        let txt = tmp.path().join("notas.txt");
        fs::write(&txt, b"x").unwrap();
        assert!(import_into(&dir, "snes", "./Zelda (USA).sfc", &txt).is_err());
    }

    #[test]
    fn zip_con_las_miniaturas() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("A.state"), b"st").unwrap();
        fs::write(tmp.path().join("A.state.png"), b"png").unwrap();
        let files = find_saves(tmp.path(), "nada", "./A.bin");
        let zip_path = tmp.path().join("copia.zip");
        write_zip(&zip_path, &files).unwrap();
        let archive = zip::ZipArchive::new(fs::File::open(&zip_path).unwrap()).unwrap();
        let mut names: Vec<String> = archive.file_names().map(String::from).collect();
        names.sort();
        assert_eq!(names, vec!["A.state", "A.state.png"]);
    }
}
