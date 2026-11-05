//! Normalizador de medios: deja todos los medios de un sistema con el nombrado unificado
//! `<carpeta>/<nombre ROM>.<ext>`.
//! - Clásica: los coloca dentro de la carpeta del sistema y actualiza las rutas del XML.
//! - ES-DE: «recupera» los que se quedaron en la carpeta de ROMs con la estructura clásica
//!   (images/, -boxart…) y los mueve a la carpeta de medios de ES-DE. La gamelist no se toca,
//!   porque en ES-DE no guarda rutas de medios.
//!
//! Funciona en dos pasos para que el usuario vea antes lo que va a pasar:
//! 1. `plan_media_normalization` → lista de cambios propuestos (no toca nada).
//! 2. `apply_media_normalization` → aplica los que el usuario haya dejado marcados.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::{Layout, SystemRef};
use crate::media::{self, MediaIndex, MediaKind};
use crate::models::{AppSettings, Game};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeStatus {
    /// El fichero se mueve/renombra a su sitio unificado
    Move,
    /// El fichero ya está en su sitio (o lo pone ahí otro cambio): solo se actualiza el campo
    Link,
    /// En el destino ya hay otro fichero distinto: no se toca, que decida el usuario
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizeItem {
    pub game_path: String,
    pub game_name: String,
    pub field: String,
    /// Rutas absolutas de origen y destino
    pub from: String,
    pub to: String,
    /// Las mismas, relativas a la carpeta del sistema (para enseñarlas)
    pub from_relative: String,
    pub to_relative: String,
    pub status: NormalizeStatus,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct NormalizeResult {
    pub moved: usize,
    pub linked: usize,
    pub skipped: Vec<String>,
}

/// Con `game_path` solo se planifica ese juego (desde el detalle del juego).
#[tauri::command]
pub fn plan_media_normalization(
    app: tauri::AppHandle,
    system_path: String,
    game_path: Option<String>,
) -> Result<Vec<NormalizeItem>, String> {
    let settings = super::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;
    let mut games = super::gamelist::read_gamelist(&layout, &system)?.games;
    if let Some(only) = &game_path {
        games.retain(|g| &g.path == only);
    }
    Ok(plan(&layout, &system, &settings, &games))
}

#[tauri::command]
pub fn apply_media_normalization(
    app: tauri::AppHandle,
    system_path: String,
    items: Vec<NormalizeItem>,
) -> Result<NormalizeResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let mut gamelist = super::gamelist::read_gamelist(&layout, &system)?;
    let before = gamelist.clone();
    media::journal_begin();
    let result = apply(&layout, &system, &mut gamelist.games, &items);
    let files = media::journal_take();
    // En ES-DE la gamelist no guarda rutas de medios: reescribirla solo dejaría una copia
    // de seguridad que no sirve para nada
    if layout.writes_media_tags() {
        // write_gamelist() hace la copia de seguridad antes de escribir
        super::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    }
    // Para «Deshacer»
    let sys = system.rom_dir.to_string_lossy().to_string();
    super::undo::record(super::undo::UndoOp {
        kind: "normalize".into(),
        system: sys.clone(),
        files,
        games: super::undo::changes_same_paths(&sys, &before, &gamelist),
        refs: super::undo::RefUndo::None,
    });
    Ok(result)
}

/// Calcula los cambios para todos los juegos. No toca el disco.
pub(super) fn plan(layout: &Layout, system: &SystemRef, settings: &AppSettings, games: &[Game]) -> Vec<NormalizeItem> {
    let index = MediaIndex::build(layout, system);
    let mut items = Vec::new();
    // destino → origen que lo ocupa, para detectar dos ficheros distintos que irían al mismo sitio
    let mut claimed: HashMap<PathBuf, PathBuf> = HashMap::new();
    // origen → destino ya decidido: un mismo fichero no se reparte a dos sitios distintos
    // (p. ej. «images/Juego.png» sin sufijo sirve para cualquier campo; gana el primero)
    let mut sources: HashMap<PathBuf, PathBuf> = HashMap::new();

    for game in games {
        for field in media::media_fields() {
            let Some(field_kind) = media::kind_for_field(field) else { continue };
            let current = media::get_field(game, field).map(|v| media::resolve_media_path(system, v));

            let (from, kind) = match current {
                // El campo apunta a un fichero que existe: ese es el que hay que colocar
                Some(path) if path.is_file() => {
                    let kind = guess_kind(system, &path, &game.path).unwrap_or(field_kind);
                    (path, kind)
                }
                // Campo vacío (o roto): ¿hay ya un fichero unificado, o uno con nombre antiguo?
                _ => match index.find(system, field_kind, &game.path) {
                    Some(found) => (found.clone(), field_kind),
                    None => match legacy_file_for(settings, system, &game.path, field) {
                        // Si por su sufijo es de otro tipo (un «-boxart» buscado para la caja 3D
                        // es una portada), no es para este campo
                        Some(found) => match guess_kind(system, &found, &game.path) {
                            Some(kind) if kind.folder != field_kind.folder => continue,
                            _ => (found, field_kind),
                        },
                        None => continue,
                    },
                },
            };

            let ext = media::extension_lower(&from).unwrap_or_else(|| "png".into());
            let to = layout.media_file_path(system, kind.folder, &game.path, &ext);
            let field_is_right = media::get_field(game, field) == Some(&media::media_ref(layout, system, &to));
            if from == to && field_is_right {
                continue; // ya está bien
            }

            match sources.get(&from) {
                Some(previous) if *previous != to => continue,
                _ => {
                    sources.insert(from.clone(), to.clone());
                }
            }
            let status = if from == to {
                NormalizeStatus::Link
            } else if let Some(owner) = claimed.get(&to) {
                if *owner == from { NormalizeStatus::Link } else { NormalizeStatus::Conflict }
            } else if to.exists() {
                NormalizeStatus::Conflict
            } else {
                claimed.insert(to.clone(), from.clone());
                NormalizeStatus::Move
            };

            items.push(NormalizeItem {
                game_path: game.path.clone(),
                game_name: game.name.clone().unwrap_or_else(|| game.path.clone()),
                field: field.to_string(),
                from_relative: relative_to(layout, system, &from),
                to_relative: relative_to(layout, system, &to),
                from: from.to_string_lossy().to_string(),
                to: to.to_string_lossy().to_string(),
                status,
            });
        }
    }
    items
}

/// Aplica los cambios elegidos y actualiza los campos de los juegos en memoria.
fn apply(layout: &Layout, system: &SystemRef, games: &mut [Game], items: &[NormalizeItem]) -> NormalizeResult {
    let mut result = NormalizeResult::default();
    let mut moved: HashSet<(PathBuf, PathBuf)> = HashSet::new();
    let mut emptied_dirs: HashSet<PathBuf> = HashSet::new();

    // Primero los movimientos, para que los «link» que dependen de ellos encuentren el fichero
    let mut ordered: Vec<&NormalizeItem> = items.iter().collect();
    ordered.sort_by_key(|i| i.status != NormalizeStatus::Move);

    for item in ordered {
        let from = PathBuf::from(&item.from);
        let to = PathBuf::from(&item.to);

        if from != to && !moved.contains(&(from.clone(), to.clone())) {
            if to.exists() || !from.is_file() {
                result.skipped.push(item.from_relative.clone());
                continue;
            }
            if let Err(e) = media::move_file(&from, &to) {
                result.skipped.push(format!("{} ({})", item.from_relative, e));
                continue;
            }
            if let Some(dir) = from.parent() {
                emptied_dirs.insert(dir.to_path_buf());
            }
            moved.insert((from.clone(), to.clone()));
            result.moved += 1;
        } else {
            result.linked += 1;
        }

        if let Some(game) = games.iter_mut().find(|g| g.path == item.game_path) {
            media::set_field(game, &item.field, Some(media::media_ref(layout, system, &to)));
        }
    }

    // Carpetas antiguas que se han quedado vacías (p. ej. images/) fuera
    for dir in emptied_dirs {
        media::remove_empty_dirs_up_to(&dir, &system.rom_dir);
    }
    result
}

/// Qué tipo de medio es un fichero, por dónde está o cómo se llama:
/// 1. Está en una carpeta unificada (covers/, screenshots/…) → ese tipo.
/// 2. Tiene un sufijo antiguo conocido (-titlescreen, -marquee…) → el tipo del sufijo.
/// Si no se sabe, se usa el del campo que lo referencia.
fn guess_kind(system: &SystemRef, file: &Path, rom_path: &str) -> Option<&'static MediaKind> {
    if let Ok(rel) = file.strip_prefix(&system.rom_dir) {
        if let Some(first) = rel.components().next() {
            if let Some(kind) = media::kind_for_folder(&first.as_os_str().to_string_lossy()) {
                return Some(kind);
            }
        }
    }

    let file_stem = file.file_stem()?.to_string_lossy().to_lowercase();
    let rom_stem = crate::layout::rom_media_stem(system, rom_path);
    let rom_stem = rom_stem.file_name()?.to_string_lossy().to_lowercase();
    let suffix = file_stem.strip_prefix(&rom_stem)?;
    let folder = match suffix {
        "-image" | "-boxart" | "-box2d" | "-box-2d" | "-cover" | "-front" => "covers",
        "-thumbnail" | "-thumb" | "-screenshot" | "-ss" | "-snap" => "screenshots",
        "-marquee" | "-logo" => "marquees",
        "-wheel" => "wheels",
        "-titlescreen" | "-titleshot" | "-title" => "titlescreens",
        "-box3d" | "-box-3d" | "-3dbox" => "3dboxes",
        "-boxback" | "-box-back" | "-back" => "backcovers",
        "-fanart" | "-fan" => "fanart",
        "-mix" | "-miximage" | "-mixrbv1" | "-mixrbv2" => "miximages",
        "-cartridge" | "-cart" => "physicalmedia",
        "-manual" => "manuals",
        "-video" => "videos",
        _ => return None,
    };
    media::kind_for_folder(folder)
}

