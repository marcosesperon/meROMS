//! Capa de «estructura»: el único sitio que sabe dónde vive cada cosa (gamelist, copias,
//! medios, colecciones) según el tipo de perfil. El resto del código pregunta aquí en lugar
//! de montar rutas a mano, así añadir otra estructura es añadir una variante y poco más.

pub mod esde_settings;

use std::path::{Path, PathBuf};

use crate::models::{Profile, StructureKind};

const GAMELIST_FILE: &str = "gamelist.xml";
const BACKUP_DIR: &str = ".backups";

/// Carpetas que ES-DE crea dentro de su carpeta de datos.
const ESDE_OWN_FOLDERS: &[&str] = &[
    "gamelists",
    "downloaded_media",
    "settings",
    "collections",
    "themes",
    "custom_systems",
    "logs",
    "controllers",
    "scripts",
    "screensavers",
];

/// Ruta real en disco para comparar (enlaces simbólicos, «..»); si no existe, tal cual.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Un sistema concreto: su nombre de carpeta (snes, psx…) y dónde están sus ROMs.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemRef {
    pub name: String,
    pub rom_dir: PathBuf,
}

impl SystemRef {
    /// A partir de la carpeta de ROMs del sistema (lo que hoy recibe cada comando).
    /// El nombre del sistema es el de la propia carpeta.
    pub fn from_rom_dir(rom_dir: impl AsRef<Path>) -> Self {
        let rom_dir = rom_dir.as_ref().to_path_buf();
        let name = rom_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        SystemRef { name, rom_dir }
    }
}

/// Rutas propias de ES-DE, ya resueltas.
#[derive(Debug, Clone, PartialEq)]
pub struct EsdePaths {
    /// Carpeta de datos de ES-DE (tiene gamelists/, collections/, settings/…)
    pub home: PathBuf,
    /// Carpeta raíz de medios (por defecto <home>/downloaded_media)
    pub media_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Layout {
    Classic,
    Esde(EsdePaths),
}

impl Layout {
    /// Monta el layout de un perfil. Un perfil ES-DE sin carpeta de ES-DE no sirve, así que es error.
    pub fn for_profile(profile: &Profile) -> Result<Self, String> {
        match profile.kind {
            StructureKind::Classic => Ok(Layout::Classic),
            StructureKind::Esde => {
                let home = profile
                    .esde_home_path
                    .as_deref()
                    .filter(|p| !p.is_empty())
                    .map(PathBuf::from)
                    .ok_or_else(|| format!("Profile '{}' has no ES-DE folder configured", profile.name))?;
                // ES-DE portable: si el perfil guarda la carpeta del ejecutable (C:\ES-DE), los
                // datos están en su subcarpeta ES-DE (perfiles creados antes de detectarlo)
                let home = esde_settings::data_folder(&home);
                let media_root = profile
                    .media_dir
                    .as_deref()
                    .filter(|p| !p.is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join("downloaded_media"));
                Ok(Layout::Esde(EsdePaths { home, media_root }))
            }
        }
    }

    /// Carpeta donde está el gamelist.xml del sistema.
    pub fn gamelist_dir(&self, system: &SystemRef) -> PathBuf {
        match self {
            Layout::Classic => system.rom_dir.clone(),
            Layout::Esde(esde) => esde.home.join("gamelists").join(&system.name),
        }
    }

    pub fn gamelist_path(&self, system: &SystemRef) -> PathBuf {
        self.gamelist_dir(system).join(GAMELIST_FILE)
    }

    /// Las copias de seguridad van siempre junto a la gamelist, en .backups/
    pub fn backups_dir(&self, system: &SystemRef) -> PathBuf {
        self.gamelist_dir(system).join(BACKUP_DIR)
    }

    /// Carpeta de un tipo de medio (covers, screenshots…) para un sistema.
    pub fn media_type_dir(&self, system: &SystemRef, media_folder: &str) -> PathBuf {
        match self {
            Layout::Classic => system.rom_dir.join(media_folder),
            Layout::Esde(esde) => esde.media_root.join(&system.name).join(media_folder),
        }
    }

    /// Ruta del fichero de medio de una ROM: <carpeta del tipo>/<subcarpetas de la ROM>/<nombre ROM>.<ext>
    /// Es igual en las dos estructuras (solo cambia la carpeta base), así migrar es mover carpetas tal cual.
    pub fn media_file_path(&self, system: &SystemRef, media_folder: &str, rom_path: &str, ext: &str) -> PathBuf {
        self.media_file_for_stem(system, media_folder, &rom_media_stem(system, rom_path), ext)
    }

