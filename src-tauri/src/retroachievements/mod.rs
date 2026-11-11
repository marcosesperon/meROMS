//! RetroAchievements sin conexión: a qué consola de RetroAchievements corresponde cada carpeta
//! de sistema y cómo calcula RetroAchievements la huella («hash») de una ROM de esa consola, para
//! saber si es una versión reconocida (los logros solo funcionan con ROMs concretas).
//! Las reglas son las de su documentación («Game Identification»); los discos aún no.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use md5::{Digest, Md5};

/// Cómo se calcula la huella en una consola
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashMethod {
    /// MD5 del fichero entero
    Plain,
    /// NES / Famicom: sin la cabecera iNES de 16 bytes, si la tiene
    Nes,
    /// Famicom Disk System: sin la cabecera de 16 bytes, si la tiene
    Fds,
    /// SNES: sin la cabecera de copiadora de 512 bytes (tamaño = múltiplo de 8 KB + 512)
    Snes,
    /// PC Engine: sin la cabecera de 512 bytes (tamaño = múltiplo de 128 KB + 512)
    PcEngine,
    /// Atari Lynx: sin la cabecera de 64 bytes, si la tiene
    Lynx,
    /// Atari 7800: sin la cabecera de 128 bytes, si la tiene
    Atari7800,
    /// Nintendo 64: siempre en orden big-endian (.z64); .v64 y .n64 se reordenan antes
    N64,
    /// Arcade: MD5 del nombre del fichero sin extensión
    Arcade,
    /// Arduboy: el .hex con saltos de línea de Unix
    Arduboy,
    /// Discos, Nintendo DS…: todavía no se calcula
    Unsupported,
}

/// Una consola de RetroAchievements
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaConsole {
    pub id: u32,
    pub method: HashMethod,
}

use HashMethod::*;

/// Carpeta de sistema (nombres de ES-DE) → consola de RetroAchievements
const CONSOLES: &[(&[&str], u32, HashMethod)] = &[
    (&["megadrive", "genesis", "megadrivejp"], 1, Plain),
    (&["n64", "n64dd"], 2, N64),
    (&["snes", "sfc", "snesna", "satellaview", "sufami"], 3, Snes),
    (&["gb"], 4, Plain),
    (&["gba"], 5, Plain),
    (&["gbc"], 6, Plain),
    (&["nes", "famicom"], 7, Nes),
    (&["pcengine", "tg16", "supergrafx"], 8, PcEngine),
    (&["segacd", "megacd", "megacdjp"], 9, Unsupported),
    (&["sega32x", "sega32xjp", "sega32xna"], 10, Plain),
    (&["mastersystem", "sms"], 11, Plain),
    (&["psx"], 12, Unsupported),
    (&["atarilynx", "lynx"], 13, Lynx),
    (&["ngp", "ngpc"], 14, Plain),
    (&["gamegear"], 15, Plain),
    (&["gc", "gamecube"], 16, Unsupported),
    (&["atarijaguar", "jaguar"], 17, Plain),
    (&["nds"], 18, Unsupported),
    (&["ps2"], 21, Unsupported),
    (&["odyssey2", "videopac"], 23, Plain),
    (&["pokemini"], 24, Plain),
    (&["atari2600"], 25, Plain),
    (&["arcade", "mame", "mame-advmame", "mame-mame4all", "fbneo", "fba", "cps", "cps1", "cps2", "cps3", "neogeo", "naomi", "atomiswave"], 27, Arcade),
    (&["virtualboy"], 28, Plain),
    (&["msx", "msx1", "msx2", "msxturbor"], 29, Plain),
    (&["sg-1000", "sg1000"], 33, Plain),
    (&["amstradcpc"], 37, Plain),
    (&["apple2"], 38, Plain),
    (&["saturn", "saturnjp"], 39, Unsupported),
    (&["dreamcast"], 40, Unsupported),
    (&["psp"], 41, Unsupported),
    (&["3do"], 43, Unsupported),
    (&["colecovision", "coleco"], 44, Plain),
    (&["intellivision"], 45, Plain),
    (&["vectrex"], 46, Plain),
    (&["pc88"], 47, Plain),
    (&["pcfx"], 49, Unsupported),
    (&["atari5200"], 50, Plain),
    (&["atari7800"], 51, Atari7800),
    (&["wonderswan", "wonderswancolor"], 53, Plain),
    (&["neogeocd", "neogeocdjp"], 56, Unsupported),
    (&["channelf"], 57, Plain),
    (&["supervision"], 63, Plain),
    (&["megaduck"], 69, Plain),
    (&["arduboy"], 71, Arduboy),
    (&["wasm4"], 72, Plain),
    (&["pcenginecd", "tg-cd"], 76, Unsupported),
    (&["atarijaguarcd"], 77, Unsupported),
    (&["fds"], 81, Fds),
];

