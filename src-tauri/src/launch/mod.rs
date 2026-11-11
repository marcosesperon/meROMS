//! Lanzar juegos como ES-DE: su configuración (copia en meROMS + la personalizada del usuario),
//! la búsqueda de emuladores y cores, y la elección del comando de cada juego.

pub mod command;
pub mod config;
pub mod find;

use std::path::Path;
use std::sync::LazyLock;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use regex::Regex;

use config::LaunchConfig;

/// De dónde sale el comando de un juego
#[derive(Debug, Clone, PartialEq)]
pub enum Chosen {
    /// El comando propio del sistema (vista del sistema en meROMS)
    Custom(String),
    /// Uno de ES-DE: su etiqueta y el comando
    Esde { label: String, command: String },
}

impl Chosen {
    pub fn command(&self) -> &str {
        match self {
            Chosen::Custom(c) => c,
            Chosen::Esde { command, .. } => command,
        }
    }
}

/// El comando de un juego, por orden: el propio del sistema; el emulador del juego
/// (`<altemulator>`); el del sistema en el gamelist (`<alternativeEmulator>`); el primero de
/// ES-DE. Una etiqueta que ya no existe se salta (como ES-DE). `None` = ES-DE no conoce el
/// sistema y no hay comando propio.
pub fn choose_command(
    config: &LaunchConfig,
    system: &str,
    custom: Option<&str>,
    game_alt: Option<&str>,
    system_alt: Option<&str>,
) -> Option<Chosen> {
    if let Some(custom) = custom.map(str::trim).filter(|c| !c.is_empty()) {
        return Some(Chosen::Custom(custom.to_string()));
    }
    let def = config.system(system)?;
    let by_label = |label: Option<&str>| label.and_then(|l| def.commands.iter().find(|c| c.label == l));
    let picked = by_label(game_alt).or_else(|| by_label(system_alt)).or_else(|| def.commands.first())?;
    Some(Chosen::Esde { label: picked.label.clone(), command: picked.command.clone() })
}

static ALT_EMULATOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)[ \t]*<alternativeEmulator>.*?</alternativeEmulator>[ \t]*\r?\n?").unwrap());

/// El emulador elegido para el sistema en el gamelist (`<alternativeEmulator><label>`)
pub fn system_alt_emulator(xml: &str) -> Option<String> {
    let block = ALT_EMULATOR_RE.find(xml)?.as_str();
    read_text(block, "label")
}

