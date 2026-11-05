import { useAppStore } from "../store/useAppStore";
import type { Game } from "../types";

// Lo que se espera a un posible segundo clic antes de abrir la ficha a pantalla completa
// (si llega, es un doble clic y se abre el editor en su lugar)
const DOUBLE_CLICK_MS = 250;
let pendingFullView: number | undefined;

/**
 * Clic en un juego del listado (tarjeta o fila). En selección múltiple lo marca/desmarca; si no,
 * lo selecciona y, con el ajuste «Al pulsar un juego» = ficha a pantalla completa, la abre.
 */
export function clickGame(game: Game) {
  const { multiSelect, toggleSelected, selectGame, settings } = useAppStore.getState();
  if (multiSelect) {
    toggleSelected(game.path);
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
