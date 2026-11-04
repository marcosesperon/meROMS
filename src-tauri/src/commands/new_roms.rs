//! «ROMs nuevas»: ficheros que están en la carpeta de un sistema pero no en su gamelist.
//!
//! 1. `list_new_roms` → las busca (también en subcarpetas) y propone un nombre limpio con las
//!    reglas de renombrado. No toca nada.
//! 2. `add_new_roms` → añade a la gamelist las que el usuario haya dejado marcadas, con el
//!    nombre que haya escrito.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::{Layout, SystemRef};
use crate::media;
use crate::models::system::extensions_for_system;
use crate::models::{AppSettings, Game};
use crate::rename_rules::CompiledRules;

/// Extensiones que seguro que no son ROMs: ni se ofrecen en «otras extensiones».
const NOT_ROM_EXTENSIONS: &[&str] = &[
    "xml", "txt", "nfo", "cfg", "ini", "json", "md", "pdf", "log", "db", "dat", "html", "htm",
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg",
    "mp4", "mkv", "avi", "mov", "webm", "mp3", "ogg", "wav",
    "srm", "sav", "state", "auto", "bak", "tmp",
    // Lo que guardan los emuladores: memoria, récords, tarjetas, trucos…
    "nv", "fs", "hi", "eep", "nvram", "rtc", "mcr", "cht",
];

/// Carpetas que nunca tienen ROMs: las de medios (nombres nuevos y antiguos) y alguna más.
const EXTRA_SKIPPED_DIRS: &[&str] = &[
    "media", "downloaded_media", "snaps", "boxart",
    // Carpetas donde guardan cosas los emuladores (FinalBurn Neo, MAME, RetroArch…)
    "fbneo", "nvram", "hiscore", "saves", "states", "cheats", "memcards",
];

#[derive(Debug, Clone, Serialize)]
pub struct NewRom {
    /// Ruta como irá en la gamelist: «./sub/Juego.sms»
    pub path: String,
    /// Nombre propuesto (el del fichero, limpio con las reglas)
    pub proposed_name: String,
    /// false = la extensión no está en la lista del sistema (se ofrece desmarcada)
    pub known_extension: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TrashResult {
    pub trashed: usize,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewRomEntry {
    pub path: String,
    pub name: String,
}

#[tauri::command]
pub fn list_new_roms(app: tauri::AppHandle, system_path: String) -> Result<Vec<NewRom>, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;
    let games = super::gamelist::read_gamelist(&layout, &system)?.games;
    let rules = CompiledRules::new(&settings.rename_rules_or_default());
    Ok(find_new_roms(&system, &games, &rules))
}

/// Juego de la gamelist cuya ROM ya no está en disco
#[derive(Debug, Clone, Serialize)]
pub struct MissingRom {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MissingRoms {
    pub games: Vec<MissingRom>,
    /// Faltan todas: lo más probable es que la carpeta o el disco no esté conectado
    pub all_missing: bool,
}

/// Juegos de la gamelist cuya ROM (fichero o carpeta) no existe.
#[tauri::command]
pub fn list_missing_roms(app: tauri::AppHandle, system_path: String) -> Result<MissingRoms, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let games = super::gamelist::read_gamelist(&layout, &system)?.games;
    Ok(find_missing(&system, &games))
}

fn find_missing(system: &SystemRef, games: &[Game]) -> MissingRoms {
    let missing: Vec<MissingRom> = games
        .iter()
        .filter(|g| !g.path.trim().is_empty() && !media::resolve_media_path(system, &g.path).exists())
        .map(|g| MissingRom { path: g.path.clone(), name: g.name.clone().unwrap_or_else(|| g.path.clone()) })
        .collect();
    MissingRoms { all_missing: !games.is_empty() && missing.len() == games.len(), games: missing }
}

#[tauri::command]
pub fn add_new_roms(app: tauri::AppHandle, system_path: String, entries: Vec<NewRomEntry>) -> Result<usize, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;
    let mut gamelist = super::gamelist::read_gamelist(&layout, &system)?;
    let added = add_to(&layout, &system, &settings, &mut gamelist.games, &entries);
    if added > 0 {
        // write_gamelist() hace la copia de seguridad antes de escribir
        super::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    }
    Ok(added)
}

