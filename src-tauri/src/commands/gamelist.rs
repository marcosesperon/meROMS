use std::fs;
use std::path::Path;

use crate::layout::{Layout, SystemRef};
use crate::media;
use crate::models::system::extensions_for_system;
use crate::models::{Game, GameList};
use crate::models::Folder;
use crate::parser;
use super::progress::Progress;

/// Read a system's gamelist wherever the layout keeps it.
/// In ES-DE the XML has no media tags, so media fields are filled in memory by ROM name;
/// the rest of the app then works exactly as with the Classic structure.
pub fn read_gamelist(layout: &Layout, system: &SystemRef) -> Result<GameList, String> {
    read_gamelist_with(layout, system, &Progress::none())
}

/// `read_gamelist` avisando de la fase (leer el XML; en ES-DE, buscar la media de cada juego)
pub fn read_gamelist_with(layout: &Layout, system: &SystemRef, progress: &Progress) -> Result<GameList, String> {
    progress.phase("reading");
    let gamelist_path = layout.gamelist_path(system);

    let mut gamelist = if gamelist_path.exists() {
        let xml = fs::read_to_string(&gamelist_path)
            .map_err(|e| format!("Failed to read gamelist.xml: {}", e))?;
        parser::parse_gamelist(&xml)?
    } else {
        GameList::default()
    };

    if !layout.writes_media_tags() {
        progress.phase("media");
        media::hydrate_gamelist(layout, system, &mut gamelist);
    }
    Ok(gamelist)
}

/// Write a system's gamelist (backing up the previous one first).
/// In ES-DE media fields are dropped: ES-DE finds media by ROM name and doesn't want paths.
pub fn write_gamelist(layout: &Layout, system: &SystemRef, gamelist: &GameList) -> Result<(), String> {
    // Read original XML to preserve unknown fields during merge
    let original_xml = fs::read_to_string(layout.gamelist_path(system)).ok();
    write_gamelist_with_original(layout, system, gamelist, original_xml.as_deref())
}

/// Same as write_gamelist, but unknown fields are taken from `original_xml` instead of the
/// file being replaced (the migrator passes the source gamelist so nothing gets lost).
pub fn write_gamelist_with_original(
    layout: &Layout,
    system: &SystemRef,
    gamelist: &GameList,
    original_xml: Option<&str>,
) -> Result<(), String> {
    crate::commands::backup::backup_gamelist(layout, system)?;
    write_gamelist_file(layout, system, gamelist, original_xml)
}

/// Same as write_gamelist but without a backup: for «Scrape All», which saves every few games
/// and only backs up once, before starting (so the 20 backups aren't filled with halfway states).
pub fn write_gamelist_no_backup(layout: &Layout, system: &SystemRef, gamelist: &GameList) -> Result<(), String> {
    let original_xml = fs::read_to_string(layout.gamelist_path(system)).ok();
    write_gamelist_file(layout, system, gamelist, original_xml.as_deref())
}

/// Orden de los juegos al escribir una gamelist (Ajustes → «Ordenar la gamelist al guardar»).
/// Las escrituras no reciben los ajustes, así que se fija al leerlos o guardarlos.
static SORT: std::sync::Mutex<Option<GamelistSort>> = std::sync::Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamelistSort {
    Name,
    Path,
}

pub fn set_sort(sort: Option<&str>) {
    let value = match sort {
        Some("name") => Some(GamelistSort::Name),
        Some("path") => Some(GamelistSort::Path),
        _ => None,
    };
    if let Ok(mut s) = SORT.lock() {
        *s = value;
    }
}

/// Ordena juegos y carpetas: por nombre (sin mayúsculas; sin nombre, por fichero) o por ruta
pub fn sort_gamelist(gamelist: &mut GameList, sort: GamelistSort) {
    let key = |name: &Option<String>, path: &str| match sort {
        GamelistSort::Name => name.as_deref().filter(|n| !n.trim().is_empty()).unwrap_or(path).to_lowercase(),
        GamelistSort::Path => path.to_lowercase(),
    };
    gamelist.games.sort_by_cached_key(|g| (key(&g.name, &g.path), g.path.to_lowercase()));
    gamelist.folders.sort_by_cached_key(|f| (key(&f.name, &f.path), f.path.to_lowercase()));
}

/// Writes the XML through a temporary file renamed over the real one, so a crash halfway
/// leaves either the old gamelist or the new one, never half a file.
fn write_gamelist_file(
    layout: &Layout,
    system: &SystemRef,
    gamelist: &GameList,
    original_xml: Option<&str>,
) -> Result<(), String> {
    let gamelist_path = layout.gamelist_path(system);
    if let Some(dir) = gamelist_path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create gamelist folder: {}", e))?;
    }

    let mut to_write = gamelist.clone();
    if let Some(sort) = SORT.lock().ok().and_then(|s| *s) {
        sort_gamelist(&mut to_write, sort);
    }
    if !layout.writes_media_tags() {
        to_write.games.iter_mut().for_each(media::clear_media_fields);
        for folder in &mut to_write.folders {
            folder.image = None;
            folder.thumbnail = None;
        }
    }

    let xml = parser::serialize_gamelist(&to_write, original_xml)?;

    let tmp_path = gamelist_path.with_extension("xml.tmp");
    fs::write(&tmp_path, xml).map_err(|e| format!("Failed to write gamelist.xml: {}", e))?;
    fs::rename(&tmp_path, &gamelist_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("Failed to write gamelist.xml: {}", e)
    })
}

