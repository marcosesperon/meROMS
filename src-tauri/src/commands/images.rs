use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::layout::SystemRef;
use crate::media::{self, MediaIndex, MEDIA_KINDS};

/// Copy a user-picked file as a game's media, using the unified naming
/// `<media folder>/<ROM name>.<ext>` of the active structure.
/// Returns the value to store in the game field (relative in Classic, absolute in ES-DE).
#[tauri::command(async)]
pub fn set_game_media(
    app: tauri::AppHandle,
    system_path: String,
    game_path: String,
    field: String,
    source: String,
) -> Result<String, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let kind = media::kind_for_field(&field).ok_or_else(|| format!("Unknown media field: {}", field))?;
    let ext = media::extension_lower(Path::new(&source)).unwrap_or_else(|| "png".into());

    // Remove the same media with another extension, or the old one could keep winning
    for other_ext in kind.extensions().iter().filter(|e| **e != ext) {
        let old = layout.media_file_path(&system, kind.folder, &game_path, other_ext);
        if old.exists() {
            fs::remove_file(&old).map_err(|e| format!("Failed to replace old media: {}", e))?;
        }
    }

    let dest = layout.media_file_path(&system, kind.folder, &game_path, &ext);
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create media folder: {}", e))?;
    }
    fs::copy(&source, &dest).map_err(|e| format!("Failed to copy media: {}", e))?;

    Ok(media::media_ref(&layout, &system, &dest))
}

