use std::fs;
use std::path::Path;

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::layout::{Layout, SystemRef};

const MAX_BACKUPS: usize = 20;
/// Labeled backups (e.g. "before Put it all in order") are kept apart from the 20 usual ones,
/// so the writes of that same operation can't push them out
const MAX_LABELED: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    pub filename: String,
    pub path: String,
    pub timestamp: String,
    pub size: u64,
    /// Why it was made, when it isn't the automatic copy of every save (`order` = Put it all in order)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Create a backup of gamelist.xml before modifying it.
/// Stores backups next to the gamelist in `.backups/gamelist_YYYYMMDD_HHMMSS.xml`
/// (system folder in Classic, `gamelists/<system>/` in ES-DE).
/// Automatically prunes old backups beyond MAX_BACKUPS.
pub fn backup_gamelist(layout: &Layout, system: &SystemRef) -> Result<(), String> {
    backup_gamelist_labeled(layout, system, None)
}

/// Like `backup_gamelist`, with a label in the name (`gamelist_<date>_<label>.xml`).
/// Labeled ones don't count for the usual 20: the last `MAX_LABELED` are kept.
pub fn backup_gamelist_labeled(layout: &Layout, system: &SystemRef, label: Option<&str>) -> Result<(), String> {
    let gamelist_path = layout.gamelist_path(system);

    if !gamelist_path.exists() {
        return Ok(()); // Nothing to back up
    }

    let backup_dir = layout.backups_dir(system);
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create backup directory: {}", e))?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_name = match label {
        Some(label) => format!("gamelist_{}_{}.xml", timestamp, label),
        None => format!("gamelist_{}.xml", timestamp),
    };
    let backup_path = backup_dir.join(&backup_name);

    fs::copy(&gamelist_path, &backup_path)
        .map_err(|e| format!("Failed to create backup: {}", e))?;

    // Prune old backups
    prune_backups(&backup_dir);

    Ok(())
}

/// Label of a backup from its name (`gamelist_YYYYMMDD_HHMMSS_<label>.xml`)
fn label_of(filename: &str) -> Option<String> {
    let ts = filename.strip_prefix("gamelist_")?.strip_suffix(".xml")?;
    ts.get(16..).filter(|l| ts.as_bytes().get(15) == Some(&b'_') && !l.is_empty()).map(str::to_string)
}

/// Remove oldest backups if we exceed MAX_BACKUPS (the labeled ones apart, MAX_LABELED).
fn prune_backups(backup_dir: &Path) {
    prune_kind(backup_dir, false, MAX_BACKUPS);
    prune_kind(backup_dir, true, MAX_LABELED);
}

fn prune_kind(backup_dir: &Path, labeled: bool, max: usize) {
    let Ok(entries) = fs::read_dir(backup_dir) else {
        return;
    };

    let mut backups: Vec<_> = entries
        .flatten()
        .filter(|e| {
            e.path().is_file()
                && e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("gamelist_") && n.ends_with(".xml") && label_of(n).is_some() == labeled)
        })
        .collect();

    if backups.len() <= max {
        return;
    }

    // Sort by name (which is timestamp-based, so alphabetical = chronological)
    backups.sort_by_key(|e| e.file_name());

    // Remove oldest
    let to_remove = backups.len() - max;
    for entry in backups.iter().take(to_remove) {
        fs::remove_file(entry.path()).ok();
    }
}

/// List all available backups for a system, newest first.
#[tauri::command(async)]
pub fn list_backups(app: tauri::AppHandle, system_path: String) -> Result<Vec<BackupEntry>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let backup_dir = layout.backups_dir(&system);

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

            // Format as readable: YYYY-MM-DD HH:MM:SS (a label may follow the date)
            let label = label_of(&filename);
            let ts_part = if label.is_some() { &ts_part[..15] } else { ts_part };
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
                label,
            })
        })
        .collect();

    // Newest first
    backups.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(backups)
}

/// Labeled backup, made on purpose before a big operation (Put it all in order).
/// `label`: letters, digits and hyphens only (it goes in the file name).
#[tauri::command(async)]
pub fn backup_labeled(app: tauri::AppHandle, system_path: String, label: String) -> Result<(), String> {
    if label.is_empty() || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("Invalid backup label: {}", label));
    }
    let (layout, system) = super::system_context(&app, &system_path)?;
    backup_gamelist_labeled(&layout, &system, Some(&label))
}

/// Restore a backup by copying it back as gamelist.xml.
/// Creates a backup of the current gamelist.xml first.
#[tauri::command(async)]
pub fn restore_backup(app: tauri::AppHandle, system_path: String, backup_filename: String) -> Result<(), String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let backup_path = backup_file_path(&layout, &system, &backup_filename)?;

    if !backup_path.exists() {
        return Err(format!("Backup not found: {}", backup_filename));
    }

    // Backup current state before restoring
    backup_gamelist(&layout, &system)?;

    let gamelist_path = layout.gamelist_path(&system);
    fs::copy(&backup_path, &gamelist_path)
        .map_err(|e| format!("Failed to restore backup: {}", e))?;

    Ok(())
}

/// Delete a specific backup file.
#[tauri::command(async)]
pub fn delete_backup(app: tauri::AppHandle, system_path: String, backup_filename: String) -> Result<(), String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let backup_path = backup_file_path(&layout, &system, &backup_filename)?;

    if backup_path.exists() {
        fs::remove_file(&backup_path)
            .map_err(|e| format!("Failed to delete backup: {}", e))?;
    }

    Ok(())
}

/// Path of a backup file, refusing names that could escape the backups folder.
fn backup_file_path(layout: &Layout, system: &SystemRef, backup_filename: &str) -> Result<std::path::PathBuf, String> {
    if Path::new(backup_filename).file_name().and_then(|n| n.to_str()) != Some(backup_filename) {
        return Err(format!("Invalid backup name: {}", backup_filename));
    }
    Ok(layout.backups_dir(system).join(backup_filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labeled_backups_are_kept_apart_from_the_twenty_usual_ones() {
        assert_eq!(label_of("gamelist_20261123_101010_order.xml").as_deref(), Some("order"));
        assert_eq!(label_of("gamelist_20261123_101010.xml"), None);
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("gamelist_20200101_000000_order.xml"), "x").unwrap();
        for i in 0..25 {
            fs::write(dir.join(format!("gamelist_20210101_0000{:02}.xml", i)), "x").unwrap();
        }
        prune_backups(dir);
        let names: Vec<String> = fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(names.len(), MAX_BACKUPS + 1);
        assert!(names.contains(&"gamelist_20200101_000000_order.xml".to_string()), "the oldest, but labeled, stays");
    }
}
