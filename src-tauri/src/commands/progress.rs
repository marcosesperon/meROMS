//! Progreso y cancelación de los procesos largos (duplicados, media sin usar, renombrar…).
//!
//! El frontend da a cada proceso un id de tarea (`task`). Mientras trabaja, el comando va
//! llamando a `Progress::step` y eso emite `task-progress` con la fase, «N de M» y el fichero o
//! juego en curso; la ventana de la herramienta lo pinta. `cancel_task` marca la tarea y el
//! comando, entre un juego y el siguiente, mira `cancelled()`: un análisis devuelve el error
//! `CANCELLED` y un cambio se para guardando lo hecho.
//!
//! Sin id (llamadas desde otros comandos o desde los tests) no se emite nada y nunca se cancela.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Error que devuelve un análisis cancelado (el frontend lo reconoce y no enseña nada)
pub const CANCELLED: &str = "cancelled";

/// Como mucho, un aviso cada tanto (con miles de ficheros, uno por fichero saturaría la ventana)
const MIN_GAP: Duration = Duration::from_millis(50);

/// Tareas que el usuario ha pedido parar (se quitan cuando la tarea termina)
static CANCELLED_TASKS: Mutex<Option<HashSet<String>>> = Mutex::new(None);

#[derive(Clone, Serialize)]
struct TaskProgress<'a> {
    task: &'a str,
    /// Clave de la fase («reading», «hashing»…): el frontend la traduce
    phase: &'a str,
    current: usize,
    /// 0 = aún no se sabe cuántos hay (barra en movimiento)
    total: usize,
    /// Fichero o juego en curso
    item: &'a str,
}

pub struct Progress {
    /// Id de la tarea (sin él, nunca se cancela)
    task: Option<String>,
    /// Por dónde se avisa (sin app, como en los tests, no se avisa)
    app: Option<AppHandle>,
    last: Mutex<Option<Instant>>,
}

impl Progress {
    /// Progreso de la tarea `task` (sin id, como `none`)
    pub fn new(app: &AppHandle, task: Option<String>) -> Self {
        let app = task.as_ref().map(|_| app.clone());
        Progress { task, app, last: Mutex::new(None) }
    }

    /// Un progreso que no avisa ni se cancela (otros comandos, tests)
    pub fn none() -> Self {
        Progress { task: None, app: None, last: Mutex::new(None) }
    }

    /// Para los tests: una tarea que se puede cancelar, sin avisos
    #[cfg(test)]
    pub fn for_task(task: &str) -> Self {
        Progress { task: Some(task.to_string()), app: None, last: Mutex::new(None) }
    }

    /// Avisa de por dónde va. El primero y el último siempre salen; los demás, como mucho
    /// cada `MIN_GAP`.
    pub fn step(&self, phase: &str, current: usize, total: usize, item: &str) {
        let (Some(app), Some(task)) = (&self.app, &self.task) else { return };
        let now = Instant::now();
        {
            let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
            let edge = current <= 1 || (total > 0 && current >= total);
            if !edge && last.is_some_and(|l| now.duration_since(l) < MIN_GAP) {
                return;
            }
            *last = Some(now);
        }
        let _ = app.emit("task-progress", TaskProgress { task, phase, current, total, item });
    }

    /// Una fase sin recuento (p. ej. «leyendo la gamelist»): barra en movimiento
    pub fn phase(&self, phase: &str) {
        self.step(phase, 0, 0, "");
    }

    /// ¿Ha pedido el usuario pararla?
    pub fn cancelled(&self) -> bool {
        self.task.as_deref().is_some_and(is_cancelled)
    }

    /// Para los análisis: `Err(CANCELLED)` si hay que parar (se usa con `?`)
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled() {
            Err(CANCELLED.to_string())
        } else {
            Ok(())
        }
    }
}

impl Drop for Progress {
    // La tarea ha terminado: su marca de cancelada ya no hace falta
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            forget(task);
        }
    }
}

fn is_cancelled(task: &str) -> bool {
    let guard = CANCELLED_TASKS.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().is_some_and(|set| set.contains(task))
}

fn forget(task: &str) {
    let mut guard = CANCELLED_TASKS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(set) = guard.as_mut() {
        set.remove(task);
    }
}

/// Pide parar la tarea `task` (lo hará entre un juego y el siguiente)
#[tauri::command]
pub fn cancel_task(task: String) {
    let mut guard = CANCELLED_TASKS.lock().unwrap_or_else(|e| e.into_inner());
    guard.get_or_insert_with(HashSet::new).insert(task);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_tarea_nunca_se_cancela() {
        cancel_task(String::new());
        let p = Progress::none();
        assert!(!p.cancelled());
        assert!(p.check().is_ok());
    }

    #[test]
    fn una_tarea_cancelada_se_para_y_se_olvida_al_terminar() {
        let p = Progress::for_task("t-drop");
        assert!(!p.cancelled());
        cancel_task("t-drop".into());
        assert!(p.cancelled());
        assert_eq!(p.check(), Err(CANCELLED.to_string()));
        drop(p);
        assert!(!is_cancelled("t-drop"));
    }

    #[test]
    fn cancelar_y_olvidar_una_tarea() {
        assert!(!is_cancelled("t-prueba"));
        cancel_task("t-prueba".into());
        assert!(is_cancelled("t-prueba"));
        assert!(!is_cancelled("t-otra"));
        forget("t-prueba");
        assert!(!is_cancelled("t-prueba"));
    }
}
