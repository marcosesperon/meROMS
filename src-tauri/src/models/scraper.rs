use serde::{Deserialize, Serialize};

use super::game::Game;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeResult {
    pub game: Game,
    pub found: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeProgress {
    pub current: usize,
    pub total: usize,
    pub game_name: String,
    /// ROM path of the game (the progress window offers a manual search for the ones not found)
    pub game_path: String,
    pub status: String,
}

/// One result of a manual search: a short card to pick from (the full data is fetched
/// with `apply_scrape` once the user picks it)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScrapeCandidate {
    /// "screenscraper" | "thegamesdb"
    pub source: String,
    /// Game id in that source
    pub id: String,
    pub name: String,
    pub system: Option<String>,
    pub year: Option<String>,
    pub developer: Option<String>,
    /// Cover thumbnail: a data: URI (ScreenScraper, its URLs carry the credentials) or a public URL
    pub thumbnail: Option<String>,
    /// ScreenScraper recognised the ROM file itself (by its MD5)
    pub rom_match: bool,
}

/// What a manual search found, plus the sources that failed (the others still count)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeSearch {
    pub candidates: Vec<ScrapeCandidate>,
    pub errors: Vec<String>,
}
