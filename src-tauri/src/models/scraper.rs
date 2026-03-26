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
    pub status: String,
}
