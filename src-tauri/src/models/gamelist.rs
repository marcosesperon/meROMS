use serde::{Deserialize, Serialize};

use super::folder::Folder;
use super::game::Game;
use super::provider::Provider;

/// GameList used for Tauri IPC (JSON serialization).
/// Field names are clean/standard for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameList {
    #[serde(default)]
    pub games: Vec<Game>,
    #[serde(default)]
    pub folders: Vec<Folder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
}
