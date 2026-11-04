use serde::{Deserialize, Serialize};

use super::profile::Profile;
use crate::rename_rules::{default_rules, RenameRule};

/// Per scraper media type: which gamelist field it fills (Classic structure only).
/// Folder and file name are fixed now (`<ES-DE folder>/<ROM name>.<ext>`); `subfolder` and
/// `suffix` are kept only as legacy hints so the media normalizer can find old files.
/// The frontend sends camelCase; snake_case aliases keep older settings files loadable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPathConfig {
    #[serde(alias = "media_type")]
    pub media_type: String,
    #[serde(default)]
    pub subfolder: String,
    #[serde(default)]
    pub suffix: String,
    #[serde(alias = "game_field")]
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
    // ROM name cleanup rules (None = built-in defaults)
    #[serde(default)]
    pub rename_rules: Option<Vec<RenameRule>>,
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
                // Migrated once: otherwise deleting every profile would bring this one back
                self.rom_root_path = None;
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

    /// Configured rename rules, or the built-in ones if the user never changed them.
    pub fn rename_rules_or_default(&self) -> Vec<RenameRule> {
        self.rename_rules.clone().unwrap_or_else(default_rules)
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
        self.ss_dev_credentials().is_some()
    }

    /// Developer credentials for ScreenScraper: the user's (Settings → Advanced) if both are
    /// set, otherwise the ones built into the app (see build.rs), if any.
    pub fn ss_dev_credentials(&self) -> Option<(String, String)> {
        let filled = |v: &Option<String>| v.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
        match (filled(&self.ss_devid), filled(&self.ss_devpassword)) {
            (Some(id), Some(password)) => Some((id, password)),
            _ => built_in_ss_credentials(),
        }
    }

    pub fn has_tgdb(&self) -> bool {
        self.tgdb_apikey.as_ref().is_some_and(|s| !s.is_empty())
    }

    /// Gamelist field a scraper media type fills in the Classic structure:
    /// the user's choice if configured, otherwise meROMS' historical default.
    pub fn media_field_for(&self, media_type: &str) -> Option<String> {
        if let Some(cfg) = self
            .media_paths
            .as_ref()
            .and_then(|paths| paths.iter().find(|p| p.media_type == media_type))
        {
            return Some(cfg.game_field.clone());
        }
        let field = match media_type {
            "box-2D" | "box-3D" | "fanart" | "mixrbv1" | "mixrbv2" => "image",
            "ss" | "sstitle" => "thumbnail",
            "marquee" | "wheel" | "wheel-hd" | "screenmarquee" => "marquee",
            "video" => "video",
            _ => return None,
        };
        Some(field.to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn user_developer_credentials_win_and_need_both_fields() {
        let mut s = super::AppSettings::default();
        s.ss_devid = Some("mio".into());
        s.ss_devpassword = Some("clave".into());
        assert_eq!(s.ss_dev_credentials(), Some(("mio".into(), "clave".into())));
        // Solo uno de los dos: se usan los de la app (si los lleva)
        s.ss_devpassword = Some("  ".into());
        assert_eq!(s.ss_dev_credentials(), super::built_in_ss_credentials());
    }

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
        assert!(settings.rom_root_path.is_none());

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
    fn media_paths_accept_camel_and_snake_case() {
        let camel = r#"{"media_paths":[{"mediaType":"ss","subfolder":"x","suffix":"","gameField":"image"}]}"#;
        let snake = r#"{"media_paths":[{"media_type":"ss","game_field":"image"}]}"#;
        for json in [camel, snake] {
            let settings: AppSettings = serde_json::from_str(json).unwrap();
            assert_eq!(settings.media_field_for("ss").as_deref(), Some("image"));
        }
        // Not configured: historical default
        assert_eq!(AppSettings::default().media_field_for("ss").as_deref(), Some("thumbnail"));
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

/// ScreenScraper developer credentials built into this copy of the app (None if it was
/// compiled without them).
pub fn built_in_ss_credentials() -> Option<(String, String)> {
    match (option_env!("MEROMS_SS_DEVID"), option_env!("MEROMS_SS_DEVPASSWORD")) {
        (Some(id), Some(password)) if !id.is_empty() && !password.is_empty() => Some((id.to_string(), password.to_string())),
        _ => None,
    }
}
