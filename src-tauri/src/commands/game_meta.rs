//! Pestaña «Metadatos» de la ficha a pantalla completa: los ficheros del juego, sus huellas
//! (CRC32, MD5, SHA-1), qué dice el DAT del sistema y los campos del `<game>` tal cual están
//! en el gamelist.xml (también los que meROMS no maneja).

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use md5::{Digest, Md5};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;
use sha1::Sha1;

use crate::layout::strip_dot_slash;
use crate::rom_hash;

#[derive(Debug, Clone, Serialize)]
pub struct FileItem {
    /// Ruta relativa a la carpeta del sistema
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveItem {
    pub name: String,
    pub size: u64,
    pub crc: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameFiles {
    /// Nombre de la ROM (fichero o carpeta)
    pub name: String,
    pub exists: bool,
    pub is_dir: bool,
    /// Tamaño de todos sus ficheros juntos
    pub size: u64,
    /// La ROM y sus acompañantes (pistas de un .cue/.gdi, discos de un .m3u) o los de la carpeta
    pub files: Vec<FileItem>,
    /// Lo que hay dentro, si es un .zip o .7z (de su índice, sin descomprimir)
    pub archive: Option<Vec<ArchiveItem>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hashes {
    /// Qué se ha medido: el fichero, o el de dentro de un comprimido de un solo fichero
    pub target: String,
    pub inside_archive: bool,
    pub size: u64,
    pub crc: String,
    pub md5: String,
    pub sha1: String,
}

#[tauri::command]
pub fn game_files(system_path: String, game_path: String) -> Result<GameFiles, String> {
    let rom = Path::new(&system_path).join(strip_dot_slash(&game_path));
    let name = rom.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if !rom.exists() {
        return Ok(GameFiles { name, exists: false, is_dir: false, size: 0, files: Vec::new(), archive: None });
    }
    let files: Vec<FileItem> = super::export_device::rom_files(&rom)
        .into_iter()
        .map(|f| FileItem {
            size: fs::metadata(&f).map(|m| m.len()).unwrap_or(0),
            path: f.strip_prefix(&system_path).unwrap_or(&f).to_string_lossy().replace('\\', "/"),
        })
        .collect();
    let archive = rom_hash::archive_entries(&rom)
        .map(|entries| entries.into_iter().map(|e| ArchiveItem { name: e.name, size: e.size, crc: e.crc }).collect());
    Ok(GameFiles { name, exists: true, is_dir: rom.is_dir(), size: files.iter().map(|f| f.size).sum(), files, archive })
}

/// CRC32, MD5 y SHA-1 de una sola pasada
fn hash_reader(reader: &mut dyn Read) -> Result<(u64, String, String, String), String> {
    let mut crc = crc32fast::Hasher::new();
    let mut md5 = Md5::new();
    let mut sha1 = Sha1::new();
    let mut size = 0u64;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        crc.update(&buffer[..read]);
        md5.update(&buffer[..read]);
        sha1.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((size, rom_hash::crc_hex(crc.finalize()), format!("{:x}", md5.finalize()), format!("{:x}", sha1.finalize())))
}

/// Huellas del juego. Un .zip o .7z con un solo fichero se mide por dentro (lo que miran los
/// DAT); cualquier otro fichero, tal cual. Una ROM carpeta no tiene huella.
fn hashes_of(rom: &Path) -> Result<Option<Hashes>, String> {
    if !rom.is_file() {
        return Ok(None);
    }
    let file_name = rom.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if let Some(entry) = rom_hash::single_archive_entry(rom) {
        if let Some(h) = hash_archive_entry(rom, &entry.name)? {
            let (size, crc, md5, sha1) = h;
            return Ok(Some(Hashes { target: entry.name, inside_archive: true, size, crc, md5, sha1 }));
        }
    }
    let mut file = File::open(rom).map_err(|e| format!("{}: {}", rom.display(), e))?;
    let (size, crc, md5, sha1) = hash_reader(&mut file)?;
    Ok(Some(Hashes { target: file_name, inside_archive: false, size, crc, md5, sha1 }))
}

/// Huellas del fichero `name` dentro de un .zip o .7z (None si no se puede descomprimir)
fn hash_archive_entry(archive: &Path, name: &str) -> Result<Option<(u64, String, String, String)>, String> {
    let ext = archive.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "zip" {
        let Ok(file) = File::open(archive) else { return Ok(None) };
        let Ok(mut zip) = zip::ZipArchive::new(file) else { return Ok(None) };
        let Ok(mut entry) = zip.by_name(name) else { return Ok(None) };
        return hash_reader(&mut entry).map(Some).or(Ok(None));
    }
    if ext == "7z" {
        let Ok(mut reader) = sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty()) else { return Ok(None) };
        let mut out = None;
        let _ = reader.for_each_entries(|entry, data| {
            if entry.name() == name {
                out = hash_reader(data).ok();
                return Ok(false);
            }
            // Hay que leerlo entero para pasar al siguiente
            std::io::copy(data, &mut std::io::sink())?;
            Ok(true)
        });
        return Ok(out);
    }
    Ok(None)
}

#[tauri::command]
pub async fn game_hashes(system_path: String, game_path: String) -> Result<Option<Hashes>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let rom = Path::new(&system_path).join(strip_dot_slash(&game_path));
        hashes_of(&rom)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Qué dice el DAT del sistema de esta ROM (None si el sistema no tiene DAT)
#[tauri::command]
pub fn dat_game_info(app: tauri::AppHandle, system_path: String, game_path: String) -> Result<Option<super::dat::DatMatch>, String> {
    let (_, system) = super::system_context(&app, &system_path)?;
    let rom = system.rom_dir.join(strip_dot_slash(&game_path));
    super::dat::match_rom(&app, &system, &rom)
}

/// «Abrir con…» del visor de manuales: abre el PDF con el programa del sistema. Solo PDF que
/// existan (así la app no necesita permiso para abrir cualquier ruta)
#[tauri::command]
pub fn open_manual(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let file = Path::new(&path);
    let is_pdf = file.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
    if !is_pdf || !file.is_file() {
        return Err(format!("Not a PDF file: {}", path));
    }
    app.opener().open_path(path, None::<String>).map_err(|e| e.to_string())
}

/// Los campos del `<game>` tal cual están en el gamelist.xml: atributos (con «@» delante) y
/// cada elemento hijo con su texto, en su orden
#[tauri::command]
pub fn raw_game_fields(app: tauri::AppHandle, system_path: String, game_path: String) -> Result<Vec<(String, String)>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let path = layout.gamelist_path(&system);
    let Ok(xml) = fs::read_to_string(&path) else { return Ok(Vec::new()) };
    raw_fields(&xml, &game_path)
}

fn raw_fields(xml: &str, game_path: &str) -> Result<Vec<(String, String)>, String> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"game" => {
                let attrs: Vec<(String, String)> = e
                    .attributes()
                    .flatten()
                    .map(|a| (format!("@{}", String::from_utf8_lossy(a.key.as_ref())), a.unescape_value().unwrap_or_default().to_string()))
                    .collect();
                let fields = game_fields(&mut reader)?;
                let is_it = fields.iter().any(|(k, v)| k == "path" && v == game_path);
                if is_it {
                    return Ok(attrs.into_iter().chain(fields).collect());
                }
            }
            Ok(Event::Eof) => return Ok(Vec::new()),
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
    }
}

