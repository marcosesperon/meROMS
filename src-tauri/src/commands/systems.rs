use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::progress::Progress;

use crate::layout::SystemRef;
use crate::models::system::{extensions_for_system, system_full_name, SystemInfo};
use crate::parser;

/// Count ROM files in a directory based on the system's known extensions.
fn count_roms(dir: &Path, folder_name: &str) -> usize {
    let extensions = extensions_for_system(folder_name);
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };

    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            // The listing already says what each entry is: no extra disk query per file (on an SD
            // card that was most of the time). Only a symlink is followed to see what it points to.
            let is_file = match e.file_type() {
                Ok(t) if t.is_symlink() => e.path().is_file(),
                Ok(t) => t.is_file(),
                Err(_) => false,
            };
            if !is_file {
                return false;
            }
            let path = e.path();
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                return false;
            };
            let dot_ext = format!(".{}", ext.to_lowercase());
            extensions.contains(&dot_ext.as_str())
        })
        .count()
}

/// Game count of each gamelist, kept in `game_counts.json` (app data dir) by path + size + date:
/// opening a profile doesn't read the gamelists that haven't changed.
#[derive(Default, Serialize, Deserialize)]
struct GameCounts {
    #[serde(default)]
    entries: HashMap<String, CountEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
struct CountEntry {
    size: u64,
    modified_ms: u64,
    games: usize,
}

/// Size and modification date (ms) of a file
fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64;
    Some((meta.len(), modified))
}

impl GameCounts {
    fn load(file: &Path) -> Self {
        fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    fn save(&self, file: &Path) {
        if let (Some(dir), Ok(text)) = (file.parent(), serde_json::to_string(self)) {
            let _ = fs::create_dir_all(dir);
            let _ = fs::write(file, text);
        }
    }

    /// Games of a gamelist: from the cache if the file is the same, else counted (and remembered)
    fn games_in(&mut self, gamelist: &Path) -> usize {
        let key = gamelist.to_string_lossy().to_string();
        let Some((size, modified_ms)) = stamp(gamelist) else { return 0 };
        if let Some(e) = self.entries.get(&key).filter(|e| e.size == size && e.modified_ms == modified_ms) {
            return e.games;
        }
        let games = fs::read_to_string(gamelist).ok().and_then(|xml| parser::count_games(&xml).ok()).unwrap_or(0);
        self.entries.insert(key, CountEntry { size, modified_ms, games });
        games
    }
}

fn counts_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("game_counts.json"))
}

#[tauri::command(async)]
pub fn scan_systems(root_path: String, app: AppHandle, task: Option<String>) -> Result<Vec<SystemInfo>, String> {
    let progress = Progress::new(&app, task);
    progress.phase("scanning");
    let root = Path::new(&root_path);
    if !root.is_dir() {
        return Err(format!("Path is not a directory: {}", root_path));
    }
    let layout = super::active_layout(&app)?;

    // First pass: collect valid directories to get total count
    let dirs: Vec<_> = fs::read_dir(root)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            if !path.is_dir() {
                return false;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // ES-DE's own folders (gamelists/, downloaded_media/...) when it lives in the ROM folder
            !name.starts_with('.') && !layout.is_own_folder(&path)
        })
        .collect();

    let total = dirs.len();
    let mut systems = Vec::new();
    let counts_path = counts_file(&app);
    let mut counts = counts_path.as_deref().map(GameCounts::load).unwrap_or_default();

    for (i, entry) in dirs.into_iter().enumerate() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let full_name = system_full_name(&name)
            .unwrap_or(&name)
            .to_string();

        // Por dónde va (se ve en la barra lateral mientras tanto)
        progress.step("systems", i + 1, total, &full_name);

        // Classic: <system>/gamelist.xml — ES-DE: <ES-DE>/gamelists/<system>/gamelist.xml
        let gamelist_path = layout.gamelist_path(&SystemRef::from_rom_dir(&path));
        let has_gamelist = gamelist_path.exists();
        let game_count = if has_gamelist { counts.games_in(&gamelist_path) } else { 0 };

        let rom_count = count_roms(&path, &name);

        systems.push(SystemInfo {
            name,
            full_name,
            path: path.to_string_lossy().to_string(),
            has_gamelist,
            game_count,
            rom_count,
        });
    }

    if let Some(file) = &counts_path {
        counts.save(file);
    }
    systems.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(systems)
}