/// Fichero con nombre antiguo para un campo vacío (Skraper, -image, carpeta images/…).
fn legacy_file_for(settings: &AppSettings, system: &SystemRef, rom_path: &str, field: &str) -> Option<PathBuf> {
    let found = super::images::scan_legacy_media(settings, system, rom_path);
    found.get(field).map(|rel| media::resolve_media_path(system, rel))
}

/// Ruta corta para enseñar: dentro de la carpeta del sistema, o (ES-DE) dentro de la de medios.
fn relative_to(layout: &Layout, system: &SystemRef, path: &Path) -> String {
    let media_root = match layout {
        Layout::Esde(paths) => Some(paths.media_root.as_path()),
        Layout::Classic => None,
    };
    path.strip_prefix(&system.rom_dir)
        .ok()
        .or_else(|| media_root.and_then(|root| path.strip_prefix(root).ok()))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(path: &Path, content: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn game(path: &str) -> Game {
        Game { path: path.into(), name: Some(path.into()), ..Default::default() }
    }

    #[test]
    fn mueve_sufijos_antiguos_a_su_carpeta_unificada() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("images/Juego-image.png"), b"cover");
        touch(&system.rom_dir.join("images/Juego-titlescreen.jpg"), b"title");

        let mut g = game("./Juego.sfc");
        g.image = Some("./images/Juego-image.png".into());
        // Configuración clásica de siempre: la pantalla de título va al campo thumbnail
        g.thumbnail = Some("./images/Juego-titlescreen.jpg".into());
        let mut games = vec![g];

        let items = plan(&Layout::Classic, &system, &AppSettings::default(), &games);
        let targets: Vec<_> = items.iter().map(|i| (i.field.as_str(), i.to_relative.as_str())).collect();
        assert!(targets.contains(&("image", "covers/Juego.png")));
        assert!(targets.contains(&("thumbnail", "titlescreens/Juego.jpg")));
        // El campo titleshot estaba vacío: se rellena con la misma pantalla de título (solo enlace)
        let titleshot = items.iter().find(|i| i.field == "titleshot").unwrap();
        assert_eq!(titleshot.status, NormalizeStatus::Link);
        assert_eq!(items.iter().filter(|i| i.status == NormalizeStatus::Move).count(), 2);

        let result = apply(&Layout::Classic, &system, &mut games, &items);
        assert_eq!((result.moved, result.linked), (2, 1));
        assert_eq!(games[0].titleshot.as_deref(), Some("./titlescreens/Juego.jpg"));
        assert_eq!(games[0].image.as_deref(), Some("./covers/Juego.png"));
        assert_eq!(games[0].thumbnail.as_deref(), Some("./titlescreens/Juego.jpg"));
        assert!(system.rom_dir.join("covers/Juego.png").exists());
        // La carpeta images/ se ha quedado vacía y se borra
        assert!(!system.rom_dir.join("images").exists());
    }

    #[test]
    fn encuentra_medios_antiguos_aunque_el_campo_este_vacio() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("images/Juego-marquee.png"), b"m");
        let games = vec![game("./Juego.sfc")];

        let items = plan(&Layout::Classic, &system, &AppSettings::default(), &games);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].field, "marquee");
        assert_eq!(items[0].to_relative, "marquees/Juego.png");
    }

    #[test]
    fn ya_unificado_solo_enlaza_el_campo() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("covers/Juego.png"), b"c");
        let mut games = vec![game("./Juego.sfc")];

        let items = plan(&Layout::Classic, &system, &AppSettings::default(), &games);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, NormalizeStatus::Link);
        apply(&Layout::Classic, &system, &mut games, &items);
        assert_eq!(games[0].image.as_deref(), Some("./covers/Juego.png"));

        // Y una segunda pasada ya no propone nada
        assert!(plan(&Layout::Classic, &system, &AppSettings::default(), &games).is_empty());
    }

    #[test]
    fn destino_ocupado_por_otro_fichero_es_conflicto_y_no_se_toca() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("covers/Juego.png"), b"otro");
        touch(&system.rom_dir.join("images/Juego-image.png"), b"este");
        let mut g = game("./Juego.sfc");
        g.image = Some("./images/Juego-image.png".into());
        let mut games = vec![g];

        let items = plan(&Layout::Classic, &system, &AppSettings::default(), &games);
        assert_eq!(items[0].status, NormalizeStatus::Conflict);
        let result = apply(&Layout::Classic, &system, &mut games, &items);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(fs::read(system.rom_dir.join("covers/Juego.png")).unwrap(), b"otro");
        assert_eq!(games[0].image.as_deref(), Some("./images/Juego-image.png"));
    }

    #[test]
    fn el_mismo_fichero_en_dos_campos_se_mueve_una_vez() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("images/Juego-image.png"), b"c");
        let mut g = game("./Juego.sfc");
        g.image = Some("./images/Juego-image.png".into());
        g.boxart = Some("./images/Juego-image.png".into());
        let mut games = vec![g];

        let items = plan(&Layout::Classic, &system, &AppSettings::default(), &games);
        assert_eq!(items.len(), 2);
        let result = apply(&Layout::Classic, &system, &mut games, &items);
        assert_eq!((result.moved, result.linked), (1, 1));
        assert_eq!(games[0].boxart.as_deref(), Some("./covers/Juego.png"));
    }

    fn esde(root: &Path) -> Layout {
        Layout::Esde(crate::layout::EsdePaths {
            home: root.join("ES-DE"),
            media_root: root.join("ES-DE/downloaded_media"),
        })
    }

    #[test]
    fn el_sufijo_boxart_va_solo_a_portada() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("snes"));
        touch(&system.rom_dir.join("images/Juego-boxart.jpg"), b"c");
        let games = vec![game("./Juego.sfc")];

        let items = plan(&Layout::Classic, &system, &AppSettings::default(), &games);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].field, "image");
        assert_eq!(items[0].to_relative, "covers/Juego.jpg");
    }

    #[test]
    fn en_es_de_recupera_los_medios_clasicos_de_la_carpeta_de_roms() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/cps1"));
        touch(&system.rom_dir.join("pang3.zip"), b"rom");
        touch(&system.rom_dir.join("images/pang3-boxart.jpg"), b"caja");
        touch(&system.rom_dir.join("images/pang3-screenshot.jpg"), b"captura");
        let mut games = vec![game("./pang3.zip")];

        let items = plan(&layout, &system, &AppSettings::default(), &games);
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.status == NormalizeStatus::Move));
        assert!(items.iter().any(|i| i.to_relative == "cps1/covers/pang3.jpg"));

        let result = apply(&layout, &system, &mut games, &items);
        assert_eq!(result.moved, 2);
        let media = tmp.path().join("ES-DE/downloaded_media/cps1");
        assert_eq!(fs::read(media.join("covers/pang3.jpg")).unwrap(), b"caja");
        assert_eq!(fs::read(media.join("screenshots/pang3.jpg")).unwrap(), b"captura");
        assert!(!system.rom_dir.join("images").exists(), "images/ queda vacía y se borra");
        assert!(system.rom_dir.join("pang3.zip").exists());
    }

    #[test]
    fn en_es_de_no_propone_nada_si_el_medio_ya_esta() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = esde(tmp.path());
        let system = SystemRef::from_rom_dir(tmp.path().join("roms/cps1"));
        touch(&system.rom_dir.join("images/pang3-boxart.jpg"), b"vieja");
        let existing = tmp.path().join("ES-DE/downloaded_media/cps1/covers/pang3.png");
        touch(&existing, b"buena");
        // Así llega el juego en ES-DE: read_gamelist rellena los medios que encuentra
        let mut g = game("./pang3.zip");
        g.image = Some(existing.to_string_lossy().to_string());

        assert!(plan(&layout, &system, &AppSettings::default(), &[g]).is_empty());
    }
}
