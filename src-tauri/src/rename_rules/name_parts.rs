//! Partes del nombre de una ROM para la pantalla de Renombrar: el nombre en sí («Super Mario
//! World») y las etiquetas de detrás («(USA) (En,Fr) (Rev 1) [!]»). Las reglas de limpieza
//! trabajan sobre el nombre, y las casillas de «qué etiquetas se quedan» sobre las etiquetas.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::commands::best_version::{is_language, region_codes, UNWANTED};

/// Cada etiqueta entre paréntesis o corchetes
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(([^()]*)\)|\[([^\[\]]*)\]").unwrap());

/// Disco o cara de un juego de varios: no se quita nunca (rompería los .m3u y la detección multidisco)
static DISC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(disc|disk|cd|side|tape|part)\s*\S+").unwrap());

/// Qué tipos de etiqueta se quedan en el nombre de fichero (todo sí = como venga de la fuente)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TagKeep {
    /// (USA), (Europe), (Japan, USA)…
    pub region: bool,
    /// (En,Fr,De)…
    pub language: bool,
    /// (Rev 1), (v1.1)…
    pub revision: bool,
    /// (Beta), (Proto), (Demo), (Hack), (Unl), (Translated Es)…
    pub status: bool,
    /// Lo que va entre corchetes: [!], [b1], [h]…
    pub dump: bool,
    /// Cualquier otro paréntesis: (PC10), (Virtual Console)…
    pub other: bool,
}

impl Default for TagKeep {
    fn default() -> Self {
        TagKeep { region: true, language: true, revision: true, status: true, dump: true, other: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    Disc,
    Region,
    Language,
    Revision,
    Status,
    Dump,
    Other,
}

/// De qué tipo es una etiqueta (`inner` sin los paréntesis/corchetes)
pub fn tag_kind(inner: &str, square: bool) -> TagKind {
    let t = inner.trim();
    if DISC.is_match(t) {
        return TagKind::Disc;
    }
    if square {
        return TagKind::Dump;
    }
    let parts: Vec<&str> = t.split(',').map(str::trim).collect();
    if parts.iter().all(|p| !region_codes(p).is_empty()) {
        return TagKind::Region;
    }
    if parts.iter().all(|p| is_language(p)) {
        return TagKind::Language;
    }
    let lower = t.to_lowercase();
    let is_version = lower.strip_prefix('v').is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit()));
    if lower.starts_with("rev ") || lower.starts_with("version ") || is_version {
        return TagKind::Revision;
    }
    let first = lower.split_whitespace().next().unwrap_or("");
    if UNWANTED.contains(&first) || first == "translated" || first == "tr" {
        return TagKind::Status;
    }
    TagKind::Other
}

/// Separa «Juego (USA) [!]» en «Juego» y «(USA) [!]». Una etiqueta al principio («[BIOS] X»)
/// cuenta como parte del nombre: solo se separan las que van detrás de algo de texto.
pub fn split(stem: &str) -> (&str, &str) {
    match TAG.find_iter(stem).find(|m| !stem[..m.start()].trim().is_empty()) {
        Some(m) => (stem[..m.start()].trim_end(), &stem[m.start()..]),
        None => (stem.trim_end(), ""),
    }
}

/// Las etiquetas (y el texto suelto entre ellas, en su sitio) sin las de los tipos desmarcados
pub fn filter_tags(tags: &str, keep: &TagKeep) -> String {
    let kept = TAG.replace_all(tags, |c: &regex::Captures| {
        let (inner, square) = match (c.get(1), c.get(2)) {
            (Some(p), _) => (p.as_str(), false),
            (_, Some(b)) => (b.as_str(), true),
            _ => return c[0].to_string(),
        };
        let stays = match tag_kind(inner, square) {
            TagKind::Disc => true,
            TagKind::Region => keep.region,
            TagKind::Language => keep.language,
            TagKind::Revision => keep.revision,
            TagKind::Status => keep.status,
            TagKind::Dump => keep.dump,
            TagKind::Other => keep.other,
        };
        if stays { c[0].to_string() } else { String::new() }
    });
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Región de la gamelist (`us`, `jp`, `wr`…, como la escriben los scrapers) → región de No-Intro
fn gamelist_region(code: &str) -> Option<&'static str> {
    Some(match code.trim().to_lowercase().as_str() {
        "us" | "usa" => "USA",
        "eu" | "europe" => "Europe",
        "jp" | "japan" => "Japan",
        "wr" | "wor" | "world" => "World",
        "cn" | "china" => "China",
        "kr" | "korea" => "Korea",
        "asi" | "asia" => "Asia",
        "fr" | "france" => "France",
        "de" | "germany" => "Germany",
        "es" | "sp" | "spain" => "Spain",
        "it" | "italy" => "Italy",
        "uk" => "UK",
        "nl" | "netherlands" => "Netherlands",
        "se" | "sweden" => "Sweden",
        "pt" | "portugal" => "Portugal",
        "br" | "brazil" => "Brazil",
        "au" | "australia" => "Australia",
        "ca" | "canada" => "Canada",
        "ru" | "russia" => "Russia",
        _ => return None,
    })
}

