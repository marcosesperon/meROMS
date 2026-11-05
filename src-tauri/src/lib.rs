mod commands;
mod layout;
mod media;
mod models;
mod parser;
mod rename_rules;
mod rom_hash;

use commands::{backup, cleanup, collections, dat, duplicates, export, gamelist, images, media_check, media_kinds, migrate, move_games, new_roms, normalize, placeholders, profiles, rename, rules, scrape_resume, scraper, scraper_launchbox, settings, systems};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // Self-update (signed releases on GitHub, see tauri.conf.json) and relaunch afterwards
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .register_uri_scheme_protocol("media", |_ctx, request| {
            // The frontend builds the URL with convertFileSrc: the whole file path, percent-encoded,
            // after the first "/" (media://localhost/<path>, or http://media.localhost/<path> on
            // Windows). Decoded, it is the absolute path as is: "/Users/..." or "C:\\...".
            let uri_path = request.uri().path();
            let decoded = percent_encoding::percent_decode_str(uri_path.trim_start_matches('/'))
                .decode_utf8_lossy()
                .to_string();
            let path = media_request_path(&decoded);
            match std::fs::read(&path) {
                Ok(data) => {
                    let mime = match path.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
                        "jpg" | "jpeg" => "image/jpeg",
                        "png" => "image/png",
                        "gif" => "image/gif",
                        "webp" => "image/webp",
                        "svg" => "image/svg+xml",
                        "mp4" => "video/mp4",
                        "webm" => "video/webm",
                        "avi" => "video/x-msvideo",
                        _ => "application/octet-stream",
                    };
                    tauri::http::Response::builder()
                        .status(200)
                        .header("Content-Type", mime)
                        .body(std::borrow::Cow::Owned(data))
                        .unwrap()
                }
                Err(_) => tauri::http::Response::builder()
                    .status(404)
                    .body(std::borrow::Cow::Borrowed(b"Not Found" as &[u8]))
                    .unwrap(),
            }
        })
        // Media «de relleno» aprendida: la lista negra vive en la carpeta de datos de la app
        .setup(|app| {
            placeholders::init(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings::get_settings,
            settings::save_settings,
            settings::export_settings,
            settings::import_settings,
            profiles::detect_esde_paths,
            profiles::detect_pending_structure,
            systems::scan_systems,
            systems::system_locations,
            gamelist::load_gamelist,
            gamelist::save_gamelist,
            gamelist::add_game,
            gamelist::remove_game,
            gamelist::generate_gamelist,
            gamelist::add_folder,
            gamelist::remove_folder,
            images::set_game_media,
            images::scan_game_media,
            scraper::scrape_game,
            scraper_launchbox::launchbox_status,
            scraper_launchbox::launchbox_download,
            scraper_launchbox::launchbox_delete,
            scraper::search_scrape,
            scraper::scrape_thumbnail,
            scraper::apply_scrape,
            scraper::scraper_status,
            scraper::get_scrape_policy,
            scraper::get_scrape_order,
            scraper::scrape_system,
            scraper::cancel_scrape,
            scrape_resume::get_scrape_resume,
            cleanup::find_unused_media,
            placeholders::find_placeholder_media,
            media_check::find_damaged_media,
            placeholders::learn_placeholders,
            placeholders::placeholder_count,
            placeholders::forget_placeholders,
            cleanup::find_game_leftover_media,
            cleanup::find_repeated_media,
            cleanup::trash_media,
            cleanup::find_broken_media_refs,
            cleanup::clear_media_refs,
            rename::list_rename_candidates,
            rename::rename_roms,
            move_games::list_subfolders,
            move_games::move_games,
            rename::check_rename_target,
            rules::get_default_rename_rules,
            rules::test_rename_rules,
            rules::import_rename_rules,
            rules::export_rename_rules,
            backup::list_backups,
            backup::restore_backup,
            backup::delete_backup,
            export::export_gamelist,
            images::switch_image_type,
            normalize::plan_media_normalization,
            normalize::apply_media_normalization,
            new_roms::list_new_roms,
            new_roms::list_missing_roms,
            new_roms::add_new_roms,
            new_roms::trash_files,
            media_kinds::swap_media,
            media_kinds::detect_swapped_media,
            migrate::plan_migration,
            migrate::run_migration,
            duplicates::find_duplicates,
            dat::set_system_dat,
            dat::clear_system_dat,
            dat::dat_report,
            dat::dat_compute_crcs,
            duplicates::remove_games_with_files,
            collections::detect_multidisc_games,
            collections::generate_m3u_playlists,
            collections::generate_collection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Absolute path asked through the media protocol. Old-style URLs (media://localhost/Users/...)
/// arrive without the leading "/", so it is put back unless it's a Windows path (C:\\, \\\\server).
fn media_request_path(decoded: &str) -> String {
    let is_windows_absolute = decoded.get(1..3).is_some_and(|s| s == ":\\" || s == ":/") || decoded.starts_with("\\\\");
    if decoded.starts_with('/') || is_windows_absolute {
        decoded.to_string()
    } else {
        format!("/{}", decoded)
    }
}

#[cfg(test)]
mod media_protocol_tests {
    use super::media_request_path;

    #[test]
    fn keeps_unix_windows_and_old_style_paths() {
        assert_eq!(media_request_path("/Users/yo/a.png"), "/Users/yo/a.png");
        assert_eq!(media_request_path("C:\\RetroBat\\roms/cps3/images/a.png"), "C:\\RetroBat\\roms/cps3/images/a.png");
        assert_eq!(media_request_path("C:/ES-DE/a.png"), "C:/ES-DE/a.png");
        assert_eq!(media_request_path("\\\\nas\\roms\\a.png"), "\\\\nas\\roms\\a.png");
        // Antiguo: media://localhost/Users/yo/a.png llegaba sin la barra inicial
        assert_eq!(media_request_path("Users/yo/a.png"), "/Users/yo/a.png");
    }
}
