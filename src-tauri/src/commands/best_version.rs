//! «Mejor versión» (1G1R: un juego, una ROM): agrupa las versiones de un mismo juego (por el
//! DAT si lo hay; si no, por el nombre sin etiquetas) y propone la preferida de cada grupo según
//! las etiquetas de No-Intro/Redump del nombre del fichero y el orden de regiones e idiomas de
//! Ajustes. Qué hacer con las demás (ocultar, mover, papelera) lo decide la ventana.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::models::Game;

/// Lo que dicen las etiquetas del nombre de una ROM
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct VersionTags {
    /// Nombre sin etiquetas, normalizado (para agrupar sin DAT)
    #[serde(skip)]
    pub base: String,
    /// Códigos de región de Ajustes (us, eu, jp, wor, es…); un país europeo cuenta también como «eu»
    pub regions: Vec<String>,
    /// Códigos de idioma (en, es, fr…)
    pub languages: Vec<String>,
    /// Revisión: «Rev 2» → 2, «Rev A» → 1, «v1.1» → 11; 0 si no tiene
    pub revision: u32,
    /// Beta, Proto, Demo, Sample, Pirate, Hack, Unl o mal volcado [b]
    pub unwanted: bool,
    /// Verificada [!]
    pub verified: bool,
}

/// Región de No-Intro → códigos de Ajustes (el primero, el propio; luego la zona)
fn region_codes(name: &str) -> &'static [&'static str] {
    match name.trim().to_lowercase().as_str() {
        "world" => &["wor"],
        "usa" | "us" => &["us"],
        "europe" | "eu" => &["eu"],
        "japan" | "jp" => &["jp"],
        "spain" => &["es", "eu"],
        "france" => &["fr", "eu"],
        "germany" => &["de", "eu"],
        "italy" => &["it", "eu"],
        "uk" | "united kingdom" => &["uk", "eu"],
        "netherlands" => &["nl", "eu"],
        "sweden" => &["se", "eu"],
        "portugal" => &["pt", "eu"],
        "australia" => &["au"],
        "brazil" => &["br"],
        "canada" => &["ca", "us"],
        "korea" => &["kr", "asi"],
        "china" => &["cn", "asi"],
        "asia" => &["asi"],
        "russia" => &["ru"],
        _ => &[],
    }
}

const UNWANTED: &[&str] = &["beta", "proto", "prototype", "demo", "sample", "pirate", "hack", "unl", "kiosk", "debug"];

/// Lee las etiquetas del nombre del fichero: «Juego (USA, Europe) (En,Fr) (Rev 1) [!]»
pub fn parse_tags(file_stem: &str) -> VersionTags {
    let mut tags = VersionTags::default();
    let mut base = String::new();
    let mut chars = file_stem.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '(' || c == '[' {
            let close = if c == '(' { ')' } else { ']' };
            let mut inner = String::new();
            for d in chars.by_ref() {
                if d == close {
                    break;
                }
                inner.push(d);
            }
            read_tag(&inner, c == '[', &mut tags);
        } else {
            base.push(c);
        }
    }
    tags.base = super::scrape_common::normalize_title(&base);
    tags
}

fn read_tag(inner: &str, square: bool, tags: &mut VersionTags) {
    let t = inner.trim();
    let lower = t.to_lowercase();
    if square {
        match lower.as_str() {
            "!" => tags.verified = true,
            s if s == "b" || s.starts_with("b ") || s.starts_with("b1") || s.starts_with("b2") => tags.unwanted = true,
            s if s.starts_with('h') || s.starts_with('t') && s.len() <= 3 || s.starts_with('p') && s.len() <= 3 => tags.unwanted = true,
            _ => {}
        }
        return;
    }
    // Regiones: «USA, Europe»
    let parts: Vec<&str> = t.split(',').map(str::trim).collect();
    let codes: Vec<&str> = parts.iter().flat_map(|p| region_codes(p).iter().copied()).collect();
    if !codes.is_empty() && parts.iter().all(|p| !region_codes(p).is_empty()) {
        for c in codes {
            if !tags.regions.iter().any(|r| r == c) {
                tags.regions.push(c.to_string());
            }
        }
        return;
    }
    // Revisión: «Rev 2», «Rev A», «v1.1»
    if let Some(rev) = lower.strip_prefix("rev ") {
        tags.revision = rev.parse::<u32>().unwrap_or_else(|_| rev.chars().next().map_or(0, |c| (c as u32).saturating_sub('a' as u32) + 1));
        return;
    }
    if let Some(v) = lower.strip_prefix('v').filter(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit())) {
        tags.revision = v.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0);
        return;
    }
    if UNWANTED.iter().any(|u| lower == *u || lower.starts_with(&format!("{} ", u))) {
        tags.unwanted = true;
        return;
    }
    // Idiomas: «En,Fr,De» (exactamente «Xx» o «Xx-YY» cada uno)
    if parts.iter().all(|p| is_language(p)) {
        tags.languages.extend(parts.iter().map(|p| p[..2].to_lowercase()));
    }
}