/// Manda a la papelera ficheros de la carpeta del sistema (los que no son ROMs válidas).
/// Por seguridad solo acepta rutas que estén dentro de esa carpeta.
#[tauri::command]
pub fn trash_files(system_path: String, paths: Vec<String>) -> Result<TrashResult, String> {
    let system = SystemRef::from_rom_dir(&system_path);
    let (inside, mut failed) = split_inside(&system, &paths);
    // Todos de una vez (un solo sonido de papelera en macOS)
    let errors = media::trash_files(&inside);
    failed.extend(errors.iter().map(|(f, e)| format!("{} ({})", f.display(), e)));
    Ok(TrashResult { trashed: inside.len() - errors.len(), failed })
}

/// Separa las rutas que son ficheros dentro de la carpeta del sistema de las que no
/// (esas se devuelven ya como fallo, con el motivo).
fn split_inside(system: &SystemRef, paths: &[String]) -> (Vec<PathBuf>, Vec<String>) {
    let root = system.rom_dir.canonicalize().unwrap_or_else(|_| system.rom_dir.clone());
    let mut inside = Vec::new();
    let mut rejected = Vec::new();
    for path in paths {
        let file = media::resolve_media_path(system, path);
        match file.canonicalize() {
            Ok(real) if real.starts_with(&root) && real != root && real.is_file() => inside.push(real),
            _ => rejected.push(format!("{} (not a file inside the system folder)", path)),
        }
    }
    (inside, rejected)
}

/// Busca las ROMs de la carpeta del sistema que no tiene ningún juego de la gamelist.
fn find_new_roms(system: &SystemRef, games: &[Game], rules: &CompiledRules) -> Vec<NewRom> {
    // Lo que ya está en la gamelist, en ruta absoluta (así da igual «./x» o «x»)
    let in_gamelist: HashSet<PathBuf> = games.iter().map(|g| media::resolve_media_path(system, &g.path)).collect();
    let known: Vec<&str> = extensions_for_system(&system.name);
    let skipped_dirs = skipped_dirs();

    let mut files = Vec::new();
    collect_files(&system.rom_dir, &skipped_dirs, &mut files);
    // Los discos que ya usa un .m3u no son juegos sueltos
    let in_playlists = m3u_entries(&files);

    let mut found: Vec<NewRom> = files
        .iter()
        .filter(|f| !in_gamelist.contains(*f) && !in_playlists.contains(*f))
        .filter_map(|file| {
            let ext = file.extension()?.to_str()?.to_lowercase();
            if NOT_ROM_EXTENSIONS.contains(&ext.as_str()) || ext.starts_with("state") {
                return None;
            }
            let rel = file.strip_prefix(&system.rom_dir).ok()?;
            let stem = file.file_stem()?.to_string_lossy().to_string();
            let cleaned = rules.apply(&stem).trim().to_string();
            Some(NewRom {
                path: format!("./{}", rel.to_string_lossy().replace('\\', "/")),
                proposed_name: if cleaned.is_empty() { stem } else { cleaned },
                known_extension: known.contains(&format!(".{}", ext).as_str()),
            })
        })
        .collect();

    found.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    found
}

/// Nombres de carpeta (en minúsculas) que no se recorren.
fn skipped_dirs() -> HashSet<String> {
    let mut dirs: HashSet<String> = super::cleanup::MEDIA_DIRS
        .iter()
        .filter_map(|d| d.split('/').next())
        .map(str::to_lowercase)
        .collect();
    for (folders, _, _) in super::images::legacy_targets() {
        dirs.extend(folders.iter().map(|d| d.to_lowercase()));
    }
    dirs.extend(EXTRA_SKIPPED_DIRS.iter().map(|d| d.to_string()));
    dirs
}

