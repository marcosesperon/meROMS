//! Todo lo relativo a los ficheros de medios (portadas, capturas, vídeos…):
//! qué tipos hay, en qué carpeta va cada uno, a qué campo de la gamelist corresponde
//! y cómo encontrarlos por el nombre de la ROM.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::layout::{media_stem_for, rom_media_stem, Layout, SystemRef};
use crate::models::{AppSettings, Game, GameList};

pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"];
pub const VIDEO_EXTS: &[&str] = &["mp4", "mkv", "avi", "mov", "webm", "wmv"];
pub const MANUAL_EXTS: &[&str] = &["pdf"];

/// Un tipo de medio: su carpeta (nombres de ES-DE) y el campo de la gamelist que le toca.
#[derive(Debug, PartialEq)]
pub struct MediaKind {
    pub folder: &'static str,
    pub field: &'static str,
    /// false = ES-DE no tiene esta carpeta (solo existe en la estructura clásica)
    pub in_esde: bool,
}

impl MediaKind {
    pub fn extensions(&self) -> &'static [&'static str] {
        match self.field {
            "video" => VIDEO_EXTS,
            "manual" => MANUAL_EXTS,
            _ => IMAGE_EXTS,
        }
    }
}

/// Tabla única de tipos de medio. El orden importa: es el orden de búsqueda y de listado.
pub const MEDIA_KINDS: &[MediaKind] = &[
    MediaKind { folder: "covers", field: "image", in_esde: true },
    MediaKind { folder: "screenshots", field: "thumbnail", in_esde: true },
    MediaKind { folder: "marquees", field: "marquee", in_esde: true },
    MediaKind { folder: "titlescreens", field: "titleshot", in_esde: true },
    MediaKind { folder: "backcovers", field: "boxback", in_esde: true },
    MediaKind { folder: "3dboxes", field: "boxart", in_esde: true },
    MediaKind { folder: "physicalmedia", field: "cartridge", in_esde: true },
    MediaKind { folder: "fanart", field: "fanart", in_esde: true },
    MediaKind { folder: "miximages", field: "mix", in_esde: true },
    MediaKind { folder: "manuals", field: "manual", in_esde: true },
    MediaKind { folder: "videos", field: "video", in_esde: true },
    // ES-DE usa «marquees» para los logos; la clásica puede tener además un campo wheel aparte
    MediaKind { folder: "wheels", field: "wheel", in_esde: false },
];

/// Todos los campos de medios de un juego, en el mismo orden que la tabla.
pub fn media_fields() -> impl Iterator<Item = &'static str> {
    MEDIA_KINDS.iter().map(|k| k.field)
}

pub fn kind_for_field(field: &str) -> Option<&'static MediaKind> {
    MEDIA_KINDS.iter().find(|k| k.field == field)
}

pub fn kind_for_folder(folder: &str) -> Option<&'static MediaKind> {
    MEDIA_KINDS.iter().find(|k| k.folder == folder)
}

/// Tipo de medio que corresponde a cada tipo de descarga del scraper.
pub fn kind_for_scraper_type(media_type: &str) -> Option<&'static MediaKind> {
    let folder = match media_type {
        "box-2D" => "covers",
        "box-2D-back" => "backcovers",
        "box-3D" => "3dboxes",
        "support-2D" => "physicalmedia",
        "ss" => "screenshots",
        "sstitle" => "titlescreens",
        "marquee" | "wheel" | "wheel-hd" | "screenmarquee" => "marquees",
        "fanart" => "fanart",
        "mixrbv1" | "mixrbv2" => "miximages",
        "manuel" => "manuals",
        "video" | "video-normalized" => "videos",
        _ => return None,
    };
    kind_for_folder(folder)
}

/// Qué tipo de medio va a parar a cada campo según la estructura:
/// - ES-DE: siempre el campo fijo del tipo (así se ve igual que al cargar).
/// - Clásica: el que haya configurado el usuario o, si no, el de siempre de meROMS.
pub fn field_for_scraper_type(layout: &Layout, settings: &AppSettings, media_type: &str) -> Option<String> {
    let kind = kind_for_scraper_type(media_type)?;
    if !layout.writes_media_tags() {
        return Some(kind.field.to_string());
    }
    Some(settings.media_field_for(media_type).unwrap_or_else(|| kind.field.to_string()))
}

/// Dónde se guarda un medio que baja el scraper y qué campo del juego rellena.
/// Devuelve (ruta absoluta de destino, campo, valor a guardar en el campo).
pub struct ScrapedMediaTarget {
    pub dest: PathBuf,
    pub field: String,
    pub value: String,
}

