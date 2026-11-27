use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::layout::esde_settings::{detect_esde_paths as detect, EsdeDetectedPaths};
use crate::layout::{Layout, SystemRef};
use crate::media::{MediaIndex, MEDIA_KINDS};
use crate::models::{Profile, StructureKind};

/// Lee las rutas de ROMs y medios de una carpeta de ES-DE (su es_settings.xml)
/// para rellenar un perfil ES-DE desde la pantalla de ajustes.
#[tauri::command(async)]
pub fn detect_esde_paths(esde_home: String) -> Result<EsdeDetectedPaths, String> {
    let home = Path::new(&esde_home);
    if !home.is_dir() {
        return Err(format!("Not a folder: {}", esde_home));
    }
    detect(home)
}

/// Systems of the active profile whose data is in the OTHER structure and that have
/// nothing in the profile's own structure, plus the (maybe unsaved) profile to migrate from.
#[derive(Debug, Clone, Serialize)]
pub struct PendingStructure {
    pub source: Profile,
    pub systems: Vec<String>,
}

/// Looks for systems to migrate when a profile is activated:
/// - ES-DE profile: classic gamelist.xml in the system folder and no ES-DE gamelist.
/// - Classic profile: no gamelist.xml in the system folder, but an ES-DE one in an ES-DE
///   that uses the same ROM folder (saved ES-DE profiles first, then ~/ES-DE).
/// Systems with data in both structures are left alone.
#[tauri::command(async)]
pub fn detect_pending_structure(app: tauri::AppHandle) -> Result<Option<PendingStructure>, String> {
    let settings = super::settings::get_settings(app)?;
    let Some(active) = settings.active_profile() else { return Ok(None) };
    // ~/ES-DE (HOME en macOS/Linux, USERPROFILE en Windows)
    let home = crate::layout::esde_settings::home_dir();
    let default_esde_home = (!home.as_os_str().is_empty()).then(|| home.join("ES-DE"));
    Ok(find_pending(active, &settings.profiles, default_esde_home.as_deref()))
}

fn find_pending(active: &Profile, profiles: &[Profile], default_esde_home: Option<&Path>) -> Option<PendingStructure> {
    let target = Layout::for_profile(active).ok()?;
    let systems: Vec<SystemRef> = super::migrate::list_system_dirs(Path::new(&active.rom_root_path), &[&target])
        .ok()?
        .iter()
        .map(SystemRef::from_rom_dir)
        .collect();

    let candidates = match active.kind {
        // The classic structure lives in the same ROM folder: nothing to look for
        StructureKind::Esde => vec![detected_profile(active, StructureKind::Classic, None, None)],
        StructureKind::Classic => esde_candidates(active, profiles, default_esde_home),
    };

    candidates.into_iter().find_map(|source| {
        let src = Layout::for_profile(&source).ok()?;
        let pending: Vec<String> = systems
            .iter()
            .filter(|s| !src.is_own_folder(&s.rom_dir))
            .filter(|s| src.gamelist_path(s).is_file() && (!target.gamelist_path(s).is_file() || media_missing(&src, &target, s)))
            .map(|s| s.name.clone())
            .collect();
        (!pending.is_empty()).then_some(PendingStructure { source, systems: pending })
    })
}

/// Con gamelist en las dos estructuras: ¿algún juego de la gamelist de origen tiene en el origen
/// un medio que en el destino no tiene? (medios de ES-DE borrados, una migración a medias…).
/// Solo los tipos que el destino admite; el de origen se busca como al migrar (nombre antiguo
/// incluido), el de destino por el nombre de la ROM.
fn media_missing(src: &Layout, target: &Layout, system: &SystemRef) -> bool {
    let Ok(gamelist) = super::gamelist::read_gamelist(src, system) else { return false };
    let src_index = MediaIndex::build(src, system);
    let dst_index = MediaIndex::build(target, system);
    let kinds: Vec<_> = MEDIA_KINDS.iter().filter(|k| k.in_esde || target.writes_media_tags()).collect();
    gamelist.games.iter().any(|game| {
        kinds.iter().any(|kind| {
            super::media_kinds::current_file(src, system, &src_index, game, kind).is_some() && dst_index.find(system, kind, &game.path).is_none()
        })
    })
}