#[tauri::command(async)]
pub fn load_gamelist(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<GameList, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    read_gamelist_with(&layout, &system, &progress)
}

#[tauri::command(async)]
pub fn save_gamelist(app: tauri::AppHandle, system_path: String, gamelist: GameList) -> Result<(), String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    write_gamelist(&layout, &system, &gamelist)
}

/// Load, change and save a gamelist in one go (used by the small edit commands below).
fn update_gamelist(
    app: &tauri::AppHandle,
    system_path: &str,
    change: impl FnOnce(&mut GameList),
) -> Result<GameList, String> {
    let (layout, system) = super::system_context(app, system_path)?;
    let mut gamelist = read_gamelist(&layout, &system)?;
    change(&mut gamelist);
    write_gamelist(&layout, &system, &gamelist)?;
    Ok(gamelist)
}

#[tauri::command(async)]
pub fn add_game(app: tauri::AppHandle, system_path: String, game: Game) -> Result<GameList, String> {
    update_gamelist(&app, &system_path, |gl| gl.games.push(game))
}

#[tauri::command(async)]
pub fn remove_game(app: tauri::AppHandle, system_path: String, game_path: String) -> Result<GameList, String> {
    update_gamelist(&app, &system_path, |gl| gl.games.retain(|g| g.path != game_path))
}

/// Scans the system folder for ROM files based on the folder name,
/// generates a gamelist.xml with one entry per ROM, and saves it.
#[tauri::command(async)]
pub fn generate_gamelist(app: tauri::AppHandle, system_path: String, task: Option<String>) -> Result<GameList, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    progress.phase("scanning");
    let path = Path::new(&system_path);
    let folder_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let extensions = extensions_for_system(folder_name);

    let entries: Vec<_> = fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .collect();
    let total = entries.len();

    let mut games: Vec<Game> = Vec::new();

    for (i, entry) in entries.into_iter().enumerate() {
        // Cancelado: no se escribe nada (la gamelist aún no existía)
        progress.check()?;
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let file_path = entry.path();
        progress.step("files", i + 1, total, &entry.file_name().to_string_lossy());

        if !file_path.is_file() {
            continue;
        }

        let Some(ext) = file_path.extension().and_then(|e| e.to_str()) else {
            continue;
        };

        let dot_ext = format!(".{}", ext.to_lowercase());
        if !extensions.contains(&dot_ext.as_str()) {
            continue;
        }

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Derive a readable name from the filename (strip extension)
        let display_name = file_path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or(&file_name)
            .to_string();

        let relative_path = format!("./{}", file_name);

        // Classic: link an existing cover if there is one (ES-DE finds media by itself on load)
        let image = if layout.writes_media_tags() {
            find_existing_image(&layout, &system, &relative_path)
        } else {
            None
        };

        games.push(Game {
            path: relative_path,
            name: Some(display_name),
            image,
            ..Default::default()
        });
    }

    games.sort_by(|a, b| {
        a.name
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .cmp(&b.name.as_deref().unwrap_or("").to_lowercase())
    });

    let gamelist = GameList { games, ..Default::default() };
    progress.check()?;
    progress.phase("saving");
    write_gamelist(&layout, &system, &gamelist)?;
    // Re-read so ES-DE media fields are filled in for the UI
    read_gamelist_with(&layout, &system, &progress)
}

/// Looks for an existing cover: first the unified `covers/<ROM>.<ext>`,
/// then legacy subdirectories (images, media/images, etc.)
fn find_existing_image(layout: &Layout, system: &SystemRef, rom_path: &str) -> Option<String> {
    if let Some(kind) = media::kind_for_field("image") {
        for ext in kind.extensions() {
            let candidate = layout.media_file_path(system, kind.folder, rom_path, ext);
            if candidate.exists() {
                return Some(media::media_ref(layout, system, &candidate));
            }
        }
    }

    let system_dir = system.rom_dir.as_path();
    let rom_stem = crate::layout::rom_media_stem(system, rom_path);
    let rom_stem = rom_stem.to_string_lossy();
    let image_dirs = ["images", "media/images", "media/screenshots", "media/box2dfront"];
    let image_exts = ["png", "jpg", "jpeg", "gif", "webp"];

    for dir in &image_dirs {
        let img_dir = system_dir.join(dir);
        if !img_dir.is_dir() {
            continue;
        }
        for ext in &image_exts {
            // Try exact stem match
            let candidate = img_dir.join(format!("{}.{}", rom_stem, ext));
            if candidate.exists() {
                return Some(format!("./{}/{}.{}", dir, rom_stem, ext));
            }
            // Try stem-image suffix (common in scraped sets)
            let candidate = img_dir.join(format!("{}-image.{}", rom_stem, ext));
            if candidate.exists() {
                return Some(format!("./{}/{}-image.{}", dir, rom_stem, ext));
            }
        }
    }
    None
}

