//! Lectura de la configuración de lanzamiento de ES-DE: `es_systems.xml` (los comandos de cada
//! sistema) y `es_find_rules.xml` (dónde buscar cada emulador y los cores de RetroArch).
//! meROMS lleva la copia del SO para el que se compila y, encima, aplica la personalizada del
//! usuario (`<ES-DE>/custom_systems/`), como hace ES-DE: lo que esté ahí sustituye a lo de la copia.

use std::collections::HashMap;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

#[cfg(target_os = "macos")]
const BUNDLED_SYSTEMS: &str = include_str!("../../resources/esde/macos/es_systems.xml");
#[cfg(target_os = "macos")]
const BUNDLED_FIND_RULES: &str = include_str!("../../resources/esde/macos/es_find_rules.xml");
#[cfg(target_os = "windows")]
const BUNDLED_SYSTEMS: &str = include_str!("../../resources/esde/windows/es_systems.xml");
#[cfg(target_os = "windows")]
const BUNDLED_FIND_RULES: &str = include_str!("../../resources/esde/windows/es_find_rules.xml");
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const BUNDLED_SYSTEMS: &str = include_str!("../../resources/esde/linux/es_systems.xml");
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const BUNDLED_FIND_RULES: &str = include_str!("../../resources/esde/linux/es_find_rules.xml");

/// Un emulador posible de un sistema: su nombre en ES-DE («Snes9x - Current») y el comando
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchCommand {
    pub label: String,
    pub command: String,
}

/// Un sistema de ES-DE: los comandos en su orden (el primero es el predeterminado)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SystemDef {
    pub name: String,
    pub full_name: String,
    pub commands: Vec<LaunchCommand>,
}

/// Tipos de regla de `es_find_rules.xml`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
    /// Rutas fijas (con «~», %ESPATH% y comodín «*» en el nombre del fichero)
    StaticPath,
    /// Nombres de ejecutable que se buscan en el PATH
    SystemPath,
    /// Carpetas donde buscar los cores de RetroArch
    CorePath,
    /// Windows: entrada «App Paths» del registro para ese ejecutable
    WinRegistryPath,
    /// Windows: valor del registro («clave\valor|resto de la ruta»)
    WinRegistryValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub kind: RuleKind,
    pub entries: Vec<String>,
}

/// Las reglas de búsqueda: de cada emulador y de cada tipo de core, por nombre (RETROARCH…)
#[derive(Debug, Clone, Default)]
pub struct FindRules {
    pub emulators: HashMap<String, Vec<Rule>>,
    pub cores: HashMap<String, Vec<Rule>>,
}

/// Toda la configuración: los sistemas por nombre de carpeta y las reglas de búsqueda
#[derive(Debug, Clone, Default)]
pub struct LaunchConfig {
    pub systems: HashMap<String, SystemDef>,
    pub rules: FindRules,
}

impl LaunchConfig {
    /// La copia de meROMS con la personalizada del usuario encima (`custom_dir` =
    /// `<ES-DE>/custom_systems`, si existe). Un fichero personalizado que no se puede leer se
    /// ignora: mejor lanzar con la copia que no lanzar.
    pub fn load(custom_dir: Option<&Path>) -> Result<Self, String> {
        let mut config = LaunchConfig {
            systems: parse_systems(BUNDLED_SYSTEMS)?,
            rules: parse_find_rules(BUNDLED_FIND_RULES)?,
        };
        if let Some(dir) = custom_dir {
            if let Ok(xml) = std::fs::read_to_string(dir.join("es_systems.xml")) {
                if let Ok(custom) = parse_systems(&xml) {
                    config.systems.extend(custom);
                }
            }
            if let Ok(xml) = std::fs::read_to_string(dir.join("es_find_rules.xml")) {
                if let Ok(custom) = parse_find_rules(&xml) {
                    config.rules.emulators.extend(custom.emulators);
                    config.rules.cores.extend(custom.cores);
                }
            }
        }
        Ok(config)
    }

    /// El sistema por el nombre de su carpeta (snes, mastersystem…; sin distinguir mayúsculas)
    pub fn system(&self, name: &str) -> Option<&SystemDef> {
        self.systems.get(&name.to_lowercase())
    }
}