    /// Lo mismo, pero partiendo ya del nombre base (con sus subcarpetas).
    pub fn media_file_for_stem(&self, system: &SystemRef, media_folder: &str, stem: &Path, ext: &str) -> PathBuf {
        let file_name = format!("{}.{}", stem.to_string_lossy(), ext.trim_start_matches('.'));
        self.media_type_dir(system, media_folder).join(file_name)
    }

    /// ¿Se escriben las rutas de los medios dentro de gamelist.xml?
    /// En clásica sí (EmulationStation las necesita); ES-DE las busca por nombre y no las quiere.
    pub fn writes_media_tags(&self) -> bool {
        matches!(self, Layout::Classic)
    }

    /// Carpeta donde se guardan las colecciones personalizadas (custom-<nombre>.cfg).
    pub fn collections_dir(&self, system: &SystemRef) -> PathBuf {
        match self {
            // Se mantiene donde lo dejaba meROMS hasta ahora: en la carpeta del sistema
            Layout::Classic => system.rom_dir.clone(),
            Layout::Esde(esde) => esde.home.join("collections"),
        }
    }

    /// ¿Es esta carpeta una de las propias de ES-DE (gamelists/, downloaded_media/…)?
    /// Pasa cuando la carpeta de ES-DE está dentro de la de ROMs (o es la misma): esas
    /// carpetas quedan junto a snes/, psx/… y no son sistemas.
    pub fn is_own_folder(&self, dir: &Path) -> bool {
        let Layout::Esde(esde) = self else { return false };
        let dir = canonical(dir);
        let mut own: Vec<PathBuf> = ESDE_OWN_FOLDERS.iter().map(|f| esde.home.join(f)).collect();
        own.push(esde.media_root.clone());
        own.push(esde.home.clone());
        own.iter().any(|o| canonical(o) == dir)
    }

