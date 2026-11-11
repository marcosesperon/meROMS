pub mod backup;
pub mod best_version;
pub mod bios;
pub mod chd;
pub mod cleanup;
pub mod dat;
pub mod collections;
pub mod compare;
pub mod duplicates;
pub mod export;
pub mod export_device;
pub mod gamelist;
pub mod launch;
pub mod rename;
pub mod rules;
pub mod images;
pub mod media_check;
pub mod media_kinds;
pub mod migrate;
pub mod move_games;
pub mod new_roms;
pub mod normalize;
pub mod game_meta;
pub mod game_status;
pub mod placeholders;
pub mod saves;
pub mod search;
pub mod shots;
pub mod profiles;
pub mod progress;
pub mod scrape_resume;
pub mod scrape_common;
pub mod scraper;
pub mod scraper_adb;
pub mod scraper_launchbox;
pub mod scraper_tgdb;
pub mod settings;
pub mod systems;
pub mod transfer_profile;
pub mod translate;
pub mod undo;

use crate::layout::{Layout, SystemRef};

/// Layout of the active profile plus the system a command works on.
/// Without an active profile (fresh install) the Classic structure is assumed,
/// which is how meROMS always behaved.
pub fn system_context(app: &tauri::AppHandle, system_path: &str) -> Result<(Layout, SystemRef), String> {
    Ok((active_layout(app)?, SystemRef::from_rom_dir(system_path)))
}

pub fn active_layout(app: &tauri::AppHandle) -> Result<Layout, String> {
    let settings = settings::get_settings(app.clone())?;
    match settings.active_profile() {
        Some(profile) => Layout::for_profile(profile),
        None => Ok(Layout::Classic),
    }
}
