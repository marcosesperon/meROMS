import { ask } from "@tauri-apps/plugin-dialog";
import { translate } from "../i18n";
import { undoLast } from "../commands";
import { useAppStore } from "../store/useAppStore";
import { bumpMediaVersion } from "./images";
import { showMessage } from "./dialogs";

// «Deshacer» la última operación que movió o renombró ficheros (Renombrar, Mover a…,
// Normalizar, Intercambiar tipos). Solo se recuerda la última; la guarda Rust.

/** Deshace la última operación, recarga y enseña el resumen */
export async function runUndo() {
  try {
    const result = await undoLast();
    bumpMediaVersion();
    const { selectedSystem, loadGames, loadSystems, selectGame, refreshUndo } = useAppStore.getState();
    selectGame(null);
    if (selectedSystem) await loadGames(selectedSystem.path);
    await loadSystems();
    await refreshUndo();
    const lines = [translate("undo.done").replace("{count}", String(result.restored))];
    if (result.skipped.length) {
      lines.push("", translate("undo.skipped").replace("{count}", String(result.skipped.length)), ...result.skipped.slice(0, 20));
    }
    await showMessage(lines.join("\n"), result.skipped.length ? "error" : "info");
  } catch (e) {
    await showMessage(String(e), "error");
  }
}

/**
 * Aviso del resultado de una operación deshacible: con «Deshacer» si hay algo que deshacer.
 * El botón principal es «Deshacer» y el otro «Aceptar»: cerrar el aviso nunca deshace.
 */
export async function reportWithUndo(message: string) {
  const { refreshUndo } = useAppStore.getState();
  await refreshUndo();
  if (!useAppStore.getState().undoInfo) {
    await showMessage(message);
    return;
  }
  const undo = await ask(message, {
    title: "meROMS",
    kind: "info",
    okLabel: translate("undo.undo"),
    cancelLabel: translate("dialog.ok"),
  });
  if (undo) await runUndo();
}
