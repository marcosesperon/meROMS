//! Del comando de `es_systems.xml` (o el propio del usuario) al comando que se ejecuta: busca el
//! emulador y el core, y sustituye las variables de ES-DE (%ROM%, %BASENAME%, %STARTDIR%=…).

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use super::config::FindRules;
use super::find::{find_core, find_emulator, FindContext, NotFound};

/// El comando listo para ejecutar
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Prepared {
    /// La línea de comandos final (la que se ejecuta y la que se enseña en «Probar»)
    pub command: String,
    /// Carpeta desde la que se lanza (%STARTDIR%=), si el comando la pide
    pub start_dir: Option<PathBuf>,
}

/// Lo que falta para lanzar: un emulador o un core, con por dónde se ha buscado
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Missing {
    /// "emulator" | "core"
    pub kind: &'static str,
    pub name: String,
    pub searched: Vec<String>,
}

impl Missing {
    fn from(kind: &'static str, nf: NotFound) -> Self {
        Missing { kind, name: nf.name, searched: nf.searched }
    }
}

static EMULATOR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"%(EMULATOR|PRECOMMAND)_([A-Za-z0-9_\-]+)%").unwrap());
static CORE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"%CORE_([A-Za-z0-9_\-]+)%([^\s"]*)"#).unwrap());
static STARTDIR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"%STARTDIR%=("[^"]*"|\S+)\s*"#).unwrap());
static INJECT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"%INJECT%=("[^"]*"|\S+)"#).unwrap());
static FLAGS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"%(HIDEWINDOW|ESCAPESPECIALS|RUNINBACKGROUND|ENABLESHORTCUTS)%\s*").unwrap());
static SPACES_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" {2,}").unwrap());

/// Un fichero de %INJECT% más grande que esto no se lee (como ES-DE)
const INJECT_MAX: u64 = 4096;