pub fn scraped_media_target(
    layout: &Layout,
    system: &SystemRef,
    settings: &AppSettings,
    media_type: &str,
    rom_path: &str,
    ext: &str,
) -> Option<ScrapedMediaTarget> {
    let kind = kind_for_scraper_type(media_type)?;
    let field = field_for_scraper_type(layout, settings, media_type)?;
    let dest = layout.media_file_path(system, kind.folder, rom_path, ext);
    let value = media_ref(layout, system, &dest);
    Some(ScrapedMediaTarget { dest, field, value })
}

/// Whether a game already has a media file of a scraper type: the file its field points to,
/// or the unified `<folder>/<ROM name>` with any extension of that kind.
pub fn has_scraped_media(layout: &Layout, system: &SystemRef, settings: &AppSettings, game: &Game, media_type: &str) -> bool {
    let by_field = field_for_scraper_type(layout, settings, media_type)
        .and_then(|field| get_field(game, &field).map(|v| resolve_media_path(system, v)))
        .is_some_and(|p| p.is_file());
    by_field
        || kind_for_scraper_type(media_type).is_some_and(|kind| {
            kind.extensions().iter().any(|ext| layout.media_file_path(system, kind.folder, &game.path, ext).is_file())
        })
}

/// Other files of the same media type and ROM with a different extension than `keep`
/// (e.g. `covers/Game.jpg` when `covers/Game.png` was just downloaded).
pub fn other_extension_files(layout: &Layout, system: &SystemRef, media_type: &str, rom_path: &str, keep: &Path) -> Vec<PathBuf> {
    let Some(kind) = kind_for_scraper_type(media_type) else { return Vec::new() };
    kind.extensions()
        .iter()
        .map(|ext| layout.media_file_path(system, kind.folder, rom_path, ext))
        .filter(|p| p.as_path() != keep && p.is_file())
        .collect()
}

/// After overwriting a media file: the same media with another extension would stay next to
/// it (ES-DE shows only one of them), so it goes to the trash.
pub fn trash_replaced_extensions(layout: &Layout, system: &SystemRef, media_type: &str, rom_path: &str, keep: &Path) {
    let others = other_extension_files(layout, system, media_type, rom_path, keep);
    let _ = trash_files(&others);
}

pub fn get_field<'a>(game: &'a Game, field: &str) -> Option<&'a String> {
    match field {
        "image" => game.image.as_ref(),
        "thumbnail" => game.thumbnail.as_ref(),
        "marquee" => game.marquee.as_ref(),
        "fanart" => game.fanart.as_ref(),
        "titleshot" => game.titleshot.as_ref(),
        "boxback" => game.boxback.as_ref(),
        "boxart" => game.boxart.as_ref(),
        "cartridge" => game.cartridge.as_ref(),
        "wheel" => game.wheel.as_ref(),
        "mix" => game.mix.as_ref(),
        "manual" => game.manual.as_ref(),
        "video" => game.video.as_ref(),
        _ => None,
    }
}

pub fn set_field(game: &mut Game, field: &str, value: Option<String>) {
    let slot = match field {
        "image" => &mut game.image,
        "thumbnail" => &mut game.thumbnail,
        "marquee" => &mut game.marquee,
        "fanart" => &mut game.fanart,
        "titleshot" => &mut game.titleshot,
        "boxback" => &mut game.boxback,
        "boxart" => &mut game.boxart,
        "cartridge" => &mut game.cartridge,
        "wheel" => &mut game.wheel,
        "mix" => &mut game.mix,
        "manual" => &mut game.manual,
        "video" => &mut game.video,
        _ => return,
    };
    *slot = value;
}

/// Vacía todos los campos de medios (para guardar en ES-DE, que no los quiere en el XML).
pub fn clear_media_fields(game: &mut Game) {
    for field in media_fields() {
        set_field(game, field, None);
    }
}

/// Convierte una ruta de medio de la gamelist en absoluta («./covers/x.png» → <ROMs>/snes/covers/x.png).
pub fn resolve_media_path(system: &SystemRef, media_path: &str) -> PathBuf {
    let p = Path::new(media_path);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    system.rom_dir.join(media_path.strip_prefix("./").unwrap_or(media_path))
}

