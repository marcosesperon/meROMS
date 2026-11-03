use std::fs;

use serde::Serialize;

use crate::rename_rules::{default_rules, CompiledRules, RenameRule, RuleError, RulesFile};

/// Resultado de probar las reglas sobre un nombre de ejemplo (pestaña «Reglas» de ajustes).
#[derive(Debug, Serialize)]
pub struct RuleTestResult {
    pub result: String,
    pub errors: Vec<RuleError>,
}

#[tauri::command]
pub fn get_default_rename_rules() -> Vec<RenameRule> {
    default_rules()
}

#[tauri::command]
pub fn test_rename_rules(name: String, rules: Vec<RenameRule>) -> RuleTestResult {
    let compiled = CompiledRules::new(&rules);
    RuleTestResult { result: compiled.apply(&name), errors: compiled.errors }
}

/// Importa un rules.json (el formato de ES-DE-Roms-Renamer o uno exportado por meROMS).
#[tauri::command]
pub fn import_rename_rules(file_path: String) -> Result<Vec<RenameRule>, String> {
    let content = fs::read_to_string(&file_path).map_err(|e| format!("Failed to read rules file: {}", e))?;
    let file: RulesFile = serde_json::from_str(&content).map_err(|e| format!("Invalid rules file: {}", e))?;
    Ok(file.rules)
}

/// Exporta en el formato rules.json original. Solo van las reglas activas y sin el campo
/// «enabled», porque el script original las aplicaría todas aunque estuvieran apagadas.
#[tauri::command]
pub fn export_rename_rules(rules: Vec<RenameRule>, dest_path: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct OriginalRule<'a> {
        pattern: &'a str,
        replacement: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<&'a str>,
    }
    #[derive(Serialize)]
    struct OriginalFile<'a> {
        rules: Vec<OriginalRule<'a>>,
    }

    let file = OriginalFile {
        rules: rules
            .iter()
            .filter(|r| r.enabled)
            .map(|r| OriginalRule {
                pattern: &r.pattern,
                replacement: &r.replacement,
                description: r.description.as_deref(),
            })
            .collect(),
    };
    let content = serde_json::to_string_pretty(&file).map_err(|e| format!("Failed to serialize rules: {}", e))?;
    fs::write(&dest_path, content).map_err(|e| format!("Failed to write rules file: {}", e))
}