/// Todos los ficheros visibles, bajando por las subcarpetas que no sean de medios ni ocultas.
fn collect_files(dir: &Path, skipped_dirs: &HashSet<String>, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if !skipped_dirs.contains(&name.to_lowercase()) {
                collect_files(&path, skipped_dirs, out);
            }
        } else if path.is_file() {
            out.push(path);
        }
    }
}

/// Ficheros a los que apunta algún .m3u (rutas relativas a la carpeta del .m3u).
fn m3u_entries(files: &[PathBuf]) -> HashSet<PathBuf> {
    let mut used = HashSet::new();
    for m3u in files.iter().filter(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("m3u"))) {
        let (Ok(content), Some(dir)) = (fs::read_to_string(m3u), m3u.parent()) else { continue };
        for line in content.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            used.insert(dir.join(line));
        }
    }
    used
}

/// Añade las entradas que aún no estén en la gamelist. En clásica enlaza los medios que ya
/// haya para esa ROM (ES-DE los encuentra solo por el nombre). Devuelve cuántas se añadieron.
fn add_to(layout: &Layout, system: &SystemRef, settings: &AppSettings, games: &mut Vec<Game>, entries: &[NewRomEntry]) -> usize {
    let mut present: HashSet<PathBuf> = games.iter().map(|g| media::resolve_media_path(system, &g.path)).collect();
    let mut added = 0;
    for entry in entries {
        if !present.insert(media::resolve_media_path(system, &entry.path)) {
            continue; // ya estaba (o venía repetida)
        }
        let name = entry.name.trim();
        let mut game = Game {
            path: entry.path.clone(),
            name: Some(if name.is_empty() { fallback_name(&entry.path) } else { name.to_string() }),
            ..Default::default()
        };
        if layout.writes_media_tags() {
            for (field, rel) in super::images::scan_legacy_media(settings, system, &entry.path) {
                media::set_field(&mut game, &field, Some(rel));
            }
        }
        games.push(game);
        added += 1;
    }
    added
}