/// Cómo se guarda en el juego la ruta de un medio recién colocado:
/// en clásica relativa al sistema («./covers/x.png»), en ES-DE absoluta (solo vive en memoria).
pub fn media_ref(layout: &Layout, system: &SystemRef, abs_path: &Path) -> String {
    if layout.writes_media_tags() {
        if let Ok(rel) = abs_path.strip_prefix(&system.rom_dir) {
            return format!("./{}", rel.to_string_lossy().replace('\\', "/"));
        }
    }
    abs_path.to_string_lossy().to_string()
}

/// Índice de los medios de un sistema: para cada tipo, «nombre base» → fichero.
/// Se recorre cada carpeta una sola vez en lugar de preguntar fichero a fichero por cada juego.
pub struct MediaIndex {
    by_folder: HashMap<&'static str, HashMap<PathBuf, PathBuf>>,
}

impl MediaIndex {
    pub fn build(layout: &Layout, system: &SystemRef) -> Self {
        let mut by_folder = HashMap::new();
        for kind in MEDIA_KINDS {
            let dir = layout.media_type_dir(system, kind.folder);
            let mut files: HashMap<PathBuf, PathBuf> = HashMap::new();
            for file in list_files_recursive(&dir) {
                let Some(ext) = extension_lower(&file) else { continue };
                let Some(priority) = kind.extensions().iter().position(|e| *e == ext) else { continue };
                let Ok(rel) = file.strip_prefix(&dir) else { continue };
                let stem = match rel.file_stem() {
                    Some(s) => rel.with_file_name(s),
                    None => continue,
                };
                // Si hay varios ficheros con el mismo nombre (x.png y x.jpg) gana el de la extensión preferida
                let keep_existing = files
                    .get(&stem)
                    .and_then(|old| extension_lower(old))
                    .and_then(|old_ext| kind.extensions().iter().position(|e| *e == old_ext))
                    .is_some_and(|old_priority| old_priority <= priority);
                if !keep_existing {
                    files.insert(stem, file);
                }
            }
            by_folder.insert(kind.folder, files);
        }
        MediaIndex { by_folder }
    }

    /// Fichero de un tipo para una ROM concreta, si existe.
    pub fn find(&self, system: &SystemRef, kind: &MediaKind, rom_path: &str) -> Option<&PathBuf> {
        let stem = rom_media_stem(system, rom_path);
        self.by_folder.get(kind.folder)?.get(&stem)
    }

    /// Todos los ficheros indexados de un tipo, con su nombre base.
    pub fn files_of(&self, kind: &MediaKind) -> impl Iterator<Item = (&PathBuf, &PathBuf)> {
        self.by_folder.get(kind.folder).into_iter().flat_map(|m| m.iter())
    }
}

/// ES-DE: rellena en memoria los campos de medios de cada juego con lo que haya en disco.
/// Así el resto de la app (tabla, cuadrícula, detalle) funciona igual que con la clásica.
pub fn hydrate_gamelist(layout: &Layout, system: &SystemRef, gamelist: &mut GameList) {
    let index = MediaIndex::build(layout, system);
    for game in &mut gamelist.games {
        for kind in MEDIA_KINDS.iter().filter(|k| k.in_esde) {
            let found = index.find(system, kind, &game.path).map(|p| p.to_string_lossy().to_string());
            set_field(game, kind.field, found);
        }
    }
}

/// Renombra los medios (nombrado unificado) de una ROM que cambia de nombre: todos los tipos
/// y extensiones. No pisa nada que ya exista. Devuelve los pares (antes, después) movidos.
pub fn rename_rom_media(
    layout: &Layout,
    system: &SystemRef,
    old_rom_path: &str,
    new_rom_path: &str,
    rom_is_dir: bool,
) -> Vec<(PathBuf, PathBuf)> {
    let old_stem = media_stem_for(old_rom_path, rom_is_dir);
    let new_stem = media_stem_for(new_rom_path, rom_is_dir);
    let mut moved = Vec::new();
    if old_stem == new_stem {
        return moved;
    }
    for kind in MEDIA_KINDS {
        for ext in kind.extensions() {
            let old = layout.media_file_for_stem(system, kind.folder, &old_stem, ext);
            let new = layout.media_file_for_stem(system, kind.folder, &new_stem, ext);
            if !old.is_file() || new.exists() {
                continue;
            }
            if let Some(dir) = new.parent() {
                let _ = fs::create_dir_all(dir);
            }
            if fs::rename(&old, &new).is_ok() {
                moved.push((old, new));
            }
        }
    }
    moved
}

