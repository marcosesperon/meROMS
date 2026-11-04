import { ask, message } from "@tauri-apps/plugin-dialog";

// Avisos de la app. No usamos alert()/confirm() del navegador: dentro de la ventana de
// Tauri en macOS se pueden quedar colgados y no hay forma de cerrarlos. Estos son los
// diálogos nativos del plugin (los abre Rust), y devuelven promesa: hay que esperarlos.

const TITLE = "meROMS";

/** Pregunta Sí/No. Devuelve true si el usuario acepta. */
export function askConfirm(text: string): Promise<boolean> {
  return ask(text, { title: TITLE, kind: "warning" });
}

/** Aviso con un solo botón (informativo o de error). */
export async function showMessage(text: string, kind: "info" | "error" = "info"): Promise<void> {
  await message(text, { title: TITLE, kind });
}