    /// Cómo se escribe una ROM dentro de un fichero de colección.
    pub fn collection_entry(&self, system: &SystemRef, rom_path: &str) -> String {
        match self {
            Layout::Classic => rom_path.to_string(),
            // ES-DE quiere rutas portables: %ROMPATH%/<sistema>/<ruta dentro del sistema>
            Layout::Esde(_) => format!("%ROMPATH%/{}/{}", system.name, strip_dot_slash(rom_path)),
        }
    }
}

/// Quita el «./» del principio que usan las rutas de gamelist.xml.
pub fn strip_dot_slash(rom_path: &str) -> &str {
    rom_path.strip_prefix("./").unwrap_or(rom_path)
}

/// Nombre base que tienen los medios de una ROM, conservando sus subcarpetas.
/// - Fichero normal: sin la extensión («./Sub/Juego.zip» → «Sub/Juego»).
/// - Carpeta que ES-DE trata como fichero (p. ej. «dig.scummvm/»): el nombre completo,
///   porque ES-DE incluye esa «extensión» en el nombre del medio («dig.scummvm.png»).
pub fn rom_media_stem(system: &SystemRef, rom_path: &str) -> PathBuf {
    let is_dir = system.rom_dir.join(strip_dot_slash(rom_path)).is_dir();
    media_stem_for(rom_path, is_dir)
}

/// Igual que `rom_media_stem` pero diciendo nosotros si la ROM es una carpeta.
/// Hace falta al renombrar: la ruta vieja ya no existe (o la nueva todavía no) y no se puede mirar en disco.
pub fn media_stem_for(rom_path: &str, is_dir: bool) -> PathBuf {
    let relative = Path::new(strip_dot_slash(rom_path));
    if is_dir {
        return relative.to_path_buf();
    }
    match relative.file_stem() {
        Some(stem) => relative.with_file_name(stem),
        None => relative.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn esde_profile(home: &str, media: Option<&str>) -> Profile {
        Profile {
            id: "p1".into(),
            name: "ES-DE prueba".into(),
            kind: StructureKind::Esde,
            rom_root_path: "/ficticio/ROMs".into(),
            esde_home_path: Some(home.into()),
            media_dir: media.map(String::from),
            skip_structure_prompt: false,
            last_system: None,
            bios_dir: None,
            saves_dir: None,
        }
    }

    #[test]
    fn perfil_portable_guardado_con_la_carpeta_del_ejecutable() {
        // Perfil creado antes de detectar la versión portable: guarda C:\\ES-DE en vez de C:\\ES-DE\\ES-DE
        let tmp = tempfile::tempdir().unwrap();
        let exe_dir = tmp.path().join("ES-DE");
        fs::create_dir_all(exe_dir.join("ES-DE/gamelists/cps2")).unwrap();
        let layout = Layout::for_profile(&esde_profile(&exe_dir.to_string_lossy(), None)).unwrap();
        let system = SystemRef::from_rom_dir(exe_dir.join("ROMs/cps2"));
        assert_eq!(layout.gamelist_path(&system), exe_dir.join("ES-DE/gamelists/cps2/gamelist.xml"));
    }

    #[test]
    fn carpetas_propias_de_esde_no_son_sistemas() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        // La carpeta de ES-DE es la misma que la de ROMs
        let layout = Layout::Esde(EsdePaths { home: root.clone(), media_root: root.join("downloaded_media") });
        fs::create_dir_all(root.join("gamelists")).unwrap();
        fs::create_dir_all(root.join("snes")).unwrap();
        assert!(layout.is_own_folder(&root.join("gamelists")));
        assert!(layout.is_own_folder(&root.join("downloaded_media")));
        assert!(!layout.is_own_folder(&root.join("snes")));
        assert!(!Layout::Classic.is_own_folder(&root.join("gamelists")));
    }

    fn snes() -> SystemRef {
        SystemRef::from_rom_dir("/ficticio/ROMs/snes")
    }

    #[test]
    fn system_ref_toma_el_nombre_de_la_carpeta() {
        let sys = snes();
        assert_eq!(sys.name, "snes");
        assert_eq!(sys.rom_dir, PathBuf::from("/ficticio/ROMs/snes"));
    }

    #[test]
    fn clasica_lo_deja_todo_en_la_carpeta_del_sistema() {
        let layout = Layout::Classic;
        let sys = snes();
        assert_eq!(layout.gamelist_path(&sys), PathBuf::from("/ficticio/ROMs/snes/gamelist.xml"));
        assert_eq!(layout.backups_dir(&sys), PathBuf::from("/ficticio/ROMs/snes/.backups"));
        assert_eq!(
            layout.media_file_path(&sys, "covers", "./Super Juego.sfc", "png"),
            PathBuf::from("/ficticio/ROMs/snes/covers/Super Juego.png")
        );
        assert!(layout.writes_media_tags());
        assert_eq!(layout.collection_entry(&sys, "./Super Juego.sfc"), "./Super Juego.sfc");
    }

    #[test]
    fn esde_separa_gamelists_medios_y_colecciones() {
        let layout = Layout::for_profile(&esde_profile("/ficticio/ES-DE", None)).unwrap();
        let sys = snes();
        assert_eq!(
            layout.gamelist_path(&sys),
            PathBuf::from("/ficticio/ES-DE/gamelists/snes/gamelist.xml")
        );
        assert_eq!(
            layout.backups_dir(&sys),
            PathBuf::from("/ficticio/ES-DE/gamelists/snes/.backups")
        );
        assert_eq!(
            layout.media_file_path(&sys, "covers", "./Super Juego.sfc", ".png"),
            PathBuf::from("/ficticio/ES-DE/downloaded_media/snes/covers/Super Juego.png")
        );
        assert!(!layout.writes_media_tags());
        assert_eq!(layout.collections_dir(&sys), PathBuf::from("/ficticio/ES-DE/collections"));
        assert_eq!(
            layout.collection_entry(&sys, "./Super Juego.sfc"),
            "%ROMPATH%/snes/Super Juego.sfc"
        );
    }

    #[test]
    fn esde_respeta_carpeta_de_medios_personalizada() {
        let layout = Layout::for_profile(&esde_profile("/ficticio/ES-DE", Some("/otro/medios"))).unwrap();
        assert_eq!(
            layout.media_type_dir(&snes(), "screenshots"),
            PathBuf::from("/otro/medios/snes/screenshots")
        );
    }

    #[test]
    fn esde_sin_carpeta_es_error() {
        let mut profile = esde_profile("", None);
        assert!(Layout::for_profile(&profile).is_err());
        profile.esde_home_path = None;
        assert!(Layout::for_profile(&profile).is_err());
    }

    #[test]
    fn los_medios_conservan_las_subcarpetas_de_la_rom() {
        let layout = Layout::Classic;
        assert_eq!(
            layout.media_file_path(&snes(), "screenshots", "./Multidisk/Juego/Juego.m3u", "png"),
            PathBuf::from("/ficticio/ROMs/snes/screenshots/Multidisk/Juego/Juego.png")
        );
    }

    #[test]
    fn carpeta_como_fichero_mantiene_su_extension_en_el_medio() {
        let tmp = tempfile::tempdir().unwrap();
        let rom_dir = tmp.path().join("scummvm");
        fs::create_dir_all(rom_dir.join("dig.scummvm")).unwrap();
        let sys = SystemRef::from_rom_dir(&rom_dir);

        assert_eq!(rom_media_stem(&sys, "./dig.scummvm"), PathBuf::from("dig.scummvm"));
        assert_eq!(
            Layout::Classic.media_file_path(&sys, "screenshots", "./dig.scummvm", "png"),
            rom_dir.join("screenshots").join("dig.scummvm.png")
        );
    }
}
