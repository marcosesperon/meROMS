use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;

/// Lo que sacamos de la carpeta de ES-DE para rellenar un perfil.
/// Las rutas ya vienen resueltas: si ES-DE las tiene vacías, ponemos las que usaría él por defecto.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EsdeDetectedPaths {
    pub rom_root_path: String,
    pub media_dir: String,
    /// true si existía settings/es_settings.xml (si no, todo son valores por defecto)
    pub settings_found: bool,
    /// ES-DE tiene activado leer las gamelists de las carpetas de ROMs (modo «legacy»).
    /// Lo avisamos porque entonces ES-DE no miraría <ES-DE>/gamelists.
    pub legacy_gamelist_location: bool,
}

/// Lee <esde_home>/settings/es_settings.xml y devuelve las rutas de ROMs y medios.
/// Si el fichero no existe no es un error: ES-DE recién instalado tampoco lo tiene.
pub fn detect_esde_paths(esde_home: &Path) -> Result<EsdeDetectedPaths, String> {
    let settings_file = esde_home.join("settings").join("es_settings.xml");

    let (raw_rom_dir, raw_media_dir, legacy) = if settings_file.exists() {
        let xml = fs::read_to_string(&settings_file)
            .map_err(|e| format!("Failed to read es_settings.xml: {}", e))?;
        (
            read_setting(&xml, "string", "ROMDirectory")?,
            read_setting(&xml, "string", "MediaDirectory")?,
            read_setting(&xml, "bool", "LegacyGamelistFileLocation")?.as_deref() == Some("true"),
        )
    } else {
        (None, None, false)
    };

    // Valor vacío en ES-DE = «usa el de por defecto», igual que si no estuviera
    let rom_root = non_empty(raw_rom_dir)
        .map(|p| expand_home(&p))
        .unwrap_or_else(|| home_dir().join("ROMs"));
    let media_dir = non_empty(raw_media_dir)
        .map(|p| expand_home(&p))
        .unwrap_or_else(|| esde_home.join("downloaded_media"));

    Ok(EsdeDetectedPaths {
        rom_root_path: rom_root.to_string_lossy().to_string(),
        media_dir: media_dir.to_string_lossy().to_string(),
        settings_found: settings_file.exists(),
        legacy_gamelist_location: legacy,
    })
}

/// Busca `<tipo name="nombre" value="..."/>` en el XML de ajustes de ES-DE.
/// El fichero no tiene elemento raíz (son entradas sueltas), por eso lo recorremos evento a evento.
fn read_setting(xml: &str, element: &str, name: &str) -> Result<Option<String>, String> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) if e.name().as_ref() == element.as_bytes() => {
                let mut found_name = false;
                let mut value = None;
                for attr in e.attributes().flatten() {
                    let attr_value = attr.unescape_value().unwrap_or_default().to_string();
                    match attr.key.as_ref() {
                        b"name" => found_name = attr_value == name,
                        b"value" => value = Some(attr_value),
                        _ => {}
                    }
                }
                if found_name {
                    return Ok(value);
                }
            }
            Ok(Event::Eof) => return Ok(None),
            Err(e) => return Err(format!("Failed to parse es_settings.xml: {}", e)),
            _ => {}
        }
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

/// Carpeta personal del usuario (HOME en macOS/Linux, USERPROFILE en Windows).
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// ES-DE admite rutas que empiezan por «~»; las convertimos a absolutas.
/// Ojo: %ESPATH% (carpeta del ejecutable de ES-DE) no se puede resolver desde aquí y se deja tal cual.
fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) => home_dir().join(rest.trim_start_matches(['/', '\\'])),
        None => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<bool name="LegacyGamelistFileLocation" value="true" />
<string name="MediaDirectory" value="/media/ficticio/medios" />
<string name="ROMDirectory" value="/media/ficticio/roms" />
<string name="ThemeSet" value="linear-es-de" />
"#;

    fn write_settings(dir: &Path, content: &str) {
        fs::create_dir_all(dir.join("settings")).unwrap();
        fs::write(dir.join("settings").join("es_settings.xml"), content).unwrap();
    }

    #[test]
    fn lee_rutas_configuradas() {
        let tmp = tempfile::tempdir().unwrap();
        write_settings(tmp.path(), SAMPLE);

        let detected = detect_esde_paths(tmp.path()).unwrap();
        assert_eq!(detected.rom_root_path, "/media/ficticio/roms");
        assert_eq!(detected.media_dir, "/media/ficticio/medios");
        assert!(detected.settings_found);
        assert!(detected.legacy_gamelist_location);
    }

    #[test]
    fn valores_vacios_usan_los_de_por_defecto() {
        let tmp = tempfile::tempdir().unwrap();
        write_settings(
            tmp.path(),
            r#"<string name="MediaDirectory" value="" /><string name="ROMDirectory" value="" />"#,
        );

        let detected = detect_esde_paths(tmp.path()).unwrap();
        assert_eq!(detected.rom_root_path, home_dir().join("ROMs").to_string_lossy());
        assert_eq!(
            detected.media_dir,
            tmp.path().join("downloaded_media").to_string_lossy()
        );
        assert!(!detected.legacy_gamelist_location);
    }

    #[test]
    fn sin_fichero_de_ajustes_no_falla() {
        let tmp = tempfile::tempdir().unwrap();
        let detected = detect_esde_paths(tmp.path()).unwrap();
        assert!(!detected.settings_found);
        assert_eq!(
            detected.media_dir,
            tmp.path().join("downloaded_media").to_string_lossy()
        );
    }

    #[test]
    fn expande_la_virgulilla() {
        assert_eq!(expand_home("~/ROMs"), home_dir().join("ROMs"));
        assert_eq!(expand_home("/absoluta"), PathBuf::from("/absoluta"));
    }
}
