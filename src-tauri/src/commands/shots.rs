//! Modo capturas (solo desarrollo): con `MEROMS_SHOTS=1 pnpm tauri dev`, el frontend
//! (`src/dev/shots.ts`) recorre unas escenas y, en cada una, pide aquí una foto de la ventana
//! para la web (`site/images/<nombre>.jpg`). En la app publicada (release) no hace nada.

use std::path::PathBuf;
use std::process::Command;

/// Ancho de las capturas de la web (las de siempre son de 1600 px)
const SHOT_WIDTH: &str = "1600";

/// ¿Se ha lanzado en modo capturas? Nunca en una compilación de release.
#[tauri::command]
pub fn shots_mode() -> bool {
    cfg!(debug_assertions) && std::env::var("MEROMS_SHOTS").is_ok_and(|v| v == "1")
}

/// Escenas a sacar (`MEROMS_SHOTS_ONLY=table,metadata`); vacío = todas
#[tauri::command]
pub fn shots_only() -> Vec<String> {
    if !shots_mode() {
        return Vec::new();
    }
    std::env::var("MEROMS_SHOTS_ONLY")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Ventana a tamaño fijo y centrada, para que todas las capturas salgan iguales
#[tauri::command]
pub fn shots_prepare(window: tauri::WebviewWindow) -> Result<(), String> {
    if !shots_mode() {
        return Err("Not in shots mode".into());
    }
    let (width, height) = shot_size(&window);
    window.set_size(tauri::LogicalSize::new(width, height)).map_err(|e| e.to_string())?;
    // Centrada a mano: `center()` justo después de cambiar el tamaño usa a veces el de antes y la
    // ventana se sale por un lado (y la foto sale cortada)
    match window.current_monitor() {
        Ok(Some(monitor)) => {
            let scale = monitor.scale_factor();
            let area = monitor.work_area();
            let (pos, size) = (area.position.to_logical::<f64>(scale), area.size.to_logical::<f64>(scale));
            let x = pos.x + ((size.width - width) / 2.0).max(0.0);
            let y = pos.y + ((size.height - height) / 2.0).max(0.0);
            window.set_position(tauri::LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
        }
        _ => window.center().map_err(|e| e.to_string())?,
    }
    // Siempre encima: la foto es de la zona de pantalla y otra ventana delante saldría en ella
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

/// 1600×1000 o, si la pantalla es más pequeña (un portátil), lo más grande con esa forma que quepa
/// (si no, la foto sale cortada: es de la zona de pantalla). La imagen final se lleva a 1600 de ancho.
fn shot_size(window: &tauri::WebviewWindow) -> (f64, f64) {
    const TITLE_BAR: f64 = 40.0;
    let Ok(Some(monitor)) = window.current_monitor() else { return (1600.0, 1000.0) };
    let area = monitor.work_area().size.to_logical::<f64>(monitor.scale_factor());
    let width = 1600.0_f64.min(area.width - 20.0).min((area.height - TITLE_BAR) * 1.6);
    (width.floor(), (width / 1.6).floor())
}

/// Carpeta de las imágenes de la web (el proyecto se conoce al compilar: solo desarrollo)
fn images_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("site").join("images")
}

/// Foto de la ventana (con su barra de título) → `site/images/<name>.jpg`, 1600 px de ancho.
/// Se fotografía la zona de pantalla que ocupa la ventana (`screencapture -R`), que está delante.
#[tauri::command]
pub async fn capture_window(window: tauri::WebviewWindow, name: String) -> Result<String, String> {
    if !shots_mode() {
        return Err("Not in shots mode".into());
    }
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("Bad shot name: {}", name));
    }
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let size = window.outer_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let region = format!("{},{},{},{}", pos.x.round(), pos.y.round(), size.width.round(), size.height.round());

    let png = std::env::temp_dir().join(format!("meroms-shot-{}.png", name));
    let dest = images_dir().join(format!("{}.jpg", name));
    tauri::async_runtime::spawn_blocking(move || {
        run(Command::new("screencapture").args(["-x", "-R", &region]).arg(&png))?;
        run(Command::new("sips")
            .args(["-s", "format", "jpeg", "-s", "formatOptions", "85", "--resampleWidth", SHOT_WIDTH])
            .arg(&png)
            .arg("--out")
            .arg(&dest))?;
        let _ = std::fs::remove_file(&png);
        Ok(dest.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

/// Fin del recorrido: cerrar la app
#[tauri::command]
pub fn shots_done(app: tauri::AppHandle) {
    if shots_mode() {
        app.exit(0);
    }
}
