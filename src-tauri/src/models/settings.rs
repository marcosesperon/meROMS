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
    // Media types to download (e.g., ["box-2D", "ss", "marquee"]). Legacy: superseded by
    // `scrape_fields` (still read when that one isn't saved yet)
    pub ss_media_types: Option<Vec<String>>,
    // What the scraper may update, per data field and per media type
    #[serde(default)]
    pub scrape_fields: Option<ScrapePolicy>,
    // ScreenScraper region order (names, dates, media) and language order (texts), in
    // ScreenScraper codes; None = defaults from the app language
    #[serde(default)]
    pub ss_regions: Option<Vec<String>>,
    #[serde(default)]
    pub ss_languages: Option<Vec<String>>,
    // Table view: visible columns in order, with their widths (same for every system)
    #[serde(default)]
    pub table_columns: Option<Vec<TableColumn>>,
    // Last list view ("grid" | "table")
    #[serde(default)]
    pub last_view: Option<String>,
    // Grid order: name | most_played | recent | playtime | release | rating (None = name)
    #[serde(default)]
    pub grid_sort: Option<String>,
    // Click on a game: "panel" (show it in the right panel, None) | "full" (full-screen game page)
    #[serde(default)]
    pub click_action: Option<String>,
    // Donation prompt: unix seconds of the first launch, then of the last time it was shown
    #[serde(default)]
    pub donation_prompt_at: Option<i64>,
    // Look for a new version at start and once a day (None = yes)
    #[serde(default)]
    pub check_updates: Option<bool>,
    // Order of the <game> entries when a gamelist is written: "name" | "path" (None = as they are)
    #[serde(default)]
    pub gamelist_sort: Option<String>,
    // DeepL API key (the free plan works) to translate descriptions
    #[serde(default)]
    pub deepl_apikey: Option<String>,
    // «Export to device»: destinations used, each with its selection (managed by the UI)
    #[serde(default)]
    pub export_targets: Option<Vec<serde_json::Value>>,
    // «Compare with another folder»: the last folder compared
    #[serde(default)]
    pub compare_dest: Option<String>,
    // «Convert to CHD»: chdman chosen by hand (None = look in the PATH and the usual places)
    #[serde(default)]
    pub chdman_path: Option<String>,
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

/// One visible column of the table view
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableColumn {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
}

/// What the scraper does with one data field or media type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldPolicy {
    /// Always write what the scraper found
    Overwrite,
    /// Only if the game doesn't have it yet (media: only if the file is missing)
    IfEmpty,
    /// Leave it alone (media: don't download)
    Keep,
}

impl FieldPolicy {
    /// Apply a value found by the scraper to a field, following the policy
    pub fn apply(self, current: &mut Option<String>, found: Option<String>) {
        let Some(found) = found.filter(|v| !v.trim().is_empty()) else { return };
        let empty = current.as_deref().is_none_or(|v| v.trim().is_empty());
        if self == FieldPolicy::Overwrite || (self == FieldPolicy::IfEmpty && empty) {
            *current = Some(found);
        }
    }
}

/// Data fields the scraper fills
pub const SCRAPE_DATA_FIELDS: &[&str] = &["name", "desc", "genre", "developer", "publisher", "players", "rating", "releasedate"];
/// Media types the scraper can download (ScreenScraper ids; mirrors AVAILABLE_MEDIA_TYPES in the UI)
pub const SCRAPE_MEDIA_TYPES: &[&str] = &[
    "box-2D", "box-3D", "ss", "sstitle", "marquee", "wheel", "wheel-hd", "fanart", "screenmarquee", "mixrbv1", "mixrbv2", "video",
];

/// Per data field and per media type, what the scraper may update
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScrapePolicy {
    #[serde(default)]
    pub data: std::collections::BTreeMap<String, FieldPolicy>,
    #[serde(default)]
    pub media: std::collections::BTreeMap<String, FieldPolicy>,
}

impl ScrapePolicy {
    /// Data field: if not set, only when empty
    pub fn data(&self, field: &str) -> FieldPolicy {
        self.data.get(field).copied().unwrap_or(FieldPolicy::IfEmpty)
    }
    /// Media type: if not set, not downloaded
    pub fn media(&self, media_type: &str) -> FieldPolicy {
        self.media.get(media_type).copied().unwrap_or(FieldPolicy::Keep)
    }
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