/// Una carpeta de medios del sistema con cuántos ficheros tiene
#[derive(Clone, Serialize)]
pub struct MediaFolderInfo {
    pub folder: String,
    pub path: String,
    pub files: usize,
}

/// Dónde está cada cosa de un sistema según el perfil activo (para enseñarlo en su ficha)
#[derive(Clone, Serialize)]
pub struct SystemLocations {
    /// "classic" | "esde"
    pub structure: String,
    pub rom_dir: String,
    pub gamelist_path: String,
    pub gamelist_exists: bool,
    pub rom_dir_exists: bool,
    pub backups_dir: String,
    pub backups_exists: bool,
    /// Carpeta base de los medios (en clásica, la propia carpeta de ROMs)
    pub media_dir: String,
    /// Carpetas de medios que existen, con su número de ficheros
    pub media_folders: Vec<MediaFolderInfo>,
    pub media_dir_exists: bool,
    pub collections_dir: String,
    pub collections_exists: bool,
}

#[tauri::command(async)]
pub fn system_locations(app: AppHandle, system_path: String) -> Result<SystemLocations, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    Ok(locations(&layout, &system))
}

fn locations(layout: &crate::layout::Layout, system: &SystemRef) -> SystemLocations {
    let text = |p: &Path| p.to_string_lossy().to_string();
    let gamelist = layout.gamelist_path(system);

    // Carpetas de los tipos de medio y, en clásica, también las de nombres antiguos (images/…)
    let mut folders: Vec<String> = crate::media::MEDIA_KINDS.iter().map(|k| k.folder.to_string()).collect();
    if layout.writes_media_tags() {
        for (dirs, _, _) in super::images::legacy_targets() {
            for d in dirs {
                if !folders.iter().any(|f| f == d) {
                    folders.push(d.to_string());
                }
            }
        }
    }
    let media_folders = folders
        .into_iter()
        .filter_map(|folder| {
            let path = layout.media_type_dir(system, &folder);
            path.is_dir().then(|| MediaFolderInfo {
                files: crate::media::list_files_recursive(&path).len(),
                path: text(&path),
                folder,
            })
        })
        .collect();

    // La carpeta que contiene las de cada tipo (sin la barra final que dejaría un nombre vacío)
    let media_dir = layout.media_type_dir(system, "covers").parent().map(Path::to_path_buf).unwrap_or_default();
    let backups = layout.backups_dir(system);
    let collections = layout.collections_dir(system);
    SystemLocations {
        structure: if layout.writes_media_tags() { "classic" } else { "esde" }.to_string(),
        rom_dir: text(&system.rom_dir),
        rom_dir_exists: system.rom_dir.is_dir(),
        gamelist_exists: gamelist.is_file(),
        gamelist_path: text(&gamelist),
        backups_exists: backups.is_dir(),
        backups_dir: text(&backups),
        media_dir_exists: media_dir.is_dir(),
        media_dir: text(&media_dir),
        media_folders,
        collections_exists: collections.is_dir(),
        collections_dir: text(&collections),
    }
}

/// Lupa de «Dónde están los datos»: una carpeta se abre en el Finder/Explorador enseñando lo de
/// dentro; un fichero (la gamelist), su carpeta con él seleccionado. Se hace aquí y no desde la
/// ventana para que la app no necesite permiso para abrir cualquier ruta.
#[tauri::command(async)]
pub fn open_location(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let target = Path::new(&path);
    if target.is_dir() {
        app.opener().open_path(path, None::<String>).map_err(|e| e.to_string())
    } else if target.is_file() {
        app.opener().reveal_item_in_dir(target).map_err(|e| e.to_string())
    } else {
        Err(format!("Not found: {}", path))
    }
}

#[cfg(test)]
mod count_tests {
    use super::*;

