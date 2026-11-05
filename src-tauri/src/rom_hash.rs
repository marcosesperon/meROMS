//! Huellas (CRC32) de las ROMs, para reconocerlas por su contenido y no solo por el nombre.
//!
//! - `.zip` / `.7z`: el CRC de cada fichero viene en el índice del comprimido, así que se lee
//!   sin descomprimir nada (instantáneo).
//! - Ficheros sueltos: se leen enteros (por trozos, sin cargarlos en memoria) y el resultado
//!   se guarda en una caché por ruta + tamaño + fecha, así que solo cuesta la primera vez.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

/// Un fichero dentro de un comprimido
#[derive(Debug, Clone, PartialEq)]
pub struct ArchiveEntry {
    pub name: String,
    pub size: u64,
    /// CRC32 en hexadecimal, 8 cifras en minúsculas
    pub crc: String,
}

pub fn crc_hex(crc: u32) -> String {
    format!("{:08x}", crc)
}

fn extension_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

pub fn is_archive(path: &Path) -> bool {
    matches!(extension_of(path).as_str(), "zip" | "7z")
}

/// Los ficheros de un `.zip` o `.7z` (sin carpetas), leídos de su índice.
/// None si no es un comprimido o no se puede leer.
pub fn archive_entries(path: &Path) -> Option<Vec<ArchiveEntry>> {
    match extension_of(path).as_str() {
        "zip" => {
            let mut zip = zip::ZipArchive::new(File::open(path).ok()?).ok()?;
            let mut entries = Vec::new();
            for i in 0..zip.len() {
                let file = zip.by_index_raw(i).ok()?;
                if file.is_dir() {
                    continue;
                }
                entries.push(ArchiveEntry { name: file.name().to_string(), size: file.size(), crc: crc_hex(file.crc32()) });
            }
            Some(entries)
        }
        "7z" => {
            let archive = sevenz_rust2::Archive::open(path).ok()?;
            Some(
                archive
                    .files
                    .iter()
                    .filter(|f| !f.is_directory && f.has_stream)
                    .map(|f| ArchiveEntry { name: f.name.clone(), size: f.size, crc: crc_hex(f.crc as u32) })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// El único fichero de un comprimido (cartuchos, No-Intro). Con varios (arcade) o ninguno, None.
pub fn single_archive_entry(path: &Path) -> Option<ArchiveEntry> {
    let mut entries = archive_entries(path)?;
    (entries.len() == 1).then(|| entries.remove(0))
}

/// CRC32 de un fichero suelto, leyéndolo por trozos
pub fn file_crc(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let mut hasher = crc32fast::Hasher::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(crc_hex(hasher.finalize()))
}

// ---------- Caché ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    size: u64,
    /// Fecha de modificación (segundos Unix)
    modified: u64,
    /// None = no tiene una huella útil (comprimido de varios ficheros)
    crc: Option<String>,
}

/// Huellas ya calculadas, por ruta absoluta. Si el fichero cambia (tamaño o fecha), no vale.
#[derive(Debug, Default)]
pub struct CrcCache {
    file: Option<PathBuf>,
    entries: HashMap<String, CacheEntry>,
    dirty: bool,
}

fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some((meta.len(), modified))
}

impl CrcCache {
    /// Lee la caché de disco (si no existe o está rota, empieza vacía)
    pub fn load(file: &Path) -> Self {
        let entries = fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        CrcCache { file: Some(file.to_path_buf()), entries, dirty: false }
    }

    /// La huella guardada de una ROM, si sigue valiendo (no calcula nada).
    /// `Some(None)` = ya se miró y no tiene huella útil.
    pub fn cached(&self, rom: &Path) -> Option<Option<String>> {
        let entry = self.entries.get(&rom.to_string_lossy().to_string())?;
        let (size, modified) = stamp(rom)?;
        (entry.size == size && entry.modified == modified).then(|| entry.crc.clone())
    }

    /// La huella de una ROM: de la caché o, si no está, calculándola (y guardándola)
    pub fn crc(&mut self, rom: &Path) -> Option<String> {
        if let Some(crc) = self.cached(rom) {
            return crc;
        }
        let (size, modified) = stamp(rom)?;
        let crc = if is_archive(rom) {
            single_archive_entry(rom).map(|e| e.crc)
        } else {
            file_crc(rom).ok()
        };
        self.entries.insert(rom.to_string_lossy().to_string(), CacheEntry { size, modified, crc: crc.clone() });
        self.dirty = true;
        crc
    }

    /// Escribe la caché si cambió
    pub fn save(&mut self) -> Result<(), String> {
        let Some(file) = &self.file else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        let json = serde_json::to_string(&self.entries).map_err(|e| e.to_string())?;
        fs::write(file, json).map_err(|e| format!("Failed to save {}: {}", file.display(), e))?;
        self.dirty = false;
        Ok(())
    }
}

/// Dónde vive la caché: carpeta de datos de la app
pub fn cache_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    Ok(dir.join("crc_cache.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Zip sin comprimir (método «stored») hecho a mano: cabecera local + directorio central
    fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut out: Vec<u8> = Vec::new();
        let mut central: Vec<u8> = Vec::new();
        for (name, data) in files {
            let crc = crc32fast::hash(data);
            let offset = out.len() as u32;
            out.extend_from_slice(&0x04034b50u32.to_le_bytes());
            out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);

            central.extend_from_slice(&0x02014b50u32.to_le_bytes());
            central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let central_offset = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x06054b50u32.to_le_bytes());
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&central_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        File::create(path).unwrap().write_all(&out).unwrap();
    }

    #[test]
    fn reads_the_crc_from_the_zip_index() {
        let tmp = tempfile::tempdir().unwrap();
        let one = tmp.path().join("juego.zip");
        write_zip(&one, &[("Juego (USA).sfc", b"contenido de la rom")]);
        let entry = single_archive_entry(&one).unwrap();
        assert_eq!(entry.name, "Juego (USA).sfc");
        assert_eq!(entry.size, 19);
        assert_eq!(entry.crc, crc_hex(crc32fast::hash(b"contenido de la rom")));

        // Arcade: varios ficheros → sin huella única
        let many = tmp.path().join("sf2.zip");
        write_zip(&many, &[("a.bin", b"uno"), ("b.bin", b"dos")]);
        assert_eq!(archive_entries(&many).unwrap().len(), 2);
        assert!(single_archive_entry(&many).is_none());
    }

    #[test]
    fn loose_files_are_hashed_once_and_again_when_they_change() {
        let tmp = tempfile::tempdir().unwrap();
        let rom = tmp.path().join("juego.sfc");
        fs::write(&rom, b"version 1").unwrap();
        let cache_path = tmp.path().join("crc_cache.json");

        let mut cache = CrcCache::load(&cache_path);
        assert_eq!(cache.cached(&rom), None, "nothing cached yet");
        let first = cache.crc(&rom).unwrap();
        assert_eq!(first, crc_hex(crc32fast::hash(b"version 1")));
        cache.save().unwrap();

        // Otra sesión: sale de la caché
        let cache = CrcCache::load(&cache_path);
        assert_eq!(cache.cached(&rom), Some(Some(first.clone())));

        // El fichero cambia (otro tamaño): la caché ya no vale
        fs::write(&rom, b"version dos, mas larga").unwrap();
        let mut cache = CrcCache::load(&cache_path);
        assert_eq!(cache.cached(&rom), None);
        assert_eq!(cache.crc(&rom).unwrap(), crc_hex(crc32fast::hash(b"version dos, mas larga")));
    }
}
