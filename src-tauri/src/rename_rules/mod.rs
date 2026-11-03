//! Limpieza de nombres de ROM con reglas de expresiones regulares, al estilo de
//! ES-DE-Roms-Renamer (https://github.com/mirai-toto/ES-DE-Roms-Renamer, licencia MIT).
//!
//! Las reglas de serie (`default_rules.json`) son las del `rules.json` original con dos cambios:
//! - En Rust `$1` sí funciona como referencia a grupo (en el script Python original no,
//!   y «Title, The» acababa como «The $1»).
//! - La regla «(Disc x) → .partx» viene desactivada para no romper la detección multidisco.

use regex::Regex;
use serde::{Deserialize, Serialize};

const DEFAULT_RULES_JSON: &str = include_str!("default_rules.json");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenameRule {
    pub pattern: String,
    #[serde(default)]
    pub replacement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Los ficheros rules.json originales no tienen este campo: entonces la regla está activa
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

/// Formato del fichero rules.json (el del proyecto original): `{ "rules": [...] }`
#[derive(Debug, Serialize, Deserialize)]
pub struct RulesFile {
    pub rules: Vec<RenameRule>,
}

/// Regla con su patrón inválido (para enseñar el error al lado de la regla).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuleError {
    pub index: usize,
    pub message: String,
}

pub fn default_rules() -> Vec<RenameRule> {
    serde_json::from_str::<RulesFile>(DEFAULT_RULES_JSON)
        .map(|f| f.rules)
        .unwrap_or_default()
}

/// Compila las reglas activas una sola vez (para aplicarlas a cientos de ROMs).
/// Las que tienen un patrón inválido se saltan y se devuelven aparte.
pub struct CompiledRules {
    rules: Vec<(Regex, String)>,
    pub errors: Vec<RuleError>,
}

impl CompiledRules {
    pub fn new(rules: &[RenameRule]) -> Self {
        let mut compiled = Vec::new();
        let mut errors = Vec::new();
        for (index, rule) in rules.iter().enumerate().filter(|(_, r)| r.enabled) {
            match Regex::new(&rule.pattern) {
                Ok(re) => compiled.push((re, rule.replacement.clone())),
                Err(e) => errors.push(RuleError { index, message: e.to_string() }),
            }
        }
        CompiledRules { rules: compiled, errors }
    }

    /// Aplica las reglas en orden a un nombre (sin extensión) y lo deja limpio.
    pub fn apply(&self, name: &str) -> String {
        let mut result = name.to_string();
        for (re, replacement) in &self.rules {
            result = re.replace_all(&result, replacement.as_str()).trim().to_string();
        }
        tidy_spaces(&result)
    }
}

/// Igual que el script original: quita espacios repetidos y los de delante de «.partN».
fn tidy_spaces(name: &str) -> String {
    let collapsed = name.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.replace(" .part", ".part")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(name: &str) -> String {
        CompiledRules::new(&default_rules()).apply(name)
    }

    #[test]
    fn las_reglas_de_serie_cargan_y_compilan() {
        let rules = default_rules();
        assert!(rules.len() > 30);
        assert!(CompiledRules::new(&rules).errors.is_empty());
        // La de los discos viene apagada
        assert!(rules.iter().any(|r| r.pattern.contains("Disc") && !r.enabled));
    }

    #[test]
    fn quita_etiquetas_de_region_idioma_y_version() {
        assert_eq!(clean("Super Juego (USA)"), "Super Juego");
        assert_eq!(clean("Super Juego (Europe) (En,Fr,De) (Rev 1)"), "Super Juego");
        assert_eq!(clean("Super Juego (v1.2.3) [!]"), "Super Juego");
        assert_eq!(clean("Super  Juego   (1993-10-04)"), "Super Juego");
    }

    #[test]
    fn reordena_el_articulo_ingles() {
        // En el script Python original esto daba «The $1»
        assert_eq!(clean("Legend of Juego, The (USA)"), "The Legend of Juego");
    }

    #[test]
    fn respeta_los_discos_por_defecto() {
        assert_eq!(clean("Juego Largo (USA) (Disc 1)"), "Juego Largo (Disc 1)");
    }

    #[test]
    fn la_regla_de_discos_activada_usa_part() {
        let mut rules = default_rules();
        rules.iter_mut().filter(|r| r.pattern.contains("Disc")).for_each(|r| r.enabled = true);
        assert_eq!(CompiledRules::new(&rules).apply("Juego Largo (USA) (Disc 2)"), "Juego Largo.part2");
    }

    #[test]
    fn patron_invalido_se_informa_y_no_rompe_el_resto() {
        let rules = vec![
            RenameRule { pattern: "(".into(), replacement: "".into(), description: None, enabled: true },
            RenameRule { pattern: r"\(USA\)".into(), replacement: "".into(), description: None, enabled: true },
        ];
        let compiled = CompiledRules::new(&rules);
        assert_eq!(compiled.errors.len(), 1);
        assert_eq!(compiled.errors[0].index, 0);
        assert_eq!(compiled.apply("Juego (USA)"), "Juego");
    }

    #[test]
    fn lee_el_formato_rules_json_original() {
        // Sin campo «enabled»: activas
        let json = r#"{"rules":[{"pattern":"\\(USA\\)","replacement":"","description":"Quitar USA"}]}"#;
        let file: RulesFile = serde_json::from_str(json).unwrap();
        assert!(file.rules[0].enabled);
    }
}
