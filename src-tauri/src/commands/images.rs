use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[tauri::command]
pub fn copy_image_to_system(
    source: String,
    system_path: String,
    subfolder: String,
    filename: String,
) -> Result<String, String> {
    let dest_dir = Path::new(&system_path).join(&subfolder);
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Failed to create directory {}: {}", subfolder, e))?;

    let dest_path = dest_dir.join(&filename);
    fs::copy(&source, &dest_path)
        .map_err(|e| format!("Failed to copy image: {}", e))?;

    Ok(format!("./{}/{}", subfolder, filename))
}

#[tauri::command]
pub fn switch_image_type(
    system_path: String,
    primary_subfolder: String,
    fallback_subfolders: Vec<String>,
) -> Result<crate::models::GameList, String> {
    let mut gamelist = crate::commands::gamelist::load_gamelist(system_path.clone())?;
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

    // Save
    crate::commands::gamelist::save_gamelist(system_path, gamelist.clone())?;
    Ok(gamelist)
}

/// Scan media subfolders for files matching a ROM's filename stem.
/// Uses configured media_paths suffixes and subfolder names from settings.
/// Returns a map of game_field → relative_path for each match found.
#[tauri::command]
pub fn scan_game_media(
    app: tauri::AppHandle,
    system_path: String,
    game_path: String,
) -> Result<HashMap<String, String>, String> {
    let settings = crate::commands::settings::get_settings(app)?;
    let sys = Path::new(&system_path);
    let rom_stem = Path::new(&game_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    if rom_stem.is_empty() {
        return Ok(HashMap::new());
    }

    let image_exts = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];
    let video_exts = ["mp4", "mkv", "avi", "mov", "webm"];

    // Build configured suffixes map: game_field → suffix (e.g. "image" → "-image")
    let mut configured_suffixes: HashMap<String, String> = HashMap::new();
    let mut configured_subfolders: HashMap<String, String> = HashMap::new();
    if let Some(paths) = &settings.media_paths {
        for cfg in paths {
            configured_subfolders.insert(cfg.game_field.clone(), cfg.subfolder.clone());
            if !cfg.suffix.is_empty() {
                configured_suffixes.insert(cfg.game_field.clone(), cfg.suffix.clone());
            }
        }
    }

    // Map: (subfolder variants, game_field, common file suffixes)
    // Suffixes are tried after the configured suffix and before exact ROM stem
    let default_targets: Vec<(Vec<&str>, &str, Vec<&str>)> = vec![
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
        (vec!["images", "box-back", "boxback", "boxbacks"], "boxback",
         vec!["-boxback", "-box-back", "-back"]),
        (vec!["images", "3dboxes", "3dbox", "box3d", "box-3D"], "boxart",
         vec!["-boxart", "-box3d", "-box-3D", "-3dbox"]),
        (vec!["images", "wheels", "wheel"], "wheel",
         vec!["-wheel", "-logo"]),
        (vec!["images", "miximages", "miximage", "mixed"], "mix",
         vec!["-mix", "-miximage", "-mixrbv1", "-mixrbv2"]),
        (vec!["images", "cartridges", "cartridge", "carts"], "cartridge",
         vec!["-cartridge", "-cart"]),
        (vec!["images", "manuals", "manual"], "manual",
         vec!["-manual"]),
        (vec!["videos", "video", "snaps"], "video",
         vec!["-video", "-snap"]),
    ];

    let mut results = HashMap::new();

    for (default_subfolders, field, common_suffixes) in &default_targets {
        // Build ordered list of subfolders: configured first, then defaults
        let mut subfolders_to_try: Vec<String> = Vec::new();
        if let Some(configured) = configured_subfolders.get(*field) {
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
        if let Some(suffix) = configured_suffixes.get(*field) {
            if !suffix.is_empty() {
                stems_to_try.push(format!("{}{}", rom_stem, suffix));
            }
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

        let exts = if *field == "video" { &video_exts[..] } else { &image_exts[..] };

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

    Ok(results)
}
