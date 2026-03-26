use std::fs;
use std::path::Path;

use chrono::Local;
use serde::{Deserialize, Serialize};

const BACKUP_DIR: &str = ".backups";
const MAX_BACKUPS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    pub filename: String,
    pub path: String,
    pub timestamp: String,
    pub size: u64,
}

/// Create a backup of gamelist.xml before modifying it.
/// Stores backups in `{system_path}/.backups/gamelist_YYYYMMDD_HHMMSS.xml`.
/// Automatically prunes old backups beyond MAX_BACKUPS.
pub fn backup_gamelist(system_path: &str) -> Result<(), String> {
    let gamelist_path = Path::new(system_path).join("gamelist.xml");

    if !gamelist_path.exists() {
        return Ok(()); // Nothing to back up
    }

    let backup_dir = Path::new(system_path).join(BACKUP_DIR);
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create backup directory: {}", e))?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_name = format!("gamelist_{}.xml", timestamp);
    let backup_path = backup_dir.join(&backup_name);

    fs::copy(&gamelist_path, &backup_path)
        .map_err(|e| format!("Failed to create backup: {}", e))?;

    // Prune old backups
    prune_backups(&backup_dir);

    Ok(())
}

/// Remove oldest backups if we exceed MAX_BACKUPS.
fn prune_backups(backup_dir: &Path) {
    let Ok(entries) = fs::read_dir(backup_dir) else {
        return;
    };

    let mut backups: Vec<_> = entries
        .flatten()
        .filter(|e| {
            e.path().is_file()
                && e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("gamelist_") && n.ends_with(".xml"))
        })
        .collect();

    if backups.len() <= MAX_BACKUPS {
        return;
    }

    // Sort by name (which is timestamp-based, so alphabetical = chronological)
    backups.sort_by_key(|e| e.file_name());

    // Remove oldest
    let to_remove = backups.len() - MAX_BACKUPS;
    for entry in backups.iter().take(to_remove) {
        fs::remove_file(entry.path()).ok();
    }
}

/// List all available backups for a system, newest first.
#[tauri::command]
pub fn list_backups(system_path: String) -> Result<Vec<BackupEntry>, String> {
    let backup_dir = Path::new(&system_path).join(BACKUP_DIR);

    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&backup_dir)
        .map_err(|e| format!("Failed to read backup directory: {}", e))?;

    let mut backups: Vec<BackupEntry> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let filename = e.file_name().to_str()?.to_string();

            if !filename.starts_with("gamelist_") || !filename.ends_with(".xml") {
                return None;
            }

            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

            // Extract timestamp from filename: gamelist_YYYYMMDD_HHMMSS.xml
            let ts_part = filename
                .strip_prefix("gamelist_")?
                .strip_suffix(".xml")?;

            // Format as readable: YYYY-MM-DD HH:MM:SS
            let timestamp = if ts_part.len() == 15 {
                format!(
                    "{}-{}-{} {}:{}:{}",
                    &ts_part[0..4],
                    &ts_part[4..6],
                    &ts_part[6..8],
                    &ts_part[9..11],
                    &ts_part[11..13],
                    &ts_part[13..15]
                )
            } else {
                ts_part.to_string()
            };

            Some(BackupEntry {
                filename,
                path: path.to_string_lossy().to_string(),
                timestamp,
                size,
            })
        })
        .collect();

    // Newest first
    backups.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(backups)
}

/// Restore a backup by copying it back as gamelist.xml.
/// Creates a backup of the current gamelist.xml first.
#[tauri::command]
pub fn restore_backup(system_path: String, backup_filename: String) -> Result<(), String> {
    let backup_path = Path::new(&system_path)
        .join(BACKUP_DIR)
        .join(&backup_filename);

    if !backup_path.exists() {
        return Err(format!("Backup not found: {}", backup_filename));
    }

    // Backup current state before restoring
    backup_gamelist(&system_path)?;

    let gamelist_path = Path::new(&system_path).join("gamelist.xml");
    fs::copy(&backup_path, &gamelist_path)
        .map_err(|e| format!("Failed to restore backup: {}", e))?;

    Ok(())
}

/// Delete a specific backup file.
#[tauri::command]
pub fn delete_backup(system_path: String, backup_filename: String) -> Result<(), String> {
    let backup_path = Path::new(&system_path)
        .join(BACKUP_DIR)
        .join(&backup_filename);

    if backup_path.exists() {
        fs::remove_file(&backup_path)
            .map_err(|e| format!("Failed to delete backup: {}", e))?;
    }

    Ok(())
}