/// La consola de RetroAchievements de una carpeta de sistema (sin distinguir mayúsculas)
pub fn console_for(system: &str) -> Option<RaConsole> {
    let name = system.to_lowercase();
    CONSOLES
        .iter()
        .find(|(names, _, _)| names.contains(&name.as_str()))
        .map(|&(_, id, method)| RaConsole { id, method })
}

/// Lo más grande que se lee para calcular una huella (los cartuchos son pequeños; un N64, 64 MB)
const MAX_ROM: u64 = 256 * 1024 * 1024;

/// La huella de RetroAchievements de una ROM (MD5 en hexadecimal). `None` = la consola aún no
/// se puede comprobar, o no se ha podido leer. Un .zip/.7z con un solo fichero se mira por dentro.
pub fn rom_hash(rom: &Path, method: HashMethod) -> Option<String> {
    match method {
        Unsupported => None,
        // El nombre, no el contenido: sirve aunque el fichero no se pueda leer
        Arcade => rom.file_stem().map(|s| md5_hex(s.to_string_lossy().as_bytes())),
        _ => hash_bytes(&rom_bytes(rom)?, method),
    }
}

/// La huella de unos datos ya leídos, según la consola
pub fn hash_bytes(data: &[u8], method: HashMethod) -> Option<String> {
    let skip = |n: usize| md5_hex(data.get(n..).unwrap_or_default());
    Some(match method {
        Plain => md5_hex(data),
        Nes if data.starts_with(b"NES\x1a") => skip(16),
        Fds if data.starts_with(b"FDS\x1a") => skip(16),
        Snes if data.len() % 8192 == 512 => skip(512),
        PcEngine if data.len() % 131072 == 512 => skip(512),
        Lynx if data.starts_with(b"LYNX\0") => skip(64),
        Atari7800 if data.starts_with(b"\x01ATARI7800") => skip(128),
        Nes | Fds | Snes | PcEngine | Lynx | Atari7800 => md5_hex(data),
        N64 => md5_hex(&n64_big_endian(data)),
        Arduboy => md5_hex(&String::from_utf8_lossy(data).replace("\r\n", "\n").into_bytes()),
        Arcade | Unsupported => return None,
    })
}

/// N64 en orden big-endian (.z64): .v64 tiene los bytes cambiados de dos en dos y .n64 va al revés
/// de cuatro en cuatro. Se sabe por los primeros 4 bytes.
fn n64_big_endian(data: &[u8]) -> Vec<u8> {
    match data.get(..4) {
        Some([0x37, 0x80, 0x40, 0x12]) => data.chunks(2).flat_map(|c| c.iter().rev().copied()).collect(),
        Some([0x40, 0x12, 0x37, 0x80]) => data.chunks(4).flat_map(|c| c.iter().rev().copied()).collect(),
        _ => data.to_vec(),
    }
}

fn md5_hex(data: &[u8]) -> String {
    format!("{:x}", Md5::digest(data))
}

/// El contenido de la ROM: el fichero, o el único fichero de dentro de un .zip/.7z
fn rom_bytes(rom: &Path) -> Option<Vec<u8>> {
    if !rom.is_file() || std::fs::metadata(rom).ok()?.len() > MAX_ROM * 2 {
        return None;
    }
    if let Some(entry) = crate::rom_hash::single_archive_entry(rom) {
        return archive_entry_bytes(rom, &entry.name);
    }
    let mut data = Vec::new();
    File::open(rom).ok()?.take(MAX_ROM).read_to_end(&mut data).ok()?;
    Some(data)
}