    #[test]
    fn the_quick_count_matches_the_full_parse() {
        let xml = r#"<?xml version="1.0"?>
<gameList>
  <provider><System>NES</System></provider>
  <!-- <game><path>./commented.nes</path></game> -->
  <folder><path>./sub</path><name>Sub</name></folder>
  <game id="1"><path>./a.nes</path><name>A</name></game>
  <game><path>./b.nes</path><name>B <![CDATA[<game>]]></name></game>
  <game><path>./c.nes</path></game>
  <alternativeEmulator><label>x</label></alternativeEmulator>
</gameList>"#;
        assert_eq!(parser::count_games(xml).unwrap(), parser::parse_gamelist(xml).unwrap().games.len());
        assert_eq!(parser::count_games(xml).unwrap(), 3);
    }

    #[test]
    fn the_cached_count_is_dropped_when_the_gamelist_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let gamelist = tmp.path().join("gamelist.xml");
        let cache = tmp.path().join("data/game_counts.json");
        fs::write(&gamelist, "<gameList><game><path>./a</path></game></gameList>").unwrap();
        let mut counts = GameCounts::load(&cache);
        assert_eq!(counts.games_in(&gamelist), 1);
        counts.save(&cache);
        // Guardada: sale de la caché aunque el fichero no se lea
        let mut again = GameCounts::load(&cache);
        assert_eq!(again.entries.len(), 1);
        assert_eq!(again.games_in(&gamelist), 1);
        // Otro tamaño: se vuelve a contar
        fs::write(&gamelist, "<gameList><game><path>./a</path></game><game><path>./b</path></game></gameList>").unwrap();
        assert_eq!(again.games_in(&gamelist), 2);
    }

    #[test]
    fn roms_are_counted_by_extension_without_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("nes");
        fs::create_dir_all(dir.join("sub.nes")).unwrap();
        fs::write(dir.join("a.nes"), b"x").unwrap();
        fs::write(dir.join("b.NES"), b"x").unwrap();
        fs::write(dir.join("gamelist.xml"), b"x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("a.nes"), dir.join("link.nes")).unwrap();
        let expected = if cfg!(unix) { 3 } else { 2 };
        assert_eq!(count_roms(&dir, "nes"), expected, "a folder named .nes is not a ROM; a symlink to a ROM is");
    }
}

#[cfg(test)]
mod location_tests {
    use super::*;
    use crate::layout::{EsdePaths, Layout};

    #[test]
    fn esde_locations_point_to_the_esde_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Esde(EsdePaths { home: tmp.path().join("ES-DE"), media_root: tmp.path().join("ES-DE/downloaded_media") });
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/cps2"));
        fs::create_dir_all(tmp.path().join("ES-DE/downloaded_media/cps2/covers")).unwrap();
        fs::write(tmp.path().join("ES-DE/downloaded_media/cps2/covers/a.png"), b"x").unwrap();

        let loc = locations(&layout, &system);
        assert_eq!(loc.structure, "esde");
        assert!(loc.gamelist_path.ends_with(&format!("gamelists{}cps2{}gamelist.xml", std::path::MAIN_SEPARATOR, std::path::MAIN_SEPARATOR)));
        assert!(!loc.gamelist_exists);
        assert_eq!(loc.media_folders.len(), 1);
        assert_eq!((loc.media_folders[0].folder.as_str(), loc.media_folders[0].files), ("covers", 1));
    }

    #[test]
    fn classic_lists_legacy_media_folders_too() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        fs::create_dir_all(system.rom_dir.join("images")).unwrap();
        fs::write(system.rom_dir.join("images/a-image.png"), b"x").unwrap();
        fs::write(system.rom_dir.join("gamelist.xml"), b"<gameList/>").unwrap();

        let loc = locations(&Layout::Classic, &system);
        assert_eq!(loc.structure, "classic");
        assert!(loc.gamelist_exists);
        assert_eq!(loc.media_dir, system.rom_dir.to_string_lossy().to_string());
        assert!(loc.media_folders.iter().any(|f| f.folder == "images" && f.files == 1));
    }
}
