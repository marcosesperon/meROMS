import { useAppStore } from "../store/useAppStore";
import type { Game } from "../types";

// Lo que se espera a un posible segundo clic antes de abrir la ficha a pantalla completa
// (si llega, es un doble clic y se abre el editor en su lugar)
const DOUBLE_CLICK_MS = 250;
let pendingFullView: number | undefined;
// Último juego marcado con un clic (el principio del tramo de Mayús+clic)
let lastMarked: string | null = null;

/** Teclas pulsadas con el clic: Cmd/Ctrl marca uno; Mayús marca el tramo */
export interface ClickKeys {
  toggle: boolean;
  range: boolean;
}

/** Las teclas de un evento de ratón (en macOS Cmd, en el resto Ctrl) */
export const clickKeys = (e: { metaKey: boolean; ctrlKey: boolean; shiftKey: boolean }): ClickKeys => ({
  toggle: e.metaKey || e.ctrlKey,
  range: e.shiftKey,
});

/** Marca todos los juegos entre el último marcado y este, en el orden en que se ven */
function markRange(path: string) {
  const { visibleOrder, selectedPaths, setSelectedPaths } = useAppStore.getState();
  const from = lastMarked ? visibleOrder.indexOf(lastMarked) : -1;
  const to = visibleOrder.indexOf(path);
  const next = new Set(selectedPaths);
  if (from < 0 || to < 0) next.add(path);
  else for (const p of visibleOrder.slice(Math.min(from, to), Math.max(from, to) + 1)) next.add(p);
  setSelectedPaths(next);
}

/**
 * Clic en un juego del listado (tarjeta o fila). En selección múltiple lo marca/desmarca (con
 * Mayús, el tramo desde el último marcado); si no, lo selecciona y, con el ajuste «Al pulsar un
 * juego» = ficha a pantalla completa, la abre. Cmd/Ctrl+clic o Mayús+clic sin la selección
 * múltiple la activan (con el juego que estaba elegido ya marcado) y marcan.
 */
export function clickGame(game: Game, keys?: ClickKeys) {
  const state = useAppStore.getState();
  const { selectGame, settings } = state;
  if (!state.multiSelect && (keys?.toggle || keys?.range)) {
    const current = state.selectedGame?.path;
    state.setMultiSelect(true);
    if (current) {
      useAppStore.getState().setSelectedPaths(new Set([current]));
      lastMarked = current;
    }
  }
  if (useAppStore.getState().multiSelect) {
    if (keys?.range) markRange(game.path);
    else useAppStore.getState().toggleSelected(game.path);
    lastMarked = game.path;
    return;
  }
  selectGame(game);
  if (settings.click_action === "full") {
    window.clearTimeout(pendingFullView);
    pendingFullView = window.setTimeout(() => useAppStore.getState().openFullView(), DOUBLE_CLICK_MS);
  }
}

/** Doble clic: abrir directamente el editor de ese juego (en selección múltiple, nada) */
export function doubleClickGame(game: Game) {
  window.clearTimeout(pendingFullView);
  const { multiSelect, selectGame, openEditor } = useAppStore.getState();
  if (multiSelect) return;
  selectGame(game);
  openEditor();
}