fn fallback_name(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rename_rules::default_rules;

    fn touch(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn game(path: &str) -> Game {
        Game { path: path.into(), name: Some(path.into()), ..Default::default() }
    }

    fn rules() -> CompiledRules {
        CompiledRules::new(&default_rules())
    }

    #[test]
    fn encuentra_solo_las_que_faltan_tambien_en_subcarpetas() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("mastersystem"));
        touch(&system.rom_dir.join("Ya Esta.sms"), "");
        touch(&system.rom_dir.join("Nueva (Europe).sms"), "");
        touch(&system.rom_dir.join("Japonesas/Otra.sms"), "");

        let found = find_new_roms(&system, &[game("Ya Esta.sms")], &rules());
        let paths: Vec<_> = found.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, vec!["./Japonesas/Otra.sms", "./Nueva (Europe).sms"]);
        assert!(found.iter().all(|r| r.known_extension));
        // El nombre propuesto sale limpio con las reglas
        assert_eq!(found[1].proposed_name, "Nueva");
    }

    #[test]
    fn salta_medios_ficheros_que_no_son_roms_y_ocultos() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("mastersystem"));
        touch(&system.rom_dir.join("images/Juego-image.png"), "");
        touch(&system.rom_dir.join("covers/Juego.sms"), ""); // dentro de una carpeta de medios
        touch(&system.rom_dir.join(".backups/Juego.sms"), "");
        touch(&system.rom_dir.join("gamelist_backup.xml"), "");
        touch(&system.rom_dir.join("Juego.srm"), "");
        touch(&system.rom_dir.join(".DS_Store"), "");
        touch(&system.rom_dir.join("LEEME"), "");

        assert!(find_new_roms(&system, &[], &rules()).is_empty());
    }

    #[test]
    fn no_ofrece_lo_que_guardan_los_emuladores() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("cps1"));
        touch(&system.rom_dir.join("fbneo/avsp.nv"), "");
        touch(&system.rom_dir.join("fbneo/aliensyn.fs"), "");
        touch(&system.rom_dir.join("dino.nv"), "");
        touch(&system.rom_dir.join("dino.hi"), "");
        touch(&system.rom_dir.join("dino.zip"), "");

        let paths: Vec<_> = find_new_roms(&system, &[], &rules()).into_iter().map(|r| r.path).collect();
        assert_eq!(paths, vec!["./dino.zip"]);
    }

    #[test]
    fn solo_se_eliminan_ficheros_de_dentro_del_sistema() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("cps1"));
        touch(&system.rom_dir.join("basura.txt"), "");
        touch(&system.rom_dir.join("sub/otra.bin"), "");
        touch(&tmp.path().join("fuera.txt"), "");

        let (inside, rejected) = split_inside(
            &system,
            &[
                "./basura.txt".into(),
                "./sub/otra.bin".into(),
                "../fuera.txt".into(),
                tmp.path().join("fuera.txt").to_string_lossy().to_string(),
                "./no-existe.zip".into(),
                "./sub".into(),
            ],
        );
        assert_eq!(inside.len(), 2);
        assert_eq!(rejected.len(), 4);
    }

    #[test]
    fn otras_extensiones_salen_como_desconocidas() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("mastersystem"));
        touch(&system.rom_dir.join("Safari Race.sc"), "");

        let found = find_new_roms(&system, &[], &rules());
        assert_eq!(found.len(), 1);
        assert!(!found[0].known_extension);
    }

    #[test]
    fn los_discos_de_un_m3u_no_se_ofrecen_sueltos() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("psx"));
        touch(&system.rom_dir.join("Juego.m3u"), "Discos/Juego (Disc 1).cue\nDiscos/Juego (Disc 2).cue\n");
        touch(&system.rom_dir.join("Discos/Juego (Disc 1).cue"), "");
        touch(&system.rom_dir.join("Discos/Juego (Disc 2).cue"), "");

        let found = find_new_roms(&system, &[], &rules());
        let paths: Vec<_> = found.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, vec!["./Juego.m3u"]);
    }

    #[test]
    fn anadir_conserva_lo_que_habia_y_enlaza_medios_en_clasica() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("mastersystem"));
        touch(&system.rom_dir.join("images/Nueva-image.png"), "");
        let mut games = vec![game("./Vieja.sms")];
        let entries = vec![
            NewRomEntry { path: "./Nueva.sms".into(), name: "Nueva Bonita".into() },
            NewRomEntry { path: "./Vieja.sms".into(), name: "Repetida".into() },
            NewRomEntry { path: "./Sin Nombre.sms".into(), name: "  ".into() },
        ];

        let added = add_to(&Layout::Classic, &system, &AppSettings::default(), &mut games, &entries);
        assert_eq!(added, 2);
        assert_eq!(games.len(), 3);
        assert_eq!(games[0].name.as_deref(), Some("./Vieja.sms"));
        assert_eq!(games[1].name.as_deref(), Some("Nueva Bonita"));
        assert_eq!(games[1].image.as_deref(), Some("./images/Nueva-image.png"));
        assert_eq!(games[2].name.as_deref(), Some("Sin Nombre"));
    }

    #[test]
    fn missing_roms_are_entries_without_file_or_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("ps3"));
        touch(&system.rom_dir.join("Esta.sms"), "");
        fs::create_dir_all(system.rom_dir.join("Carpeta.ps3")).unwrap(); // ROM que es una carpeta
        let games = vec![game("./Esta.sms"), game("./Carpeta.ps3"), game("./Borrada.sms")];

        let result = find_missing(&system, &games);
        assert_eq!(result.games.len(), 1);
        assert_eq!(result.games[0].path, "./Borrada.sms");
        assert!(!result.all_missing);
    }

    #[test]
    fn all_missing_means_the_folder_is_probably_not_there() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("disco-desconectado"));
        let result = find_missing(&system, &[game("./A.sms"), game("./B.sms")]);
        assert_eq!(result.games.len(), 2);
        assert!(result.all_missing);
        assert!(!find_missing(&system, &[]).all_missing);
    }
}
