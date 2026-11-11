//! Dónde está cada emulador y cada core, siguiendo las reglas de `es_find_rules.xml` en su
//! orden (la primera que encuentra algo gana), como ES-DE.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::config::{FindRules, Rule, RuleKind};

/// Lo que hace falta para resolver las rutas de las reglas
#[derive(Debug, Clone, Default)]
pub struct FindContext {
    /// %ESPATH%: carpeta del ejecutable de ES-DE (solo se conoce en la versión portable)
    pub es_path: Option<PathBuf>,
    /// Carpeta personal, para las rutas con «~»
    pub home: PathBuf,
    /// El PATH donde buscar las reglas «systempath»
    pub path_var: Option<OsString>,
}

impl FindContext {
    /// El de este equipo (carpeta personal y PATH reales)
    pub fn current(es_path: Option<PathBuf>) -> Self {
        FindContext {
            es_path,
            home: crate::layout::esde_settings::home_dir(),
            path_var: std::env::var_os("PATH"),
        }
    }
}

/// Un emulador (o core) que no se encuentra: su nombre y por dónde se ha buscado
#[derive(Debug, Clone, PartialEq)]
pub struct NotFound {
    pub name: String,
    pub searched: Vec<String>,
}

/// Busca el ejecutable de un emulador (RETROARCH, SNES9X…)
pub fn find_emulator(rules: &FindRules, name: &str, ctx: &FindContext) -> Result<PathBuf, NotFound> {
    let mut searched = Vec::new();
    for rule in rules.emulators.get(name).map(Vec::as_slice).unwrap_or_default() {
        if let Some(found) = apply_rule(rule, ctx, None, &mut searched) {
            return Ok(found);
        }
    }
    Err(NotFound { name: name.to_string(), searched })
}

/// Busca un core (`file`, p. ej. «snes9x_libretro.dylib») en las carpetas de cores de `name`;
/// `emu_dir` es la carpeta del emulador ya encontrado (%EMUPATH% en las reglas)
pub fn find_core(rules: &FindRules, name: &str, file: &str, emu_dir: Option<&Path>, ctx: &FindContext) -> Result<PathBuf, NotFound> {
    let mut searched = Vec::new();
    for rule in rules.cores.get(name).map(Vec::as_slice).unwrap_or_default() {
        if rule.kind != RuleKind::CorePath {
            continue;
        }
        for entry in &rule.entries {
            let Some(dir) = expand_entry(entry, ctx, emu_dir) else { continue };
            let candidate = dir.join(file.trim_start_matches(['/', '\\']));
            searched.push(candidate.to_string_lossy().to_string());
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(NotFound { name: format!("{} ({})", name, file.trim_start_matches(['/', '\\'])), searched })
}

/// Aplica una regla de emulador: la primera ruta que existe, o nada (apunta lo que ha mirado)
fn apply_rule(rule: &Rule, ctx: &FindContext, emu_dir: Option<&Path>, searched: &mut Vec<String>) -> Option<PathBuf> {
    match rule.kind {
        RuleKind::StaticPath => rule.entries.iter().find_map(|entry| {
            let path = expand_entry(entry, ctx, emu_dir)?;
            searched.push(path.to_string_lossy().to_string());
            find_file(&path)
        }),
        RuleKind::SystemPath => {
            let dirs: Vec<PathBuf> = ctx.path_var.as_ref().map(|p| std::env::split_paths(p).collect()).unwrap_or_default();
            rule.entries.iter().find_map(|entry| {
                searched.push(format!("PATH: {}", entry));
                dirs.iter().map(|dir| dir.join(entry)).find(|p| p.is_file())
            })
        }
        RuleKind::WinRegistryPath => rule.entries.iter().find_map(|entry| {
            searched.push(format!("App Paths: {}", entry));
            registry::app_path(entry).filter(|p| p.is_file())
        }),
        RuleKind::WinRegistryValue => rule.entries.iter().find_map(|entry| {
            searched.push(format!("Registry: {}", entry));
            registry::value_path(entry).filter(|p| p.is_file())
        }),
        RuleKind::CorePath => None,
    }
}

/// Ruta de una regla ya resuelta: «~», %ESPATH% y %EMUPATH%. Sin el valor de alguna de ellas
/// (p. ej. %ESPATH% sin ES-DE portable), la entrada no sirve.
fn expand_entry(entry: &str, ctx: &FindContext, emu_dir: Option<&Path>) -> Option<PathBuf> {
    let mut text = entry.to_string();
    if text.contains("%ESPATH%") {
        text = text.replace("%ESPATH%", &ctx.es_path.as_ref()?.to_string_lossy());
    }
    if text.contains("%EMUPATH%") {
        text = text.replace("%EMUPATH%", &emu_dir?.to_string_lossy());
    }
    Some(match text.strip_prefix('~') {
        Some(rest) => ctx.home.join(rest.trim_start_matches(['/', '\\'])),
        None => PathBuf::from(text),
    })
}

/// La ruta si existe; con «*» en el nombre del fichero, el primero que encaje (por orden)
fn find_file(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().to_string();
    if !name.contains('*') {
        return path.is_file().then(|| path.to_path_buf());
    }
    let mut matches: Vec<PathBuf> = std::fs::read_dir(path.parent()?)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.file_name().is_some_and(|n| wildcard(&name, &n.to_string_lossy())))
        .collect();
    matches.sort();
    matches.into_iter().next()
}

/// ¿Encaja `text` con `pattern` («*» = cualquier cosa; sin distinguir mayúsculas)?
fn wildcard(pattern: &str, text: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let text = text.to_lowercase();
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = text.as_str();
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            match rest.strip_prefix(part) {
                Some(r) => rest = r,
                None => return false,
            }
        } else if i == parts.len() - 1 {
            return rest.ends_with(part);
        } else {
            match rest.find(part) {
                Some(at) => rest = &rest[at + part.len()..],
                None => return false,
            }
        }
    }
    rest.is_empty()
}

