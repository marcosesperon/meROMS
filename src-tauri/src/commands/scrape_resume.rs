// «Scrape All» left halfway (app closed, crash or «Stop»): which games of each system were
// still to do and with which rules, so the next «Scrape All» can offer to continue.
// Kept in the app data dir as scrape_resume.json; one entry per gamelist.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::layout::{Layout, SystemRef};
use crate::models::settings::ScrapePolicy;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumeEntry {
    /// The system's gamelist path: tells apart the same ROM folder in Classic and ES-DE profiles
    pub key: String,
    /// ROM paths still to scrape, in gamelist order
    pub remaining: Vec<String>,
    /// Rules chosen for that run (None = the ones in Settings)
    pub policy: Option<ScrapePolicy>,
    /// Games in the run when it was first started
    pub total: usize,
    /// When it was first started (Unix seconds)
    pub started_at: i64,
}

/// What the «Scrape All» window shows about a run left halfway
#[derive(Debug, Clone, Serialize)]
pub struct ScrapeResumeInfo {
    pub remaining: usize,
    pub total: usize,
    pub started_at: i64,
}

pub fn resume_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    Ok(dir.join("scrape_resume.json"))
}

pub fn key(layout: &Layout, system: &SystemRef) -> String {
    layout.gamelist_path(system).to_string_lossy().to_string()
}

/// Every entry in the file (a missing or broken file counts as empty: it's only a reminder)
fn load_all(file: &Path) -> Vec<ResumeEntry> {
    fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_all(file: &Path, entries: &[ResumeEntry]) -> Result<(), String> {
    if entries.is_empty() {
        let _ = fs::remove_file(file);
        return Ok(());
    }
    let json = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(file, json).map_err(|e| format!("Failed to save {}: {}", file.display(), e))
}

pub fn get(file: &Path, key: &str) -> Option<ResumeEntry> {
    load_all(file).into_iter().find(|e| e.key == key)
}

/// Adds or replaces the entry of that gamelist
pub fn put(file: &Path, entry: ResumeEntry) -> Result<(), String> {
    let mut entries = load_all(file);
    entries.retain(|e| e.key != entry.key);
    entries.push(entry);
    save_all(file, &entries)
}

pub fn remove(file: &Path, key: &str) -> Result<(), String> {
    let mut entries = load_all(file);
    entries.retain(|e| e.key != key);
    save_all(file, &entries)
}

/// Run of this system left halfway, if any
#[tauri::command]
pub fn get_scrape_resume(app: tauri::AppHandle, system_path: String) -> Result<Option<ScrapeResumeInfo>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    Ok(get(&resume_file(&app)?, &key(&layout, &system)).map(|e| ScrapeResumeInfo {
        remaining: e.remaining.len(),
        total: e.total,
        started_at: e.started_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, remaining: &[&str]) -> ResumeEntry {
        ResumeEntry {
            key: key.into(),
            remaining: remaining.iter().map(|s| s.to_string()).collect(),
            policy: None,
            total: 10,
            started_at: 1_700_000_000,
        }
    }

    #[test]
    fn entries_are_kept_per_gamelist_and_the_file_goes_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("scrape_resume.json");

        put(&file, entry("/roms/snes/gamelist.xml", &["./a.sfc", "./b.sfc"])).unwrap();
        put(&file, entry("/es-de/gamelists/snes/gamelist.xml", &["./c.sfc"])).unwrap();
        // Saving again the same gamelist replaces its entry
        put(&file, entry("/roms/snes/gamelist.xml", &["./b.sfc"])).unwrap();

        assert_eq!(get(&file, "/roms/snes/gamelist.xml").unwrap().remaining, vec!["./b.sfc".to_string()]);
        assert_eq!(get(&file, "/es-de/gamelists/snes/gamelist.xml").unwrap().remaining.len(), 1);

        remove(&file, "/roms/snes/gamelist.xml").unwrap();
        assert!(get(&file, "/roms/snes/gamelist.xml").is_none());
        remove(&file, "/es-de/gamelists/snes/gamelist.xml").unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn a_broken_file_counts_as_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("scrape_resume.json");
        fs::write(&file, "{ not json").unwrap();
        assert!(get(&file, "x").is_none());
        put(&file, entry("x", &["./a.sfc"])).unwrap();
        assert!(get(&file, "x").is_some());
    }
}