#[tauri::command(async)]
pub fn add_folder(app: tauri::AppHandle, system_path: String, folder: Folder) -> Result<GameList, String> {
    update_gamelist(&app, &system_path, |gl| gl.folders.push(folder))
}

#[tauri::command(async)]
pub fn remove_folder(app: tauri::AppHandle, system_path: String, folder_path: String) -> Result<GameList, String> {
    update_gamelist(&app, &system_path, |gl| gl.folders.retain(|f| f.path != folder_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::EsdePaths;

    fn esde_layout(root: &Path) -> Layout {
        Layout::Esde(EsdePaths {
            home: root.join("ES-DE"),
            media_root: root.join("ES-DE/downloaded_media"),
        })
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn esde_writes_without_media_and_reads_them_back_by_rom_name() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde_layout(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        let cover = tmp.path().join("ES-DE/downloaded_media/snes/covers/Juego.png");
        touch(&cover);

        let gamelist = GameList {
            games: vec![Game {
                path: "./Juego.sfc".into(),
                name: Some("Juego".into()),
                image: Some(cover.to_string_lossy().to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };
        write_gamelist(&layout, &system, &gamelist).unwrap();

        let on_disk = fs::read_to_string(tmp.path().join("ES-DE/gamelists/snes/gamelist.xml")).unwrap();
        assert!(!on_disk.contains("<image>"), "ES-DE gamelist must not carry media paths");
        assert!(on_disk.contains("<name>Juego</name>"));

        let loaded = read_gamelist(&layout, &system).unwrap();
        assert_eq!(loaded.games[0].image.as_deref(), Some(cover.to_str().unwrap()));
    }

    #[test]
    fn esde_backups_live_next_to_the_gamelist() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde_layout(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        let gamelist = GameList {
            games: vec![Game { path: "./a.sfc".into(), ..Default::default() }],
            ..Default::default()
        };
        // Second save backs up the first one
        write_gamelist(&layout, &system, &gamelist).unwrap();
        write_gamelist(&layout, &system, &gamelist).unwrap();

        let backups: Vec<_> = fs::read_dir(tmp.path().join("ES-DE/gamelists/snes/.backups"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(backups.len(), 1);
        assert!(!tmp.path().join("ROMs/snes/.backups").exists());
    }

    #[test]
    fn classic_keeps_media_paths_in_the_xml() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        fs::create_dir_all(&system.rom_dir).unwrap();
        let gamelist = GameList {
            games: vec![Game {
                path: "./Juego.sfc".into(),
                image: Some("./covers/Juego.png".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        write_gamelist(&Layout::Classic, &system, &gamelist).unwrap();
        let on_disk = fs::read_to_string(system.rom_dir.join("gamelist.xml")).unwrap();
        assert!(on_disk.contains("<image>./covers/Juego.png</image>"));
    }

    #[test]
    fn saving_without_backup_leaves_no_backup_nor_temp_file() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde_layout(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        let mut gamelist = GameList {
            games: vec![Game { path: "./a.sfc".into(), name: Some("Uno".into()), ..Default::default() }],
            ..Default::default()
        };
        write_gamelist_no_backup(&layout, &system, &gamelist).unwrap();
        gamelist.games[0].name = Some("Dos".into());
        write_gamelist_no_backup(&layout, &system, &gamelist).unwrap();

        let dir = tmp.path().join("ES-DE/gamelists/snes");
        assert!(!dir.join(".backups").exists(), "no backups while saving every few games");
        assert!(!dir.join("gamelist.xml.tmp").exists(), "the temporary file is renamed away");
        let on_disk = fs::read_to_string(dir.join("gamelist.xml")).unwrap();
        assert!(on_disk.contains("<name>Dos</name>"));
    }

    #[test]
    fn sorts_by_name_or_by_file() {
        let mut gl = GameList {
            games: vec![
                Game { path: "./b.sfc".into(), name: Some("zelda".into()), ..Default::default() },
                Game { path: "./c.sfc".into(), name: None, ..Default::default() },
                Game { path: "./a.sfc".into(), name: Some("Mario".into()), ..Default::default() },
            ],
            ..Default::default()
        };
        sort_gamelist(&mut gl, GamelistSort::Name);
        let paths: Vec<&str> = gl.games.iter().map(|g| g.path.as_str()).collect();
        // Sin nombre cuenta el fichero («./c.sfc» va antes que «mario»)
        assert_eq!(paths, vec!["./c.sfc", "./a.sfc", "./b.sfc"]);
        sort_gamelist(&mut gl, GamelistSort::Path);
        let paths: Vec<&str> = gl.games.iter().map(|g| g.path.as_str()).collect();
        assert_eq!(paths, vec!["./a.sfc", "./b.sfc", "./c.sfc"]);
    }
}