/// Prepara `command` para la ROM `rom` (ruta absoluta) de la carpeta de ROMs `rom_root`
pub fn prepare(command: &str, rules: &FindRules, ctx: &FindContext, rom: &Path, rom_root: &Path) -> Result<Prepared, Missing> {
    let mut cmd = command.to_string();
    if let Some(es) = &ctx.es_path {
        cmd = cmd.replace("%ESPATH%", &es.to_string_lossy());
    }

    // Emuladores (y los «precomandos» de Linux, que son lo mismo): ruta encontrada, escapada.
    // La carpeta del primero es %EMUDIR% (y donde se buscan sus cores con %EMUPATH%)
    let mut emu_dir: Option<PathBuf> = None;
    for caps in EMULATOR_RE.captures_iter(&cmd.clone()) {
        let found = find_emulator(rules, &caps[2], ctx).map_err(|nf| Missing::from("emulator", nf))?;
        if emu_dir.is_none() && &caps[1] == "EMULATOR" {
            emu_dir = found.parent().map(Path::to_path_buf);
        }
        cmd = cmd.replace(&caps[0], &escape(&found.to_string_lossy()));
    }

    // Cores: %CORE_RETROARCH%/snes9x_libretro.dylib → la ruta del core, si está
    for caps in CORE_RE.captures_iter(&cmd.clone()) {
        let found = find_core(rules, &caps[1], &caps[2], emu_dir.as_deref(), ctx).map_err(|nf| Missing::from("core", nf))?;
        cmd = cmd.replace(&caps[0], &escape(&found.to_string_lossy()));
    }

    let game_dir = rom.parent().map(Path::to_path_buf).unwrap_or_default();
    let base_name = rom.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();

    // %STARTDIR%=carpeta: desde dónde se lanza (se quita del comando)
    let mut start_dir = None;
    if let Some(caps) = STARTDIR_RE.captures(&cmd.clone()) {
        let value = caps[1].trim_matches('"');
        let value = value
            .replace("%EMUDIR%", &emu_dir.as_deref().unwrap_or(Path::new("")).to_string_lossy())
            .replace("%GAMEDIR%", &game_dir.to_string_lossy())
            .replace("%GAMEENTRYDIR%", &game_dir.to_string_lossy());
        start_dir = Some(match value.strip_prefix('~') {
            Some(rest) => ctx.home.join(rest.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(value),
        });
        cmd = cmd.replace(&caps[0], "");
    }

    // %INJECT%=fichero: sus argumentos van al comando (si existe y es pequeño)
    if let Some(caps) = INJECT_RE.captures(&cmd.clone()) {
        let name = caps[1].trim_matches('"').replace("%BASENAME%", &base_name);
        let file = if Path::new(&name).is_absolute() { PathBuf::from(&name) } else { game_dir.join(&name) };
        let injected = std::fs::metadata(&file)
            .ok()
            .filter(|m| m.is_file() && m.len() <= INJECT_MAX)
            .and_then(|_| std::fs::read_to_string(&file).ok())
            .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_default();
        cmd = cmd.replace(&caps[0], &injected);
    }

    // Opciones de ES-DE que aquí no cambian nada
    cmd = FLAGS_RE.replace_all(&cmd, "").to_string();

    let rom_text = rom.to_string_lossy();
    let game_dir_text = game_dir.to_string_lossy();
    let file_name = rom.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let replacements: [(&str, String); 11] = [
        ("%ROMRAWWIN%", rom_text.replace('/', "\\")),
        ("%ROMRAW%", rom_text.to_string()),
        ("%ROMPATH%", escape(&rom_root.to_string_lossy())),
        ("%ROM%", escape(&rom_text)),
        ("%BASENAME%", base_name.clone()),
        ("%FILENAME%", file_name),
        ("%GAMEDIRRAW%", game_dir_text.to_string()),
        ("%GAMEDIR%", escape(&game_dir_text)),
        ("%GAMEENTRYDIR%", escape(&game_dir_text)),
        ("%EMUDIR%", escape(&emu_dir.as_deref().unwrap_or(Path::new("")).to_string_lossy())),
        ("%EMUPATH%", escape(&emu_dir.as_deref().unwrap_or(Path::new("")).to_string_lossy())),
    ];
    for (token, value) in &replacements {
        cmd = cmd.replace(token, value);
    }

    let command = SPACES_RE.replace_all(cmd.trim(), " ").to_string();
    Ok(Prepared { command, start_dir })
}

/// Ruta preparada para la línea de comandos, como ES-DE: en Windows entre comillas (y con «\»);
/// en macOS y Linux, con «\» delante de cada carácter que el shell trataría como especial
pub fn escape(path: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", path.replace('/', "\\"))
    } else {
        let mut out = String::with_capacity(path.len() + 8);
        for c in path.chars() {
            if !(c.is_alphanumeric() || "/._-+,:@=~%".contains(c)) {
                out.push('\\');
            }
            out.push(c);
        }
        out
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

    /// Un RetroArch y su core falsos en una carpeta temporal
    fn setup(tmp: &Path) -> (FindRules, FindContext) {
        touch(&tmp.join("apps/RetroArch"));
        touch(&tmp.join("cores/snes9x_libretro.so"));
        let rules = parse_find_rules(&format!(
            r#"<ruleList>
              <emulator name="RETROARCH"><rule type="staticpath"><entry>{0}/apps/RetroArch</entry></rule></emulator>
              <core name="RETROARCH"><rule type="corepath"><entry>{0}/cores</entry></rule></core>
            </ruleList>"#,
            tmp.display()
        ))
        .unwrap();
        (rules, FindContext { es_path: None, home: tmp.to_path_buf(), path_var: None })
    }

    #[cfg(not(windows))]
    #[test]
    fn retroarch_con_core_y_rom_con_espacios() {
        let tmp = tempfile::tempdir().unwrap();
        let (rules, ctx) = setup(tmp.path());
        let rom = tmp.path().join("roms/snes/Super Mario World (USA).sfc");
        let prepared = prepare(
            "%EMULATOR_RETROARCH% -L %CORE_RETROARCH%/snes9x_libretro.so %ROM%",
            &rules,
            &ctx,
            &rom,
            &tmp.path().join("roms"),
        )
        .unwrap();
        let t = tmp.path().display().to_string();
        assert_eq!(
            prepared.command,
            format!("{t}/apps/RetroArch -L {t}/cores/snes9x_libretro.so {t}/roms/snes/Super\\ Mario\\ World\\ \\(USA\\).sfc")
        );
        assert_eq!(prepared.start_dir, None);
    }

    #[test]
    fn falta_el_emulador_o_el_core() {
        let tmp = tempfile::tempdir().unwrap();
        let (rules, ctx) = setup(tmp.path());
        let rom = tmp.path().join("roms/snes/a.sfc");
        let root = tmp.path().join("roms");
        let missing = prepare("%EMULATOR_SNES9X% %ROM%", &rules, &ctx, &rom, &root).unwrap_err();
        assert_eq!((missing.kind, missing.name.as_str()), ("emulator", "SNES9X"));
        let missing = prepare("%EMULATOR_RETROARCH% -L %CORE_RETROARCH%/bsnes_libretro.so %ROM%", &rules, &ctx, &rom, &root).unwrap_err();
        assert_eq!((missing.kind, missing.name.as_str()), ("core", "RETROARCH (bsnes_libretro.so)"));
    }

    #[cfg(not(windows))]
    #[test]
    fn variables_startdir_inject_y_opciones() {
        let tmp = tempfile::tempdir().unwrap();
        let (rules, ctx) = setup(tmp.path());
        let rom = tmp.path().join("roms/amiga/Juego.lha");
        std::fs::create_dir_all(rom.parent().unwrap()).unwrap();
        std::fs::write(tmp.path().join("roms/amiga/Juego.commands"), "--opcion 1\n--otra\n").unwrap();
        let prepared = prepare(
            "%HIDEWINDOW% %STARTDIR%=%EMUDIR% %EMULATOR_RETROARCH% %INJECT%=%BASENAME%.commands -n %BASENAME% -f %FILENAME% -r %ROMPATH% -g %GAMEDIRRAW% %ROMRAW%",
            &rules,
            &ctx,
            &rom,
            &tmp.path().join("roms"),
        )
        .unwrap();
        let t = tmp.path().display().to_string();
        assert_eq!(prepared.start_dir, Some(tmp.path().join("apps")));
        assert_eq!(
            prepared.command,
            format!("{t}/apps/RetroArch --opcion 1 --otra -n Juego -f Juego.lha -r {t}/roms -g {t}/roms/amiga {t}/roms/amiga/Juego.lha")
        );
        // Sin el fichero de %INJECT%, la variable desaparece y ya
        let other = tmp.path().join("roms/amiga/Otro.lha");
        let prepared = prepare("%EMULATOR_RETROARCH% %INJECT%=%BASENAME%.commands %ROM%", &rules, &ctx, &other, &tmp.path().join("roms")).unwrap();
        assert_eq!(prepared.command, format!("{t}/apps/RetroArch {t}/roms/amiga/Otro.lha"));
    }

    #[test]
    fn comando_propio_sin_variables_de_emulador() {
        let tmp = tempfile::tempdir().unwrap();
        let (rules, ctx) = setup(tmp.path());
        let rom = tmp.path().join("roms/x/a.bin");
        let prepared = prepare("/usr/bin/emu --full %ROM%", &rules, &ctx, &rom, &tmp.path().join("roms")).unwrap();
        assert!(prepared.command.starts_with("/usr/bin/emu --full "));
        assert!(prepared.command.contains("a.bin"));
    }

    #[cfg(not(windows))]
    #[test]
    fn escapado_para_el_shell() {
        assert_eq!(escape("/a b/c'd&e.zip"), "/a\\ b/c\\'d\\&e.zip");
        assert_eq!(escape("/Roms/ñandú-1.sfc"), "/Roms/ñandú-1.sfc");
    }
}