/// Registro de Windows (en los demás sistemas estas reglas no encuentran nada)
mod registry {
    use std::path::PathBuf;

    /// «App Paths» del ejecutable: primero el usuario, luego el equipo
    #[cfg(windows)]
    pub fn app_path(exe: &str) -> Option<PathBuf> {
        let key = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{}", exe);
        read_string(&key, "").map(PathBuf::from)
    }

    /// «clave\valor|resto»: el valor del registro con el resto de la ruta detrás
    #[cfg(windows)]
    pub fn value_path(entry: &str) -> Option<PathBuf> {
        let (key_value, suffix) = entry.split_once('|').unwrap_or((entry, ""));
        let (key, value) = key_value.rsplit_once('\\')?;
        let base = read_string(key, value)?;
        let suffix = suffix.trim_start_matches(['\\', '/']);
        Some(if suffix.is_empty() { PathBuf::from(base) } else { PathBuf::from(base).join(suffix) })
    }

    #[cfg(windows)]
    fn read_string(key: &str, value: &str) -> Option<String> {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;
        [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE].into_iter().find_map(|root| {
            RegKey::predef(root).open_subkey(key).ok()?.get_value::<String, _>(value).ok()
        })
    }

    #[cfg(not(windows))]
    pub fn app_path(_exe: &str) -> Option<PathBuf> {
        None
    }

    #[cfg(not(windows))]
    pub fn value_path(_entry: &str) -> Option<PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::config::parse_find_rules;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    #[test]
    fn ruta_fija_con_casa_comodin_y_espath() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let es = tmp.path().join("es");
        touch(&home.join("Apps/RetroArch-1.19.AppImage"));
        touch(&es.join("Emulators/pcsx2-qt-v2.exe"));
        let rules = parse_find_rules(
            r#"<ruleList>
              <emulator name="RA"><rule type="staticpath"><entry>/no/existe</entry><entry>~/Apps/RetroArch*.AppImage</entry></rule></emulator>
              <emulator name="PCSX2"><rule type="staticpath"><entry>%ESPATH%/Emulators/pcsx2-qt*.exe</entry></rule></emulator>
            </ruleList>"#,
        )
        .unwrap();
        let ctx = FindContext { es_path: Some(es.clone()), home: home.clone(), path_var: None };
        assert_eq!(find_emulator(&rules, "RA", &ctx).unwrap(), home.join("Apps/RetroArch-1.19.AppImage"));
        assert_eq!(find_emulator(&rules, "PCSX2", &ctx).unwrap(), es.join("Emulators/pcsx2-qt-v2.exe"));
        // Sin %ESPATH% conocido esa entrada no sirve
        let no_es = FindContext { es_path: None, ..ctx };
        assert!(find_emulator(&rules, "PCSX2", &no_es).is_err());
    }

    #[test]
    fn en_el_path_y_lo_que_se_busco() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        touch(&bin.join("mednafen"));
        let rules = parse_find_rules(
            r#"<ruleList><emulator name="MEDNAFEN"><rule type="staticpath"><entry>/nada/mednafen</entry></rule><rule type="systempath"><entry>mednafen</entry></rule></emulator></ruleList>"#,
        )
        .unwrap();
        let ctx = FindContext { es_path: None, home: tmp.path().to_path_buf(), path_var: Some(bin.clone().into_os_string()) };
        assert_eq!(find_emulator(&rules, "MEDNAFEN", &ctx).unwrap(), bin.join("mednafen"));

        let empty = FindContext { path_var: None, ..ctx };
        let missing = find_emulator(&rules, "MEDNAFEN", &empty).unwrap_err();
        assert_eq!(missing.name, "MEDNAFEN");
        assert_eq!(missing.searched, vec!["/nada/mednafen".to_string(), "PATH: mednafen".to_string()]);
        // Un emulador sin reglas: no encontrado, sin búsquedas
        assert!(find_emulator(&rules, "OTRO", &empty).unwrap_err().searched.is_empty());
    }

    #[test]
    fn cores_junto_al_emulador_o_en_su_carpeta() {
        let tmp = tempfile::tempdir().unwrap();
        let emu_dir = tmp.path().join("RetroArch");
        touch(&emu_dir.join("cores/snes9x_libretro.dll"));
        let rules = parse_find_rules(
            r#"<ruleList><core name="RETROARCH"><rule type="corepath"><entry>~/no-hay</entry><entry>%EMUPATH%/cores</entry></rule></core></ruleList>"#,
        )
        .unwrap();
        let ctx = FindContext { es_path: None, home: tmp.path().to_path_buf(), path_var: None };
        let core = find_core(&rules, "RETROARCH", "/snes9x_libretro.dll", Some(&emu_dir), &ctx).unwrap();
        assert_eq!(core, emu_dir.join("cores/snes9x_libretro.dll"));
        let missing = find_core(&rules, "RETROARCH", "/bsnes_libretro.dll", Some(&emu_dir), &ctx).unwrap_err();
        assert_eq!(missing.name, "RETROARCH (bsnes_libretro.dll)");
        assert_eq!(missing.searched.len(), 2);
    }

    #[test]
    fn comodines() {
        assert!(wildcard("pcsx2-qt*.exe", "pcsx2-qt-avx2.exe"));
        assert!(wildcard("RetroArch*.AppImage", "retroarch-x86_64.appimage"));
        assert!(!wildcard("pcsx2-qt*.exe", "pcsx2.exe"));
        assert!(wildcard("a*b*c", "aXXbYYc"));
    }
}