/// Mueve un fichero creando las carpetas que falten. Si es a otro disco (rename falla),
/// copia y borra el original. Nunca pisa un destino que ya exista.
pub fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Err(format!("Destination already exists: {}", to.display()));
    }
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    }
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    fs::copy(from, to).map_err(|e| format!("Failed to copy {}: {}", from.display(), e))?;
    fs::remove_file(from).map_err(|e| format!("Failed to remove {}: {}", from.display(), e))
}

/// Manda varios ficheros a la papelera en una sola operación (en macOS suena una vez, no una
/// por fichero). Si falla en bloque, prueba uno a uno para saber cuáles no se pudieron.
/// Devuelve los que fallaron, con el motivo.
pub fn trash_files(files: &[PathBuf]) -> Vec<(PathBuf, String)> {
    if files.is_empty() || trash::delete_all(files).is_ok() {
        return Vec::new();
    }
    files
        .iter()
        // Puede que el intento en bloque ya mandara alguno antes de fallar
        .filter(|f| f.exists())
        .filter_map(|f| trash::delete(f).err().map(|e| (f.clone(), e.to_string())))
        .collect()
}

/// Copia un fichero creando las carpetas que falten. Nunca pisa un destino que ya exista.
pub fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Err(format!("Destination already exists: {}", to.display()));
    }
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    }
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("Failed to copy {}: {}", from.display(), e))
}