/// ES-DE installations that read their ROMs from the active profile's folder.
fn esde_candidates(active: &Profile, profiles: &[Profile], default_esde_home: Option<&Path>) -> Vec<Profile> {
    let mut found: Vec<Profile> = profiles
        .iter()
        .filter(|p| p.kind == StructureKind::Esde && same_dir(&p.rom_root_path, &active.rom_root_path))
        .filter(|p| p.esde_home_path.is_some())
        .cloned()
        .collect();

    if let Some(home) = default_esde_home.filter(|h| h.is_dir()) {
        let already_known = found
            .iter()
            .any(|p| p.esde_home_path.as_deref().is_some_and(|h| same_dir(h, &home.to_string_lossy())));
        if let (false, Ok(paths)) = (already_known, detect(home)) {
            if same_dir(&paths.rom_root_path, &active.rom_root_path) {
                found.push(detected_profile(active, StructureKind::Esde, Some(home), Some(paths.media_dir)));
            }
        }
    }
    found
}

/// Profile that is not saved in the settings: only used as migration source.
fn detected_profile(active: &Profile, kind: StructureKind, esde_home: Option<&Path>, media_dir: Option<String>) -> Profile {
    Profile {
        id: format!("detected-{}", if kind == StructureKind::Esde { "esde" } else { "classic" }),
        name: esde_home.map(|h| h.to_string_lossy().to_string()).unwrap_or_default(),
        kind,
        rom_root_path: active.rom_root_path.clone(),
        esde_home_path: esde_home.map(|h| h.to_string_lossy().to_string()),
        media_dir,
        skip_structure_prompt: false,
        last_system: None,
        bios_dir: None,
        saves_dir: None,
        set_aside_dir: None,
        system_views: None,
    }
}

/// Same folder even if written differently (trailing slash, symlinks).
fn same_dir(a: &str, b: &str) -> bool {
    let canon = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p.trim_end_matches('/')));
    canon(a) == canon(b)
}

/// Is the profile's library reachable right now? Its ROM folder (and, in ES-DE, the ES-DE folder)
/// must exist: a profile on an SD card or a USB disk that isn't plugged in is not available.
fn is_available(profile: &Profile) -> bool {
    let exists = |path: &str| !path.trim().is_empty() && Path::new(path).is_dir();
    let esde_ok = match (&profile.kind, profile.esde_home_path.as_deref()) {
        (StructureKind::Esde, Some(home)) if !home.trim().is_empty() => exists(home),
        _ => true,
    };
    exists(&profile.rom_root_path) && esde_ok
}

