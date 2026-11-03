use serde::{Deserialize, Serialize};

use super::profile::Profile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPathConfig {
    pub media_type: String,
    pub subfolder: String,
    pub suffix: String,
    pub game_field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    // Legacy single ROM root (pre-profiles). Only read to migrate into a default profile.
    pub rom_root_path: Option<String>,
    // Saved library profiles and the one currently in use
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub active_profile_id: Option<String>,
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
    /// Migrate pre-profiles settings: if there are no profiles but a legacy `rom_root_path`,
    /// create a default Classic profile from it and make it active.
    /// Also repairs a missing/dangling `active_profile_id`. Returns true if anything changed.
    pub fn migrate_legacy(&mut self) -> bool {
        let mut changed = false;

        if self.profiles.is_empty() {
            if let Some(root) = self.rom_root_path.as_deref().filter(|r| !r.is_empty()) {
                let profile = Profile::default_classic(root, self.language.as_deref());
                self.active_profile_id = Some(profile.id.clone());
                self.profiles.push(profile);
                changed = true;
            }
        }

        let active_is_valid = self
            .active_profile_id
            .as_ref()
            .is_some_and(|id| self.profiles.iter().any(|p| &p.id == id));
        if !active_is_valid {
            let first_id = self.profiles.first().map(|p| p.id.clone());
            if self.active_profile_id != first_id {
                self.active_profile_id = first_id;
                changed = true;
            }
        }

        changed
    }

    /// The profile currently in use, if any.
    pub fn active_profile(&self) -> Option<&Profile> {
        let id = self.active_profile_id.as_ref()?;
        self.profiles.iter().find(|p| &p.id == id)
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::StructureKind;

    #[test]
    fn legacy_root_becomes_active_classic_profile() {
        let mut settings = AppSettings {
            rom_root_path: Some("/ficticio/ROMs".into()),
            language: Some("es".into()),
            ..Default::default()
        };

        assert!(settings.migrate_legacy());
        let active = settings.active_profile().unwrap();
        assert_eq!(active.kind, StructureKind::Classic);
        assert_eq!(active.rom_root_path, "/ficticio/ROMs");
        assert_eq!(active.name, "Clásico");

        // Running it again changes nothing
        assert!(!settings.migrate_legacy());
        assert_eq!(settings.profiles.len(), 1);
    }

    #[test]
    fn no_legacy_root_means_no_profile() {
        let mut settings = AppSettings::default();
        assert!(!settings.migrate_legacy());
        assert!(settings.profiles.is_empty());
        assert!(settings.active_profile().is_none());
    }

    #[test]
    fn dangling_active_id_falls_back_to_first_profile() {
        let mut settings = AppSettings {
            profiles: vec![Profile::default_classic("/ficticio/ROMs", Some("en"))],
            active_profile_id: Some("borrado".into()),
            ..Default::default()
        };
        assert!(settings.migrate_legacy());
        assert_eq!(settings.active_profile().unwrap().name, "Classic");
    }

    #[test]
    fn old_settings_json_still_loads() {
        // Settings files written before profiles existed have no profiles fields
        let json = r#"{"rom_root_path":"/ficticio/ROMs","language":"en"}"#;
        let mut settings: AppSettings = serde_json::from_str(json).unwrap();
        assert!(settings.profiles.is_empty());
        settings.migrate_legacy();
        assert_eq!(settings.profiles.len(), 1);
    }
}