fn archive_entry_bytes(archive: &Path, name: &str) -> Option<Vec<u8>> {
    let ext = archive.extension()?.to_string_lossy().to_lowercase();
    let mut data = Vec::new();
    if ext == "zip" {
        let mut zip = zip::ZipArchive::new(File::open(archive).ok()?).ok()?;
        zip.by_name(name).ok()?.take(MAX_ROM).read_to_end(&mut data).ok()?;
        return Some(data);
    }
    if ext == "7z" {
        let mut reader = sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty()).ok()?;
        let mut found = false;
        let _ = reader.for_each_entries(|entry, content| {
            if entry.name() == name {
                found = content.take(MAX_ROM).read_to_end(&mut data).is_ok();
                return Ok(false);
            }
            std::io::copy(content, &mut std::io::sink())?;
            Ok(true)
        });
        return found.then_some(data);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn consolas_por_carpeta() {
        assert_eq!(console_for("SNES"), Some(RaConsole { id: 3, method: Snes }));
        assert_eq!(console_for("genesis").unwrap().id, 1);
        assert_eq!(console_for("cps1").unwrap().method, Arcade);
        assert_eq!(console_for("psx").unwrap().method, Unsupported);
        assert_eq!(console_for("odyssey3"), None);
    }

    #[test]
    fn cabeceras_que_se_quitan() {
        let body = vec![7u8; 8192];
        let plain = md5_hex(&body);
        // NES con cabecera iNES: la huella es la del contenido sin ella
        let mut nes = b"NES\x1a".to_vec();
        nes.extend([0u8; 12]);
        nes.extend(&body);
        assert_eq!(hash_bytes(&nes, Nes).unwrap(), plain);
        assert_eq!(hash_bytes(&body, Nes).unwrap(), plain);
        // SNES: 512 bytes de más (copiadora) se quitan; sin ellos, igual
        let mut snes = vec![0u8; 512];
        snes.extend(&body);
        assert_eq!(hash_bytes(&snes, Snes).unwrap(), plain);
        assert_eq!(hash_bytes(&body, Snes).unwrap(), plain);
        // Lynx y Atari 7800
        let mut lynx = b"LYNX\0".to_vec();
        lynx.resize(64, 0);
        lynx.extend(&body);
        assert_eq!(hash_bytes(&lynx, Lynx).unwrap(), plain);
        let mut a78 = b"\x01ATARI7800".to_vec();
        a78.resize(128, 0);
        a78.extend(&body);
        assert_eq!(hash_bytes(&a78, Atari7800).unwrap(), plain);
        // PC Engine: múltiplo de 128 KB + 512
        let pce_body = vec![3u8; 131072];
        let mut pce = vec![0u8; 512];
        pce.extend(&pce_body);
        assert_eq!(hash_bytes(&pce, PcEngine).unwrap(), md5_hex(&pce_body));
    }

    #[test]
    fn n64_en_cualquier_orden() {
        let z64 = [0x80u8, 0x37, 0x12, 0x40, 1, 2, 3, 4];
        let v64 = [0x37u8, 0x80, 0x40, 0x12, 2, 1, 4, 3];
        let n64 = [0x40u8, 0x12, 0x37, 0x80, 4, 3, 2, 1];
        let expected = md5_hex(&z64);
        assert_eq!(hash_bytes(&z64, N64).unwrap(), expected);
        assert_eq!(hash_bytes(&v64, N64).unwrap(), expected);
        assert_eq!(hash_bytes(&n64, N64).unwrap(), expected);
    }

    #[test]
    fn arcade_por_el_nombre_y_arduboy_con_saltos_unix() {
        assert_eq!(rom_hash(Path::new("/roms/arcade/galaga.zip"), Arcade).unwrap(), md5_hex(b"galaga"));
        assert_eq!(hash_bytes(b":10\r\n:00\r\n", Arduboy).unwrap(), md5_hex(b":10\n:00\n"));
        assert_eq!(hash_bytes(b"x", Unsupported), None);
    }

    #[test]
    fn dentro_de_un_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("game.zip");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        zip.start_file::<_, ()>("game.gb", zip::write::FileOptions::default()).unwrap();
        zip.write_all(b"contenido de la rom").unwrap();
        zip.finish().unwrap();
        assert_eq!(rom_hash(&path, Plain).unwrap(), md5_hex(b"contenido de la rom"));
        // Un fichero suelto, tal cual
        let loose = tmp.path().join("game.gg");
        std::fs::write(&loose, b"abc").unwrap();
        assert_eq!(rom_hash(&loose, Plain).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
    }
}