/// Which profiles are available now (by id). Only checks that the folders exist, so an unmounted
/// SD answers at once; it runs on a thread in case a network drive is slow.
#[tauri::command(async)]
pub fn profiles_available(profiles: Vec<Profile>) -> std::collections::HashMap<String, bool> {
    profiles.iter().map(|p| (p.id.clone(), is_available(p))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"<gameList/>").unwrap();
    }

    fn profile(id: &str, kind: StructureKind, roms: &Path, esde_home: Option<&Path>) -> Profile {
        Profile {
            id: id.into(),
            name: id.into(),
            kind,
            rom_root_path: roms.to_string_lossy().to_string(),
            esde_home_path: esde_home.map(|h| h.to_string_lossy().to_string()),
            media_dir: None,
            skip_structure_prompt: false,
            last_system: None,
            bios_dir: None,
            saves_dir: None,
            set_aside_dir: None,
            system_views: None,
        }
    }

    #[test]
    fn esde_profile_finds_classic_systems_and_skips_mixed_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let roms = tmp.path().join("ROMs");
        let home = tmp.path().join("ES-DE");
        touch(&roms.join("snes/gamelist.xml")); // only classic → pending
        touch(&roms.join("psx/gamelist.xml")); // both → left alone
        touch(&home.join("gamelists/psx/gamelist.xml"));
        fs::create_dir_all(roms.join("n64")).unwrap(); // nothing at all

        let active = profile("esde", StructureKind::Esde, &roms, Some(&home));
        let pending = find_pending(&active, &[active.clone()], None).unwrap();
        assert_eq!(pending.systems, vec!["snes"]);
        assert_eq!(pending.source.kind, StructureKind::Classic);
    }

    #[test]
    fn esde_profile_with_media_only_in_the_classic_structure_is_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let roms = tmp.path().join("ROMs");
        let home = tmp.path().join("ES-DE");
        let gamelist = r#"<gameList><game><path>./Sonic.md</path><name>Sonic</name><image>./covers/Sonic.png</image></game></gameList>"#;
        for sys in ["megadrive", "snes", "n64"] {
            fs::create_dir_all(roms.join(sys)).unwrap();
            fs::write(roms.join(format!("{sys}/gamelist.xml")), gamelist).unwrap();
            fs::create_dir_all(home.join(format!("gamelists/{sys}"))).unwrap();
            fs::write(home.join(format!("gamelists/{sys}/gamelist.xml")), gamelist).unwrap();
        }
        // megadrive: the cover only in the classic structure → pending
        touch(&roms.join("megadrive/covers/Sonic.png"));
        // snes: the cover in both → left alone
        touch(&roms.join("snes/covers/Sonic.png"));
        touch(&home.join("downloaded_media/snes/covers/Sonic.png"));
        // n64: only a wheel, which ES-DE doesn't have → left alone
        touch(&roms.join("n64/wheels/Sonic.png"));

        let active = profile("esde", StructureKind::Esde, &roms, Some(&home));
        let pending = find_pending(&active, &[active.clone()], None).unwrap();
        assert_eq!(pending.systems, vec!["megadrive"]);
    }

    #[test]
    fn classic_profile_finds_es_de_data_in_saved_profiles_or_default_home() {
        let tmp = tempfile::tempdir().unwrap();
        let roms = tmp.path().join("ROMs");
        let home = tmp.path().join("ES-DE");
        fs::create_dir_all(roms.join("snes")).unwrap();
        touch(&home.join("gamelists/snes/gamelist.xml"));
        let active = profile("classic", StructureKind::Classic, &roms, None);

        // Saved ES-DE profile on the same ROM folder
        let saved = profile("esde", StructureKind::Esde, &roms, Some(&home));
        let pending = find_pending(&active, &[active.clone(), saved], None).unwrap();
        assert_eq!(pending.source.id, "esde");
        assert_eq!(pending.systems, vec!["snes"]);

        // ~/ES-DE without profile, whose es_settings.xml points to this ROM folder
        touch(&home.join("settings/es_settings.xml"));
        fs::write(
            home.join("settings/es_settings.xml"),
            format!(r#"<string name="ROMDirectory" value="{}" />"#, roms.display()),
        )
        .unwrap();
        let pending = find_pending(&active, &[active.clone()], Some(&home)).unwrap();
        assert_eq!(pending.source.id, "detected-esde");
        assert_eq!(pending.source.esde_home_path.as_deref(), Some(home.to_string_lossy().as_ref()));
    }

    #[test]
    fn es_de_reading_another_rom_folder_is_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let roms = tmp.path().join("ROMs");
        let home = tmp.path().join("ES-DE");
        fs::create_dir_all(roms.join("snes")).unwrap();
        touch(&home.join("gamelists/snes/gamelist.xml"));
        touch(&home.join("settings/es_settings.xml"));
        fs::write(home.join("settings/es_settings.xml"), r#"<string name="ROMDirectory" value="/otra/carpeta" />"#).unwrap();

        let active = profile("classic", StructureKind::Classic, &roms, None);
        let other = profile("esde", StructureKind::Esde, &tmp.path().join("Otras"), Some(&home));
        assert!(find_pending(&active, &[active.clone(), other], Some(&home)).is_none());
    }

    #[test]
    fn a_profile_whose_folders_are_gone_is_not_available() {
        let tmp = tempfile::tempdir().unwrap();
        let roms = tmp.path().join("roms");
        std::fs::create_dir_all(&roms).unwrap();
        let sd = tmp.path().join("Volumes/GAMES");
        assert!(is_available(&profile("a", StructureKind::Classic, &roms, None)));
        assert!(!is_available(&profile("b", StructureKind::Classic, &sd, None)), "SD not plugged in");
        // ES-DE: también hace falta la carpeta de ES-DE
        assert!(!is_available(&profile("c", StructureKind::Esde, &roms, Some(&sd))));
        assert!(is_available(&profile("d", StructureKind::Esde, &roms, Some(&roms))));
        let found = profiles_available(vec![profile("a", StructureKind::Classic, &roms, None), profile("b", StructureKind::Classic, &sd, None)]);
        assert_eq!((found["a"], found["b"]), (true, false));
    }
}
