use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPathConfig {
    pub media_type: String,
    pub subfolder: String,
    pub suffix: String,
    pub game_field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub rom_root_path: Option<String>,
    // ScreenScraper credentials
    pub ss_devid: Option<String>,
    pub ss_devpassword: Option<String>,
    pub ss_user: Option<String>,
    pub ss_password: Option<String>,
    // Media types to download (e.g., ["box-2D", "ss", "marquee"])
    pub ss_media_types: Option<Vec<String>>,
    // TheGamesDB
    pub tgdb_apikey: Option<String>,
    // Media path configuration
    pub media_paths: Option<Vec<MediaPathConfig>>,
    // Which game media field to display in grid view (e.g., "image", "thumbnail", "mix")
    pub grid_media_field: Option<String>,
    // App language: "es", "en", etc.
    pub language: Option<String>,
    // Per-system configuration keyed by normalized system name
    pub system_configs: Option<std::collections::HashMap<String, SystemConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemConfig {
    pub aspect_ratio: Option<String>,
}

impl AppSettings {
    pub fn default_media_types() -> Vec<String> {
        vec![
            "box-2D".to_string(),
            "ss".to_string(),
            "marquee".to_string(),
        ]
    }

    pub fn media_types(&self) -> Vec<String> {
        self.ss_media_types
            .clone()
            .unwrap_or_else(Self::default_media_types)
    }

    pub fn has_screenscraper(&self) -> bool {
        self.ss_devid.as_ref().is_some_and(|s| !s.is_empty())
            && self.ss_devpassword.as_ref().is_some_and(|s| !s.is_empty())
    }

    pub fn has_tgdb(&self) -> bool {
        self.tgdb_apikey.as_ref().is_some_and(|s| !s.is_empty())
    }

    /// Get the media path config for a given scraper media type.
    /// Falls back to ES-DE defaults if not configured.
    pub fn media_path_for(&self, media_type: &str) -> (String, String, String) {
        if let Some(paths) = &self.media_paths {
            if let Some(cfg) = paths.iter().find(|p| p.media_type == media_type) {
                return (
                    cfg.subfolder.clone(),
                    cfg.suffix.clone(),
                    cfg.game_field.clone(),
                );
            }
        }
        // ES-DE defaults
        match media_type {
            "box-2D" => ("covers".into(), "".into(), "image".into()),
            "box-3D" => ("3dboxes".into(), "".into(), "image".into()),
            "ss" => ("screenshots".into(), "".into(), "thumbnail".into()),
            "sstitle" => ("titlescreens".into(), "".into(), "thumbnail".into()),
            "marquee" | "wheel" | "wheel-hd" | "screenmarquee" => {
                ("marquees".into(), "".into(), "marquee".into())
            }
            "fanart" => ("fanart".into(), "".into(), "image".into()),
            "mixrbv1" | "mixrbv2" => ("miximages".into(), "".into(), "image".into()),
            "video" => ("videos".into(), "".into(), "video".into()),
            _ => ("images".into(), "".into(), "image".into()),
        }
    }
}