/// Idioma de la gamelist (`en`, `jp`, `cn`…) → código de No-Intro (`En`, `Ja`, `Zh`…)
fn gamelist_language(code: &str) -> Option<&'static str> {
    Some(match code.trim().to_lowercase().as_str() {
        "en" => "En",
        "ja" | "jp" => "Ja",
        "fr" => "Fr",
        "de" => "De",
        "es" | "sp" => "Es",
        "it" => "It",
        "pt" => "Pt",
        "nl" => "Nl",
        "sv" | "se" => "Sv",
        "zh" | "cn" => "Zh",
        "ko" | "kr" => "Ko",
        "ru" => "Ru",
        "pl" => "Pl",
        "da" => "Da",
        "fi" => "Fi",
        "no" => "No",
        "el" => "El",
        "tr" => "Tr",
        _ => return None,
    })
}

/// Los códigos reconocidos de una lista «us, eu» o «jp,fr», traducidos y sin repetir
fn translate_list(list: Option<&str>, translate: fn(&str) -> Option<&'static str>) -> Vec<&'static str> {
    let mut out = Vec::new();
    for code in list.unwrap_or("").split([',', ';', '/']) {
        if let Some(t) = translate(code) {
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    out
}

/// Fuente «Datos de la gamelist»: el título con su región e idiomas al estilo No-Intro
/// («Super Mario World» + us + en → «Super Mario World (USA) (En)»). Lo que no se reconoce no se
/// pone, y si el título ya lleva región o idiomas entre paréntesis, no se repiten.
pub fn gamelist_name(title: &str, region: Option<&str>, lang: Option<&str>) -> String {
    let (_, tags) = split(title);
    let has = |kind: TagKind| {
        TAG.captures_iter(tags).any(|c| c.get(1).is_some_and(|p| tag_kind(p.as_str(), false) == kind))
    };
    let mut name = title.trim().to_string();
    let regions = translate_list(region, gamelist_region);
    if !regions.is_empty() && !has(TagKind::Region) {
        name.push_str(&format!(" ({})", regions.join(", ")));
    }
    let languages = translate_list(lang, gamelist_language);
    if !languages.is_empty() && !has(TagKind::Language) {
        name.push_str(&format!(" ({})", languages.join(",")));
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_and_its_tags_are_told_apart() {
        assert_eq!(split("Super Mario World (USA) [!]"), ("Super Mario World", "(USA) [!]"));
        assert_eq!(split("Donkey Kong"), ("Donkey Kong", ""));
        assert_eq!(split("[BIOS] PlayStation (USA)"), ("[BIOS] PlayStation", "(USA)"));
    }

    #[test]
    fn each_tag_has_its_kind() {
        for (tag, square, kind) in [
            ("USA", false, TagKind::Region),
            ("Japan, USA", false, TagKind::Region),
            ("En,Fr,De", false, TagKind::Language),
            ("Rev 1", false, TagKind::Revision),
            ("v1.1", false, TagKind::Revision),
            ("Beta 2", false, TagKind::Status),
            ("Proto", false, TagKind::Status),
            ("Translated Es", false, TagKind::Status),
            ("!", true, TagKind::Dump),
            ("b1", true, TagKind::Dump),
            ("Disc 2", false, TagKind::Disc),
            ("PC10", false, TagKind::Other),
            ("Virtual Console", false, TagKind::Other),
        ] {
            assert_eq!(tag_kind(tag, square), kind, "{}", tag);
        }
    }

    #[test]
    fn gamelist_data_becomes_no_intro_tags() {
        assert_eq!(gamelist_name("Super Mario World", Some("us"), Some("en")), "Super Mario World (USA) (En)");
        assert_eq!(gamelist_name("Gradius", Some("jp"), Some("jp,fr")), "Gradius (Japan) (Ja,Fr)");
        assert_eq!(gamelist_name("Tetris", Some("wr"), None), "Tetris (World)");
        assert_eq!(gamelist_name("Sonic", Some("us, eu"), Some("")), "Sonic (USA, Europe)");
        // Lo que no se reconoce no se pone; sin nada, solo el título
        assert_eq!(gamelist_name("Juego", Some("xx"), Some("??")), "Juego");
        assert_eq!(gamelist_name("Juego", None, None), "Juego");
        // El título ya trae la región: no se repite (el idioma sí se añade)
        assert_eq!(gamelist_name("Zelda (USA)", Some("us"), Some("en")), "Zelda (USA) (En)");
    }

    #[test]
    fn unchecked_kinds_leave_the_file_name_but_discs_stay() {
        let keep = TagKeep { language: false, dump: false, status: false, ..Default::default() };
        assert_eq!(filter_tags("(USA) (En,Fr) (Rev 1) (Beta) [!]", &keep), "(USA) (Rev 1)");
        let none = TagKeep { region: false, language: false, revision: false, status: false, dump: false, other: false };
        assert_eq!(filter_tags("(Europe) (Disc 2) [b1]", &none), "(Disc 2)");
        assert_eq!(filter_tags("(USA) (Rev 1)", &TagKeep::default()), "(USA) (Rev 1)");
    }
}