/// «En», «Fr», «En-US»: mayúscula + minúscula, opcionalmente «-» y dos mayúsculas
fn is_language(p: &str) -> bool {
    let c: Vec<char> = p.chars().collect();
    let two = c.len() >= 2 && c[0].is_ascii_uppercase() && c[1].is_ascii_lowercase();
    two && (c.len() == 2 || (c.len() == 5 && c[2] == '-' && c[3].is_ascii_uppercase() && c[4].is_ascii_uppercase()))
}

/// Puntuación para ordenar (menor = mejor): (no deseada, región, idioma, -revisión, no verificada)
fn score(tags: &VersionTags, regions: &[String], languages: &[String]) -> (bool, usize, usize, i64, bool) {
    let region = tags.regions.iter().filter_map(|r| regions.iter().position(|p| p == r)).min().unwrap_or(usize::MAX);
    // Sin etiqueta de idioma: se supone el de su región (USA → en, Spain → es…)
    let langs: Vec<String> = if tags.languages.is_empty() {
        tags.regions.iter().filter_map(|r| region_language(r)).map(String::from).collect()
    } else {
        tags.languages.clone()
    };
    let language = langs.iter().filter_map(|l| languages.iter().position(|p| p == l)).min().unwrap_or(usize::MAX);
    (tags.unwanted, region, language, -(tags.revision as i64), !tags.verified)
}

fn region_language(code: &str) -> Option<&'static str> {
    Some(match code {
        "us" | "uk" | "au" | "wor" => "en",
        "jp" => "ja",
        "es" => "es",
        "fr" => "fr",
        "de" => "de",
        "it" => "it",
        "br" | "pt" => "pt",
        "nl" => "nl",
        "kr" => "ko",
        "cn" => "zh",
        _ => return None,
    })
}