/// El emulador elegido para un juego (`<altemulator>` dentro de su `<game>`)
pub fn game_alt_emulator(xml: &str, game_path: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    // Dentro de un <game>: su ruta y su altemulator, según van saliendo
    let mut in_game = false;
    let mut field: Option<&'static str> = None;
    let (mut path, mut alt) = (None::<String>, None::<String>);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"game" => {
                    in_game = true;
                    path = None;
                    alt = None;
                }
                b"path" if in_game => field = Some("path"),
                b"altemulator" if in_game => field = Some("alt"),
                _ => field = None,
            },
            Ok(Event::Text(t)) => {
                let text = t.unescape().unwrap_or_default().trim().to_string();
                match field {
                    Some("path") => path = Some(text),
                    Some("alt") => alt = Some(text),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                field = None;
                if e.name().as_ref() == b"game" {
                    in_game = false;
                    if path.as_deref() == Some(game_path) {
                        return alt.filter(|a| !a.is_empty());
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

/// El gamelist con otro emulador para el sistema (`None` = el predeterminado: se quita el
/// bloque). Va antes de `<gameList>`, como lo escribe ES-DE.
pub fn with_system_alt_emulator(xml: Option<&str>, label: Option<&str>) -> String {
    let xml = xml.unwrap_or("<?xml version=\"1.0\"?>\n<gameList>\n</gameList>\n");
    let mut out = ALT_EMULATOR_RE.replace_all(xml, "").to_string();
    if let Some(label) = label.filter(|l| !l.is_empty()) {
        let block = format!("<alternativeEmulator>\n\t<label>{}</label>\n</alternativeEmulator>\n", escape_xml(label));
        // Justo antes de <gameList> (o al principio si no lo hubiera)
        let at = out.find("<gameList").unwrap_or(0);
        out.insert_str(at, &block);
    }
    out
}

/// Texto del primer `<tag>` de un trozo de XML
fn read_text(xml: &str, tag: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    let mut inside = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => inside = e.name().as_ref() == tag.as_bytes(),
            Ok(Event::Text(t)) if inside => {
                let text = t.unescape().unwrap_or_default().trim().to_string();
                return (!text.is_empty()).then_some(text);
            }
            Ok(Event::End(_)) => inside = false,
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// %ESPATH% de una carpeta de datos de ES-DE: la de encima si es una instalación portable
/// (tiene ES-DE.exe o portable.txt)
pub fn es_path_for(esde_home: &Path) -> Option<std::path::PathBuf> {
    let parent = esde_home.parent()?;
    ["ES-DE.exe", "portable.txt"].iter().any(|m| parent.join(m).is_file()).then(|| parent.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::{LaunchCommand, SystemDef};

    fn config() -> LaunchConfig {
        let mut config = LaunchConfig::default();
        config.systems.insert(
            "snes".into(),
            SystemDef {
                name: "snes".into(),
                full_name: "SNES".into(),
                commands: vec![
                    LaunchCommand { label: "A".into(), command: "a %ROM%".into() },
                    LaunchCommand { label: "B".into(), command: "b %ROM%".into() },
                    LaunchCommand { label: "C".into(), command: "c %ROM%".into() },
                ],
            },
        );
        config
    }

    #[test]
    fn orden_de_eleccion() {
        let c = config();
        let label = |ch: Option<Chosen>| match ch {
            Some(Chosen::Esde { label, .. }) => label,
            Some(Chosen::Custom(c)) => format!("custom:{c}"),
            None => "none".into(),
        };
        assert_eq!(label(choose_command(&c, "snes", None, None, None)), "A");
        assert_eq!(label(choose_command(&c, "snes", None, None, Some("B"))), "B");
        assert_eq!(label(choose_command(&c, "snes", None, Some("C"), Some("B"))), "C");
        // Una etiqueta que ya no existe se salta
        assert_eq!(label(choose_command(&c, "snes", None, Some("Z"), Some("B"))), "B");
        assert_eq!(label(choose_command(&c, "snes", Some(" mio %ROM% "), Some("C"), None)), "custom:mio %ROM%");
        assert_eq!(label(choose_command(&c, "otro", None, None, None)), "none");
        assert_eq!(label(choose_command(&c, "otro", Some("x"), None, None)), "custom:x");
    }

    #[test]
    fn emulador_del_sistema_y_del_juego_en_el_gamelist() {
        let xml = "<?xml version=\"1.0\"?>\n<alternativeEmulator>\n\t<label>Snes9x 2010</label>\n</alternativeEmulator>\n<gameList>\n\t<game>\n\t\t<path>./a.sfc</path>\n\t\t<altemulator>bsnes</altemulator>\n\t</game>\n\t<game>\n\t\t<path>./b.sfc</path>\n\t</game>\n</gameList>\n";
        assert_eq!(system_alt_emulator(xml).as_deref(), Some("Snes9x 2010"));
        assert_eq!(game_alt_emulator(xml, "./a.sfc").as_deref(), Some("bsnes"));
        assert_eq!(game_alt_emulator(xml, "./b.sfc"), None);
        assert_eq!(system_alt_emulator("<gameList></gameList>"), None);
    }

    #[test]
    fn escribir_el_emulador_del_sistema() {
        let original = "<?xml version=\"1.0\"?>\n<gameList>\n\t<game><path>./a.sfc</path></game>\n</gameList>\n";
        let with = with_system_alt_emulator(Some(original), Some("Snes9x & co"));
        assert_eq!(
            with,
            "<?xml version=\"1.0\"?>\n<alternativeEmulator>\n\t<label>Snes9x &amp; co</label>\n</alternativeEmulator>\n<gameList>\n\t<game><path>./a.sfc</path></game>\n</gameList>\n"
        );
        assert_eq!(system_alt_emulator(&with).as_deref(), Some("Snes9x & co"));
        // Cambiarlo no duplica el bloque; quitarlo deja el gamelist como estaba
        let other = with_system_alt_emulator(Some(&with), Some("bsnes"));
        assert_eq!(other.matches("<alternativeEmulator>").count(), 1);
        assert_eq!(with_system_alt_emulator(Some(&other), None), original);
        // Sin gamelist, uno nuevo con el bloque
        let fresh = with_system_alt_emulator(None, Some("bsnes"));
        assert!(fresh.contains("<label>bsnes</label>") && fresh.contains("<gameList>"));
    }

    #[test]
    fn espath_solo_en_la_portable() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("ES-DE");
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(es_path_for(&home), None);
        std::fs::write(tmp.path().join("portable.txt"), "").unwrap();
        assert_eq!(es_path_for(&home), Some(tmp.path().to_path_buf()));
    }
}
