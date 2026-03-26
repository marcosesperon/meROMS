mod commands;
mod models;
mod parser;

use commands::{backup, cleanup, collections, duplicates, export, gamelist, images, rename, scraper, settings, systems};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .register_uri_scheme_protocol("media", |_ctx, request| {
            let uri_path = request.uri().path();
            // Decode percent-encoded path, keep leading '/' for absolute paths
            let decoded = percent_encoding::percent_decode_str(uri_path)
                .decode_utf8_lossy()
                .to_string();
            // On macOS/Linux paths are absolute (/Users/...), on Windows they start with drive letter
            let path = if decoded.starts_with('/') {
                decoded
            } else {
                format!("/{}", decoded)
            };
            match std::fs::read(&path) {
                Ok(data) => {
                    let mime = match path.rsplit('.').next().unwrap_or("") {
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
        .invoke_handler(tauri::generate_handler![
            settings::get_settings,
            settings::save_settings,
            settings::export_settings,
            settings::import_settings,
            systems::scan_systems,
            gamelist::load_gamelist,
            gamelist::save_gamelist,
            gamelist::add_game,
            gamelist::remove_game,
            gamelist::generate_gamelist,
            gamelist::add_folder,
            gamelist::remove_folder,
            images::copy_image_to_system,
            images::scan_game_media,
            scraper::scrape_game,
            scraper::scrape_system,
            cleanup::find_unused_media,
            cleanup::delete_files,
            rename::list_rename_candidates,
            rename::rename_roms,
            backup::list_backups,
            backup::restore_backup,
            backup::delete_backup,
            export::export_gamelist,
            images::switch_image_type,
            duplicates::find_duplicates,
            collections::detect_multidisc_games,
            collections::generate_m3u_playlists,
            collections::generate_collection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
