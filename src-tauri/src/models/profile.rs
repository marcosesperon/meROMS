use serde::{Deserialize, Serialize};

/// Tipo de estructura de ficheros que usa un perfil.
/// Si algún día se añade Batocera/Recalbox, va aquí como una variante más.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StructureKind {
    /// EmulationStation clásico: gamelist.xml y medios dentro de cada carpeta de sistema,
    /// con las rutas de los medios escritas en el XML.
    #[default]
    Classic,
    /// ES-DE: gamelists, medios y colecciones en carpetas propias fuera de las ROMs;
    /// los medios se encuentran por el nombre de la ROM.
    Esde,
}

/// Un perfil = una biblioteca de ROMs con su forma de organizarse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: StructureKind,
    /// Carpeta raíz donde están las carpetas de sistema con las ROMs.
    pub rom_root_path: String,
    /// Solo ES-DE: carpeta de datos de ES-DE (la que tiene settings/, gamelists/, collections/…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esde_home_path: Option<String>,
    /// Solo ES-DE: carpeta de medios si no es la de por defecto (<ES-DE>/downloaded_media).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_dir: Option<String>,
    /// «No volver a preguntar»: no avisar al entrar si hay sistemas con la otra estructura.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skip_structure_prompt: bool,
    /// Last system opened with this profile (its folder name), reopened on start
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_system: Option<String>,
}

impl Profile {
    /// Perfil clásico que se crea al migrar los ajustes antiguos (solo tenían `rom_root_path`).
    pub fn default_classic(rom_root_path: &str, language: Option<&str>) -> Self {
        // El nombre lo ve el usuario, así que lo ponemos en su idioma.
        let name = match language {
            Some("en") => "Classic",
            _ => "Clásico",
        };
        Profile {
            id: "default".to_string(),
            name: name.to_string(),
            kind: StructureKind::Classic,
            rom_root_path: rom_root_path.to_string(),
            esde_home_path: None,
            media_dir: None,
            skip_structure_prompt: false,
            last_system: None,
        }
    }
}
