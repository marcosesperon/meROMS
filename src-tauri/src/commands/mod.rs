pub mod backup;
pub mod cleanup;
pub mod collections;
pub mod duplicates;
pub mod export;
pub mod gamelist;
pub mod rename;
pub mod images;
pub mod scraper;
pub mod scraper_tgdb;
pub mod settings;
pub mod systems;

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