/// Los hijos de un `<game>` (hasta su cierre): nombre y texto de cada uno
fn game_fields(reader: &mut Reader<&[u8]>) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    let mut depth = 1;
    let mut current: Option<(String, String)> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                depth += 1;
                if depth == 2 {
                    current = Some((String::from_utf8_lossy(e.name().as_ref()).to_string(), String::new()));
                }
            }
            Ok(Event::Empty(e)) if depth == 1 => {
                out.push((String::from_utf8_lossy(e.name().as_ref()).to_string(), String::new()));
            }
            Ok(Event::Text(t)) => {
                if let Some((_, value)) = current.as_mut() {
                    value.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::CData(t)) => {
                if let Some((_, value)) = current.as_mut() {
                    value.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 1 {
                    out.extend(current.take());
                }
                if depth == 0 {
                    return Ok(out);
                }
            }
            Ok(Event::Eof) => return Ok(out),
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn huellas_conocidas() {
        let (size, crc, md5, sha1) = hash_reader(&mut "abc".as_bytes()).unwrap();
        assert_eq!(size, 3);
        assert_eq!(crc, "352441c2");
        assert_eq!(md5, "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn un_zip_de_un_fichero_se_mide_por_dentro() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("juego.zip");
        let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        zip.start_file("juego.sfc", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
        zip.write_all(b"abc").unwrap();
        zip.finish().unwrap();
        let h = hashes_of(&zip_path).unwrap().unwrap();
        assert!(h.inside_archive);
        assert_eq!(h.target, "juego.sfc");
        assert_eq!(h.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn campos_tal_cual() {
        let xml = r#"<?xml version="1.0"?>
<gameList>
  <game id="123" source="ScreenScraper.fr"><path>./a.zip</path><name>A</name></game>
  <game id="9"><path>./b.zip</path><name>B &amp; C</name><desc><![CDATA[Hola]]></desc><raro>x</raro><vacio/></game>
</gameList>"#;
        let f = raw_fields(xml, "./b.zip").unwrap();
        assert_eq!(
            f,
            vec![
                ("@id".to_string(), "9".to_string()),
                ("path".to_string(), "./b.zip".to_string()),
                ("name".to_string(), "B & C".to_string()),
                ("desc".to_string(), "Hola".to_string()),
                ("raro".to_string(), "x".to_string()),
                ("vacio".to_string(), String::new()),
            ]
        );
        assert!(raw_fields(xml, "./c.zip").unwrap().is_empty());
    }
}