    /// ScreenScraper regions in order of preference (saved, or by app language: English ->
    /// USA first, any other -> Europe first)
    pub fn scrape_regions(&self) -> Vec<String> {
        match self.ss_regions.as_ref().filter(|r| !r.is_empty()) {
            Some(saved) => saved.clone(),
            None => {
                let order: &[&str] = if self.app_language() == "en" { &["us", "wor", "eu", "jp", "ss"] } else { &["eu", "wor", "us", "jp", "ss"] };
                order.iter().map(|r| r.to_string()).collect()
            }
        }
    }

    /// ScreenScraper languages in order of preference (saved, or the app language then English)
    pub fn scrape_languages(&self) -> Vec<String> {
        match self.ss_languages.as_ref().filter(|l| !l.is_empty()) {
            Some(saved) => saved.clone(),
            None => {
                let lang = self.app_language();
                if lang == "en" { vec!["en".into()] } else { vec![lang.to_string(), "en".into()] }
            }
        }
    }

    fn app_language(&self) -> &str {
        self.language.as_deref().filter(|l| !l.is_empty()).unwrap_or("en")
    }

    /// Media types to download: those whose policy isn't «don't touch»
    pub fn media_types(&self) -> Vec<String> {
        let policy = self.scrape_policy();
        SCRAPE_MEDIA_TYPES
            .iter()
            .filter(|t| policy.media(t) != FieldPolicy::Keep)
            .map(|t| t.to_string())
            .collect()
    }

    /// What the scraper may update. Until it's saved, it comes from the old «media to
    /// download» list (checked → only if missing, unchecked → don't touch) and data «only if empty».
    pub fn scrape_policy(&self) -> ScrapePolicy {
        if let Some(saved) = &self.scrape_fields {
            return saved.clone();
        }
        let legacy = self.ss_media_types.clone().unwrap_or_else(Self::default_media_types);
        ScrapePolicy {
            data: SCRAPE_DATA_FIELDS.iter().map(|f| (f.to_string(), FieldPolicy::IfEmpty)).collect(),
            media: SCRAPE_MEDIA_TYPES
                .iter()
                .map(|t| {
                    let policy = if legacy.iter().any(|l| l == t) { FieldPolicy::IfEmpty } else { FieldPolicy::Keep };
                    (t.to_string(), policy)
                })
                .collect(),
        }
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
    use super::{FieldPolicy, ScrapePolicy};

    #[test]
    fn field_policies() {
        let mut v = Some("Mío".to_string());
        FieldPolicy::IfEmpty.apply(&mut v, Some("Scraper".into()));
        assert_eq!(v.as_deref(), Some("Mío"));
        FieldPolicy::Overwrite.apply(&mut v, Some("Scraper".into()));
        assert_eq!(v.as_deref(), Some("Scraper"));
        FieldPolicy::Keep.apply(&mut v, Some("Otro".into()));
        assert_eq!(v.as_deref(), Some("Scraper"));
        // Nada encontrado: no se borra
        FieldPolicy::Overwrite.apply(&mut v, None);
        assert_eq!(v.as_deref(), Some("Scraper"));
        let mut empty: Option<String> = Some("  ".into());
        FieldPolicy::IfEmpty.apply(&mut empty, Some("Nuevo".into()));
        assert_eq!(empty.as_deref(), Some("Nuevo"));
    }

    #[test]
    fn policy_comes_from_the_old_media_list_until_saved() {
        let mut s = super::AppSettings::default();
        s.ss_media_types = Some(vec!["box-2D".into(), "video".into()]);
        let p = s.scrape_policy();
        assert_eq!(p.media("box-2D"), FieldPolicy::IfEmpty);
        assert_eq!(p.media("video"), FieldPolicy::IfEmpty);
        assert_eq!(p.media("ss"), FieldPolicy::Keep);
        assert_eq!(p.data("name"), FieldPolicy::IfEmpty);
        assert_eq!(s.media_types(), vec!["box-2D".to_string(), "video".to_string()]);

        // Guardada: manda la nueva
        let mut saved = ScrapePolicy::default();
        saved.media.insert("ss".into(), FieldPolicy::Overwrite);
        s.scrape_fields = Some(saved);
        assert_eq!(s.media_types(), vec!["ss".to_string()]);
    }

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