/// Borra las carpetas que se han quedado vacías, subiendo desde `dir` hasta `stop` (sin incluirlo).
pub fn remove_empty_dirs_up_to(dir: &Path, stop: &Path) {
    let mut current = dir.to_path_buf();
    while current.starts_with(stop) && current != stop {
        // remove_dir solo funciona si está vacía: justo lo que queremos
        if fs::remove_dir(&current).is_err() {
            break;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
}

pub fn extension_lower(path: &Path) -> Option<String> {
    path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase())
}

/// Lista todos los ficheros de una carpeta y sus subcarpetas (vacío si no existe).
pub fn list_files_recursive(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(list_files_recursive(&path));
        } else if path.is_file() {
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::EsdePaths;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    fn esde_fixture() -> (tempfile::TempDir, Layout, SystemRef) {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Esde(EsdePaths {
            home: tmp.path().join("ES-DE"),
            media_root: tmp.path().join("ES-DE/downloaded_media"),
        });
        let system = SystemRef::from_rom_dir(tmp.path().join("ROMs/snes"));
        (tmp, layout, system)
    }

    #[test]
    fn encuentra_el_mismo_medio_con_otra_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("covers/Juego.png"));
        touch(&system.rom_dir.join("covers/Juego.jpg"));
        touch(&system.rom_dir.join("covers/Otro.jpg"));
        let keep = system.rom_dir.join("covers/Juego.png");
        let others = other_extension_files(&Layout::Classic, &system, "box-2D", "./Juego.sfc", &keep);
        assert_eq!(others, vec![system.rom_dir.join("covers/Juego.jpg")]);
    }

    #[test]
    fn sabe_si_un_juego_ya_tiene_un_medio_del_scraper() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        let settings = AppSettings::default();
        let mut game = Game { path: "./Juego.sfc".into(), ..Default::default() };
        assert!(!has_scraped_media(&Layout::Classic, &system, &settings, &game, "box-2D"));
        // Nombrado unificado, con cualquier extensión del tipo
        touch(&system.rom_dir.join("covers/Juego.jpg"));
        assert!(has_scraped_media(&Layout::Classic, &system, &settings, &game, "box-2D"));
        // O el fichero al que apunta su campo (nombre antiguo)
        touch(&system.rom_dir.join("images/Juego-thumb.png"));
        game.thumbnail = Some("./images/Juego-thumb.png".into());
        assert!(has_scraped_media(&Layout::Classic, &system, &settings, &game, "ss"));
        assert!(!has_scraped_media(&Layout::Classic, &system, &settings, &game, "video"));
    }

    #[test]
    fn el_scraper_y_los_campos_apuntan_a_carpetas_de_esde() {
        assert_eq!(kind_for_scraper_type("box-2D").unwrap().folder, "covers");
        assert_eq!(kind_for_scraper_type("wheel-hd").unwrap().folder, "marquees");
        assert_eq!(kind_for_scraper_type("video-normalized").unwrap().field, "video");
        assert!(kind_for_scraper_type("desconocido").is_none());
        assert_eq!(kind_for_field("thumbnail").unwrap().folder, "screenshots");
    }

    #[test]
    fn hidrata_por_nombre_de_rom_y_prefiere_png() {
        let (tmp, layout, system) = esde_fixture();
        let media = tmp.path().join("ES-DE/downloaded_media/snes");
        touch(&media.join("covers/Juego Uno.jpg"));
        touch(&media.join("covers/Juego Uno.png"));
        touch(&media.join("videos/Juego Uno.mp4"));
        touch(&media.join("screenshots/Otro.png"));

        let mut gamelist = GameList {
            games: vec![Game { path: "./Juego Uno.sfc".into(), ..Default::default() }],
            ..Default::default()
        };
        hydrate_gamelist(&layout, &system, &mut gamelist);

        let game = &gamelist.games[0];
        assert_eq!(game.image.as_deref(), Some(media.join("covers/Juego Uno.png").to_str().unwrap()));
        assert_eq!(game.video.as_deref(), Some(media.join("videos/Juego Uno.mp4").to_str().unwrap()));
        assert!(game.thumbnail.is_none());
    }

    #[test]
    fn hidratar_limpia_rutas_viejas_que_ya_no_existen() {
        let (_tmp, layout, system) = esde_fixture();
        let mut gamelist = GameList {
            games: vec![Game {
                path: "./Juego.sfc".into(),
                image: Some("./images/Juego-image.png".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        hydrate_gamelist(&layout, &system, &mut gamelist);
        assert!(gamelist.games[0].image.is_none());
    }

    #[test]
    fn media_ref_relativa_en_clasica_y_absoluta_en_esde() {
        let system = SystemRef::from_rom_dir("/ficticio/ROMs/snes");
        let abs = Path::new("/ficticio/ROMs/snes/covers/Juego.png");
        assert_eq!(media_ref(&Layout::Classic, &system, abs), "./covers/Juego.png");

        let (_tmp, esde, esde_sys) = esde_fixture();
        let esde_abs = esde.media_file_path(&esde_sys, "covers", "./Juego.sfc", "png");
        assert_eq!(media_ref(&esde, &esde_sys, &esde_abs), esde_abs.to_string_lossy());
    }

    #[test]
    fn resolve_media_path_entiende_relativas_y_absolutas() {
        let system = SystemRef::from_rom_dir("/ficticio/ROMs/snes");
        assert_eq!(
            resolve_media_path(&system, "./covers/x.png"),
            PathBuf::from("/ficticio/ROMs/snes/covers/x.png")
        );
        assert_eq!(resolve_media_path(&system, "/abs/x.png"), PathBuf::from("/abs/x.png"));
    }

    #[test]
    fn clear_vacia_todos_los_campos_de_medios() {
        let mut game = Game {
            path: "./x.sfc".into(),
            name: Some("X".into()),
            image: Some("a".into()),
            video: Some("b".into()),
            wheel: Some("c".into()),
            ..Default::default()
        };
        clear_media_fields(&mut game);
        assert!(game.image.is_none() && game.video.is_none() && game.wheel.is_none());
        assert_eq!(game.name.as_deref(), Some("X"));
    }
}

#[cfg(test)]
mod rename_tests {
    use super::*;

    #[test]
    fn renombra_todos_los_tipos_sin_pisar_nada() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        let covers = system.rom_dir.join("covers");
        fs::create_dir_all(&covers).unwrap();
        fs::write(covers.join("Viejo.png"), b"a").unwrap();
        fs::create_dir_all(system.rom_dir.join("videos")).unwrap();
        fs::write(system.rom_dir.join("videos/Viejo.mp4"), b"v").unwrap();
        // Ya existe un destino: no se toca
        fs::create_dir_all(system.rom_dir.join("screenshots")).unwrap();
        fs::write(system.rom_dir.join("screenshots/Viejo.png"), b"old").unwrap();
        fs::write(system.rom_dir.join("screenshots/Nuevo.png"), b"keep").unwrap();

        let moved = rename_rom_media(&Layout::Classic, &system, "./Viejo.sfc", "./Nuevo.sfc", false);

        assert_eq!(moved.len(), 2);
        assert!(covers.join("Nuevo.png").exists() && !covers.join("Viejo.png").exists());
        assert!(system.rom_dir.join("videos/Nuevo.mp4").exists());
        assert_eq!(fs::read(system.rom_dir.join("screenshots/Nuevo.png")).unwrap(), b"keep");
        assert!(system.rom_dir.join("screenshots/Viejo.png").exists());
    }
}
