import { translate } from "../i18n";
import { undoLast } from "../commands";
import { useAppStore } from "../store/useAppStore";
import { bumpMediaVersion } from "./images";
import { showMessage } from "./dialogs";
import { askUndo } from "../components/common/ResultDialog";

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
 * Va en una ventana propia (`ResultDialog`): «Aceptar» es el botón principal y cerrar el aviso
 * (Intro, Esc, la X) nunca deshace. El diálogo nativo de Windows dejaba «Deshacer» por defecto.
 */
export async function reportWithUndo(message: string) {
  const { refreshUndo } = useAppStore.getState();
  await refreshUndo();
  if (!useAppStore.getState().undoInfo) {
    await showMessage(message);
    return;
  }
  if (await askUndo(message)) await runUndo();
}
