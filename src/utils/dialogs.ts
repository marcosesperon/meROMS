import { ask, message } from "@tauri-apps/plugin-dialog";
import { translate } from "../i18n";

// Avisos de la app. No usamos alert()/confirm() del navegador: dentro de la ventana de
// Tauri en macOS se pueden quedar colgados y no hay forma de cerrarlos. Estos son los
// diálogos nativos del plugin (los abre Rust), y devuelven promesa: hay que esperarlos.
// Los botones se pasan traducidos; si no, salen en el idioma del sistema.

const TITLE = "meROMS";

/** Pregunta Sí/No. Devuelve true si el usuario acepta. */
/** Pregunta Sí / No. `labels`: otros textos para los botones (p. ej. «Renombrar» / «Ahora no») */
export function askConfirm(text: string, labels?: { ok: string; cancel: string; kind?: "info" | "warning" }): Promise<boolean> {
  return ask(text, {
    title: TITLE,
    kind: labels?.kind ?? "warning",
    okLabel: labels?.ok ?? translate("dialog.yes"),
    cancelLabel: labels?.cancel ?? translate("dialog.no"),
  });
}

/** Aviso con un solo botón (informativo o de error). */
export async function showMessage(text: string, kind: "info" | "error" = "info"): Promise<void> {
  await message(text, { title: TITLE, kind, okLabel: translate("dialog.ok") });
}
