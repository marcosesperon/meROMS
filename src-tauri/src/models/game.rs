use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Game {
    // === XML Attributes ===
    #[serde(rename = "@id", skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
    #[serde(rename = "@source", skip_serializing_if = "Option::is_none", default)]
    pub source: Option<String>,

    // === Core fields ===
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sortname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,

    // === Media: images ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marquee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fanart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titleshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boxback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boxart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cartridge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wheel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual: Option<String>,

    // === Media: video ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<String>,

    // === Metadata ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub releasedate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genreid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub players: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arcadesystemname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,

    // === Boolean flags (ES stores as "true"/"false" strings) ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kidgame: Option<String>,

    // === Play stats ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playcount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastplayed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gametime: Option<String>,

    // === Emulation overrides ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emulator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub core: Option<String>,

    // === Hashes ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crc32: Option<String>,

    // === RetroAchievements ===
    #[serde(rename = "cheevosHash", skip_serializing_if = "Option::is_none", default)]
    pub cheevos_hash: Option<String>,
    #[serde(rename = "cheevosId", skip_serializing_if = "Option::is_none", default)]
    pub cheevos_id: Option<String>,
}