/// Lee un `es_systems.xml`: cada `<system>` con su `<name>`, `<fullname>` y sus `<command label>`
pub fn parse_systems(xml: &str) -> Result<HashMap<String, SystemDef>, String> {
    let mut reader = Reader::from_str(xml);
    let mut systems = HashMap::new();
    let mut current: Option<SystemDef> = None;
    // Elemento cuyo texto se está leyendo y, en los <command>, su etiqueta
    let mut field: Option<(String, Option<String>)> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "system" => current = Some(SystemDef::default()),
                    "name" | "fullname" | "command" if current.is_some() => {
                        let label = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == b"label")
                            .map(|a| a.unescape_value().unwrap_or_default().to_string());
                        field = Some((tag, label));
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if let (Some(system), Some((tag, label))) = (current.as_mut(), field.as_ref()) {
                    let text = t.unescape().unwrap_or_default().trim().to_string();
                    match tag.as_str() {
                        "name" => system.name = text,
                        "fullname" => system.full_name = text,
                        "command" => {
                            let label = label.clone().unwrap_or_default();
                            system.commands.push(LaunchCommand { label, command: text });
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"system" => {
                    if let Some(system) = current.take() {
                        if !system.name.is_empty() {
                            systems.insert(system.name.to_lowercase(), system);
                        }
                    }
                }
                b"name" | b"fullname" | b"command" => field = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Failed to parse es_systems.xml: {}", e)),
            _ => {}
        }
    }
    // Sin etiqueta, ES-DE llama a los comandos por su número
    for system in systems.values_mut() {
        for (i, command) in system.commands.iter_mut().enumerate() {
            if command.label.is_empty() {
                command.label = format!("#{}", i + 1);
            }
        }
    }
    Ok(systems)
}

/// Lee un `es_find_rules.xml`: `<emulator name>` y `<core name>` con sus `<rule type>` de `<entry>`
pub fn parse_find_rules(xml: &str) -> Result<FindRules, String> {
    let mut reader = Reader::from_str(xml);
    let mut rules = FindRules::default();
    // A quién pertenecen las reglas que se están leyendo (emulador o core, y su nombre)
    let mut owner: Option<(bool, String)> = None;
    let mut rule: Option<Rule> = None;
    let mut in_entry = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                tag @ (b"emulator" | b"core") => {
                    let name = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"name")
                        .map(|a| a.unescape_value().unwrap_or_default().to_string())
                        .unwrap_or_default();
                    owner = Some((tag == b"core", name));
                }
                b"rule" => {
                    let kind = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"type")
                        .and_then(|a| rule_kind(&a.unescape_value().unwrap_or_default()));
                    rule = kind.map(|kind| Rule { kind, entries: Vec::new() });
                }
                b"entry" => in_entry = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_entry => {
                if let Some(rule) = rule.as_mut() {
                    let text = t.unescape().unwrap_or_default().trim().to_string();
                    if !text.is_empty() {
                        rule.entries.push(text);
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"entry" => in_entry = false,
                b"rule" => {
                    if let (Some((is_core, name)), Some(done)) = (owner.as_ref(), rule.take()) {
                        let map = if *is_core { &mut rules.cores } else { &mut rules.emulators };
                        map.entry(name.clone()).or_default().push(done);
                    }
                }
                b"emulator" | b"core" => owner = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Failed to parse es_find_rules.xml: {}", e)),
            _ => {}
        }
    }
    Ok(rules)
}

fn rule_kind(kind: &str) -> Option<RuleKind> {
    match kind {
        "staticpath" => Some(RuleKind::StaticPath),
        "systempath" => Some(RuleKind::SystemPath),
        "corepath" => Some(RuleKind::CorePath),
        "winregistrypath" => Some(RuleKind::WinRegistryPath),
        "winregistryvalue" => Some(RuleKind::WinRegistryValue),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_la_copia_de_es_de() {
        let config = LaunchConfig::load(None).unwrap();
        // Unos 195 sistemas; SNES con varios emuladores, el primero RetroArch
        assert!(config.systems.len() > 150);
        let snes = config.system("SNES").unwrap();
        assert!(snes.commands.len() > 3);
        assert!(snes.commands[0].command.contains("%EMULATOR_RETROARCH%"));
        assert!(config.rules.emulators.contains_key("RETROARCH"));
        assert!(config.rules.cores.contains_key("RETROARCH"));
    }

    #[test]
    fn lo_personalizado_sustituye_a_la_copia() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("es_systems.xml"),
            r#"<systemList><system><name>snes</name><fullname>Mi SNES</fullname>
               <command label="Mío">/usr/bin/mio %ROM%</command></system></systemList>"#,
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("es_find_rules.xml"),
            r#"<ruleList><emulator name="RETROARCH"><rule type="staticpath"><entry>/opt/ra</entry></rule></emulator></ruleList>"#,
        )
        .unwrap();
        let config = LaunchConfig::load(Some(tmp.path())).unwrap();
        let snes = config.system("snes").unwrap();
        assert_eq!(snes.full_name, "Mi SNES");
        assert_eq!(snes.commands, vec![LaunchCommand { label: "Mío".into(), command: "/usr/bin/mio %ROM%".into() }]);
        assert_eq!(config.rules.emulators["RETROARCH"][0].entries, vec!["/opt/ra".to_string()]);
        // Lo que no se toca sigue como en la copia
        assert!(config.system("mastersystem").is_some());
    }

    #[test]
    fn comandos_sin_etiqueta_y_entidades() {
        let systems = parse_systems(
            r#"<systemList><system><name>x</name><command>a &amp; b</command><command label="L">c</command></system></systemList>"#,
        )
        .unwrap();
        let x = &systems["x"];
        assert_eq!(x.commands[0].label, "#1");
        assert_eq!(x.commands[0].command, "a & b");
        assert_eq!(x.commands[1].label, "L");
    }
}
