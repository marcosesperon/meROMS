use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;

/// Lo que sacamos de la carpeta de ES-DE para rellenar un perfil.
/// Las rutas ya vienen resueltas: si ES-DE las tiene vacías, ponemos las que usaría él por defecto.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EsdeDetectedPaths {
    /// Carpeta de datos de ES-DE que hay que usar (puede no ser la elegida: en la versión
    /// portable se elige la del ejecutable y los datos están en su subcarpeta «ES-DE»)
    pub esde_home: String,
    /// Instalación portable (ES-DE.exe o portable.txt junto a la carpeta de datos)
    pub portable: bool,
    pub rom_root_path: String,
    pub media_dir: String,
    /// true si existía settings/es_settings.xml (si no, todo son valores por defecto)
    pub settings_found: bool,
    /// ES-DE tiene activado leer las gamelists de las carpetas de ROMs (modo «legacy»).
    /// Lo avisamos porque entonces ES-DE no miraría <ES-DE>/gamelists.
    pub legacy_gamelist_location: bool,
}

/// Ficheros que delatan la carpeta del ejecutable de una instalación portable de ES-DE
const PORTABLE_MARKERS: &[&str] = &["ES-DE.exe", "portable.txt"];

/// Lee <esde_home>/settings/es_settings.xml y devuelve las rutas de ROMs y medios.
/// Si el fichero no existe no es un error: ES-DE recién instalado tampoco lo tiene.
/// `picked` puede ser la carpeta de datos o, en la versión portable, la del ejecutable.
pub fn detect_esde_paths(picked: &Path) -> Result<EsdeDetectedPaths, String> {
    let esde_home = data_folder(picked);
    let esde_home = esde_home.as_path();
    // Portable: la carpeta del ejecutable (lo que ES-DE llama %ESPATH%) es la de encima
    let es_path = esde_home
        .parent()
        .filter(|parent| PORTABLE_MARKERS.iter().any(|m| parent.join(m).is_file()))
        .map(Path::to_path_buf);
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

    // Valor vacío en ES-DE = «usa el de por defecto», igual que si no estuviera.
    // Por defecto las ROMs van en ~/ROMs, o junto al ejecutable en la versión portable.
    let expand = |p: &str| expand_home(&expand_es_path(p, es_path.as_deref()));
    let rom_root = non_empty(raw_rom_dir)
        .map(|p| expand(&p))
        .unwrap_or_else(|| es_path.as_ref().map_or_else(|| home_dir().join("ROMs"), |e| e.join("ROMs")));
    let media_dir = non_empty(raw_media_dir)
        .map(|p| expand(&p))
        .unwrap_or_else(|| esde_home.join("downloaded_media"));

    Ok(EsdeDetectedPaths {
        esde_home: esde_home.to_string_lossy().to_string(),
        portable: es_path.is_some(),
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

/// La carpeta de datos de ES-DE a partir de la elegida: ella misma si tiene settings/ o
/// gamelists/; si no, su subcarpeta «ES-DE» si la tiene (versión portable); si no, la elegida.
pub fn data_folder(picked: &Path) -> PathBuf {
    let looks_like_data = |dir: &Path| dir.join("settings").is_dir() || dir.join("gamelists").is_dir();
    if looks_like_data(picked) {
        return picked.to_path_buf();
    }
    let inner = picked.join("ES-DE");
    if looks_like_data(&inner) {
        return inner;
    }
    picked.to_path_buf()
}

/// %ESPATH% (carpeta del ejecutable de ES-DE) por su valor, si lo conocemos (portable)
fn expand_es_path(path: &str, es_path: Option<&Path>) -> String {
    match es_path {
        Some(dir) if path.contains("%ESPATH%") => path.replace("%ESPATH%", &dir.to_string_lossy()),
        _ => path.to_string(),
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

/// Carpeta personal del usuario (HOME en macOS/Linux, USERPROFILE en Windows).
pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// ES-DE admite rutas que empiezan por «~»; las convertimos a absolutas.
/// (%ESPATH% se resuelve antes, en `expand_es_path`, cuando la instalación es portable.)
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

    /// Instalación portable como la de Windows: C:\\ES-DE\\{ES-DE.exe, ROMs\\, ES-DE\\settings…}
    fn portable(root: &Path, settings: Option<&str>) -> PathBuf {
        let exe_dir = root.join("ES-DE");
        fs::create_dir_all(exe_dir.join("ROMs")).unwrap();
        fs::write(exe_dir.join("ES-DE.exe"), b"").unwrap();
        let data = exe_dir.join("ES-DE");
        fs::create_dir_all(data.join("gamelists")).unwrap();
        if let Some(content) = settings {
            write_settings(&data, content);
        }
        exe_dir
    }

    #[test]
    fn portable_eligiendo_la_carpeta_del_ejecutable() {
        let tmp = tempfile::tempdir().unwrap();
        let exe_dir = portable(tmp.path(), None);

        let detected = detect_esde_paths(&exe_dir).unwrap();
        assert!(detected.portable);
        assert_eq!(detected.esde_home, exe_dir.join("ES-DE").to_string_lossy());
        assert_eq!(detected.rom_root_path, exe_dir.join("ROMs").to_string_lossy());
        assert_eq!(detected.media_dir, exe_dir.join("ES-DE").join("downloaded_media").to_string_lossy());
    }

    #[test]
    fn portable_eligiendo_la_carpeta_de_datos_y_espath() {
        let tmp = tempfile::tempdir().unwrap();
        let exe_dir = portable(tmp.path(), Some(r#"<string name="ROMDirectory" value="%ESPATH%/Juegos" />"#));

        let detected = detect_esde_paths(&exe_dir.join("ES-DE")).unwrap();
        assert!(detected.portable);
        assert!(detected.settings_found);
        assert_eq!(detected.esde_home, exe_dir.join("ES-DE").to_string_lossy());
        assert_eq!(PathBuf::from(&detected.rom_root_path), PathBuf::from(format!("{}/Juegos", exe_dir.to_string_lossy())));
    }

    #[test]
    fn instalacion_normal_no_es_portable() {
        let tmp = tempfile::tempdir().unwrap();
        write_settings(tmp.path(), SAMPLE);
        let detected = detect_esde_paths(tmp.path()).unwrap();
        assert!(!detected.portable);
        assert_eq!(detected.esde_home, tmp.path().to_string_lossy());
    }
}