#[tauri::command(async)]
pub fn switch_image_type(
    app: tauri::AppHandle,
    system_path: String,
    primary_subfolder: String,
    fallback_subfolders: Vec<String>,
) -> Result<crate::models::GameList, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    if !layout.writes_media_tags() {
        // ES-DE decides which media to show by itself; there is no image path to switch
        return Err("Switching image type is only available for the Classic structure".into());
    }

    let mut gamelist = crate::commands::gamelist::read_gamelist(&layout, &system)?;
    let sys_path = Path::new(&system_path);
    let image_exts = ["png", "jpg", "jpeg", "gif", "webp"];

    for game in &mut gamelist.games {
        let rom_stem = Path::new(&game.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if rom_stem.is_empty() {
            continue;
        }

        let mut found = false;
        // Try primary subfolder first, then fallbacks
        let folders_to_try: Vec<&str> = std::iter::once(primary_subfolder.as_str())
            .chain(fallback_subfolders.iter().map(|s| s.as_str()))
            .collect();

        for subfolder in &folders_to_try {
            for ext in &image_exts {
                let candidate = sys_path.join(subfolder).join(format!("{}.{}", rom_stem, ext));
                if candidate.exists() {
                    game.image = Some(format!("./{}/{}.{}", subfolder, rom_stem, ext));
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
    }

    crate::commands::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    Ok(gamelist)
}

/// Look for a ROM's media files on disk.
/// Returns a map of game_field → path for each match found.
/// - ES-DE: only the unified `<folder>/<ROM name>.<ext>` (that's all ES-DE reads).
/// - Classic: the unified name first, then legacy folders/suffixes left by other scrapers.
#[tauri::command(async)]
pub fn scan_game_media(
    app: tauri::AppHandle,
    system_path: String,
    game_path: String,
) -> Result<HashMap<String, String>, String> {
    let settings = crate::commands::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;

    let mut results = HashMap::new();
    let index = MediaIndex::build(&layout, &system);
    for kind in MEDIA_KINDS {
        if let Some(found) = index.find(&system, kind, &game_path) {
            results.insert(kind.field.to_string(), media::media_ref(&layout, &system, found));
        }
    }

    if layout.writes_media_tags() {
        for (field, path) in scan_legacy_media(&settings, &system, &game_path) {
            results.entry(field).or_insert(path);
        }
    }
    Ok(results)
}

/// Classic only: media named the old ways (Skraper/EmulationStation suffixes, "images/" folder…).
/// Uses the legacy subfolder/suffix hints still stored in settings, then common patterns.
pub fn scan_legacy_media(
    settings: &crate::models::AppSettings,
    system: &SystemRef,
    game_path: &str,
) -> HashMap<String, String> {
    let sys = system.rom_dir.as_path();
    let rom_stem = Path::new(&game_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    if rom_stem.is_empty() {
        return HashMap::new();
    }

    let image_exts = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];
    let video_exts = ["mp4", "mkv", "avi", "mov", "webm"];

    // Build configured suffixes map: game_field → suffix (e.g. "image" → "-image")
    let mut configured_suffixes: HashMap<String, String> = HashMap::new();
    let mut configured_subfolders: HashMap<String, String> = HashMap::new();
    if let Some(paths) = &settings.media_paths {
        for cfg in paths {
            if !cfg.subfolder.is_empty() {
                configured_subfolders.insert(cfg.game_field.clone(), cfg.subfolder.clone());
            }
            if !cfg.suffix.is_empty() {
                configured_suffixes.insert(cfg.game_field.clone(), cfg.suffix.clone());
            }
        }
    }

    let mut results = HashMap::new();

    for (default_subfolders, field, common_suffixes) in legacy_targets() {
        // Build ordered list of subfolders: configured first, then defaults
        let mut subfolders_to_try: Vec<String> = Vec::new();
        if let Some(configured) = configured_subfolders.get(field) {
            subfolders_to_try.push(configured.clone());
        }
        for sf in default_subfolders {
            let s = sf.to_string();
            if !subfolders_to_try.contains(&s) {
                subfolders_to_try.push(s);
            }
        }

        // Build ordered list of stems to try
        let mut stems_to_try: Vec<String> = Vec::new();
        // 1. Configured suffix from settings (e.g. "-boxart" from media paths config)
        if let Some(suffix) = configured_suffixes.get(field) {
            stems_to_try.push(format!("{}{}", rom_stem, suffix));
        }
        // 2. Exact ROM stem (e.g. "Ghoul Patrol")
        stems_to_try.push(rom_stem.clone());
        // 3. All common suffixes for this field (e.g. "-screenshot", "-ss", "-snap")
        for suffix in common_suffixes {
            let s = format!("{}{}", rom_stem, suffix);
            if !stems_to_try.contains(&s) {
                stems_to_try.push(s);
            }
        }

        let exts = if field == "video" { &video_exts[..] } else { &image_exts[..] };

        let mut found = false;
        for subfolder in &subfolders_to_try {
            let dir = sys.join(subfolder);
            if !dir.is_dir() {
                continue;
            }

            for stem in &stems_to_try {
                for ext in exts {
                    let candidate = dir.join(format!("{}.{}", stem, ext));
                    if candidate.exists() {
                        results.insert(
                            field.to_string(),
                            format!("./{}/{}.{}", subfolder, stem, ext),
                        );
                        found = true;
                        break;
                    }
                }
                if found { break; }
            }
            if found { break; }
        }
    }

    results
}

/// Legacy media locations: (subfolder variants, game_field, common file suffixes).
/// Also used by the media normalizer to find files to bring to the unified naming.
pub fn legacy_targets() -> Vec<(Vec<&'static str>, &'static str, Vec<&'static str>)> {
    vec![
        (vec!["images", "covers", "box2d", "box-2D", "boxart"], "image",
         vec!["-image", "-boxart", "-box2d", "-box-2D", "-cover", "-front"]),
        (vec!["images", "screenshots", "screenshot", "snap"], "thumbnail",
         vec!["-thumbnail", "-screenshot", "-ss", "-snap", "-thumb"]),
        (vec!["images", "marquees", "marquee", "wheel", "wheels"], "marquee",
         vec!["-marquee", "-wheel", "-logo"]),
        (vec!["images", "fanart", "fanarts"], "fanart",
         vec!["-fanart", "-fan"]),
        (vec!["images", "titlescreens", "titlescreen", "titleshots"], "titleshot",
         vec!["-titleshot", "-titlescreen", "-title"]),
        (vec!["images", "box-back", "boxback", "boxbacks", "backcovers"], "boxback",
         vec!["-boxback", "-box-back", "-back"]),
        (vec!["images", "3dboxes", "3dbox", "box3d", "box-3D"], "boxart",
         vec!["-boxart", "-box3d", "-box-3D", "-3dbox"]),
        (vec!["images", "wheels", "wheel"], "wheel",
         vec!["-wheel", "-logo"]),
        (vec!["images", "miximages", "miximage", "mixed"], "mix",
         vec!["-mix", "-miximage", "-mixrbv1", "-mixrbv2"]),
        (vec!["images", "cartridges", "cartridge", "carts", "physicalmedia"], "cartridge",
         vec!["-cartridge", "-cart"]),
        (vec!["images", "manuals", "manual"], "manual",
         vec!["-manual"]),
        (vec!["videos", "video", "snaps"], "video",
         vec!["-video", "-snap"]),
    ]
}