/// Por qué ganó la elegida frente a la siguiente
fn reason(best: &(bool, usize, usize, i64, bool), next: Option<&(bool, usize, usize, i64, bool)>) -> &'static str {
    let Some(next) = next else { return "only" };
    if best.0 != next.0 {
        "not_unwanted"
    } else if best.1 != next.1 {
        "region"
    } else if best.2 != next.2 {
        "language"
    } else if best.3 != next.3 {
        "revision"
    } else if best.4 != next.4 {
        "verified"
    } else {
        "shortest"
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Version {
    pub game_path: String,
    pub name: String,
    pub file: String,
    pub tags: VersionTags,
    pub chosen: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionGroup {
    pub title: String,
    /// Por qué se propone la elegida: not_unwanted | region | language | revision | verified | shortest
    pub reason: String,
    pub versions: Vec<Version>,
}

/// Agrupa y elige. `dat_groups`: grupo del DAT de cada juego (si lo hay), por ruta.
fn group_versions(games: &[Game], dat_groups: &HashMap<String, String>, regions: &[String], languages: &[String]) -> Vec<VersionGroup> {
    let mut groups: HashMap<String, Vec<(&Game, String, VersionTags)>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for game in games {
        let file = Path::new(&game.path).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let stem = Path::new(&game.path).file_stem().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let tags = parse_tags(&stem);
        let key = match dat_groups.get(&game.path) {
            Some(g) => format!("dat:{}", g),
            None if !tags.base.is_empty() => format!("name:{}", tags.base),
            None => continue,
        };
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push((game, file, tags));
    }

    order
        .into_iter()
        .filter_map(|key| {
            let mut list = groups.remove(&key)?;
            if list.len() < 2 {
                return None;
            }
            list.sort_by(|a, b| {
                score(&a.2, regions, languages)
                    .cmp(&score(&b.2, regions, languages))
                    .then(a.1.len().cmp(&b.1.len()))
            });
            let best = score(&list[0].2, regions, languages);
            let next = list.get(1).map(|v| score(&v.2, regions, languages));
            let why = reason(&best, next.as_ref());
            let title = list[0].0.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| list[0].2.base.clone());
            Some(VersionGroup {
                title,
                reason: why.to_string(),
                versions: list
                    .into_iter()
                    .enumerate()
                    .map(|(i, (game, file, tags))| Version {
                        game_path: game.path.clone(),
                        name: game.name.clone().unwrap_or_else(|| file.clone()),
                        file,
                        tags,
                        chosen: i == 0,
                    })
                    .collect(),
            })
        })
        .collect()
}

/// Grupos de versiones del sistema, con la preferida propuesta
#[tauri::command(async)]
pub fn best_versions(app: tauri::AppHandle, system_path: String) -> Result<Vec<VersionGroup>, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;
    let games = super::gamelist::read_gamelist(&layout, &system)?.games;
    let dat_groups = super::dat::version_groups(&app, &system, &games)?;
    let mut groups = group_versions(&games, &dat_groups, &settings.scrape_regions(), &settings.scrape_languages());
    groups.sort_by_key(|g| g.title.to_lowercase());
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn reads_no_intro_tags() {
        let t = parse_tags("Sonic the Hedgehog (USA, Europe) (En,Fr,De) (Rev 1) [!]");
        assert_eq!(t.base, "sonic hedgehog");
        assert_eq!(t.regions, codes(&["us", "eu"]));
        assert_eq!(t.languages, codes(&["en", "fr", "de"]));
        assert_eq!(t.revision, 1);
        assert!(t.verified && !t.unwanted);

        let t = parse_tags("Super Mario World (Spain)");
        assert_eq!(t.regions, codes(&["es", "eu"]), "a European country also counts as Europe");
        assert!(parse_tags("Zelda (USA) (Beta)").unwanted);
        assert!(parse_tags("Zelda (Japan) [b]").unwanted);
        assert_eq!(parse_tags("Zelda (USA) (Rev A)").revision, 1);
        assert_eq!(parse_tags("Zelda (USA) (v1.1)").revision, 11);
    }

    fn g(path: &str) -> Game {
        Game { path: path.into(), ..Default::default() }
    }

    #[test]
    fn picks_by_unwanted_then_region_language_and_revision() {
        let eu = codes(&["eu", "wor", "us", "jp"]);
        let es = codes(&["es", "en"]);
        let games = vec![
            g("./Sonic (USA).md"),
            g("./Sonic (Europe).md"),
            g("./Sonic (Europe) (Beta).md"),
            g("./Sonic (Japan).md"),
            g("./Otro (USA).md"),
        ];
        let groups = group_versions(&games, &HashMap::new(), &eu, &es);
        assert_eq!(groups.len(), 1, "a game with a single version isn't listed");
        let chosen = groups[0].versions.iter().find(|v| v.chosen).unwrap();
        assert_eq!(chosen.game_path, "./Sonic (Europe).md");
        assert_eq!(groups[0].reason, "region");
        assert_eq!(groups[0].versions.last().unwrap().game_path, "./Sonic (Europe) (Beta).md", "the beta goes last");

        // Misma región: gana la revisión más nueva; si no, la verificada
        let games = vec![g("./Zelda (USA).sfc"), g("./Zelda (USA) (Rev 1).sfc")];
        let groups = group_versions(&games, &HashMap::new(), &eu, &es);
        assert_eq!(groups[0].versions[0].game_path, "./Zelda (USA) (Rev 1).sfc");
        assert_eq!(groups[0].reason, "revision");

        // Mismo puesto de región («World» y «USA» no; aquí dos europeas): gana la que tiene tu idioma
        let games = vec![g("./Mario (Europe) (En,Fr,De).sfc"), g("./Mario (Europe) (En,Es,It).sfc")];
        let groups = group_versions(&games, &HashMap::new(), &eu, &es);
        assert_eq!(groups[0].versions[0].game_path, "./Mario (Europe) (En,Es,It).sfc");
        assert_eq!(groups[0].reason, "language");
    }

    #[test]
    fn the_dat_groups_clones_whatever_their_names() {
        let games = vec![g("./sf2.zip"), g("./sf2ua.zip"), g("./ffight.zip")];
        let dat: HashMap<String, String> = [("./sf2.zip", "sf2"), ("./sf2ua.zip", "sf2"), ("./ffight.zip", "ffight")]
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let groups = group_versions(&games, &dat, &codes(&["eu"]), &codes(&["en"]));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].versions.len(), 2);
    }
}
