use std::path::Path;

use crate::layout::esde_settings::{detect_esde_paths as detect, EsdeDetectedPaths};

/// Lee las rutas de ROMs y medios de una carpeta de ES-DE (su es_settings.xml)
/// para rellenar un perfil ES-DE desde la pantalla de ajustes.
#[tauri::command]
pub fn detect_esde_paths(esde_home: String) -> Result<EsdeDetectedPaths, String> {
    let home = Path::new(&esde_home);
    if !home.is_dir() {
        return Err(format!("Not a folder: {}", esde_home));
    }
    detect(home)
}
