import { useEffect, useRef, type RefObject } from "react";
import { useAppStore } from "../store/useAppStore";
import type { Game } from "../types";

interface ListKeyboardOptions {
  /** Games in the order they are shown */
  items: Game[];
  /** The scrolling container (each game element inside carries data-game-path) */
  containerRef: RefObject<HTMLElement | null>;
  /** Games per row (grid); 1 for the table */
  getColumns: () => number;
}

/** Element of a game inside the list (cards and table rows carry data-game-path) */
function elementFor(container: HTMLElement, path: string): HTMLElement | null {
  return container.querySelector<HTMLElement>(`[data-game-path="${CSS.escape(path)}"]`);
}

/**
 * Moverse por la cuadrícula o la tabla con el teclado: flechas, Inicio/Fin, RePág/AvPág
 * e Intro (abre el editor). No hace nada mientras se escribe en un campo ni con una
 * ventana abierta encima.
 */
export function useListKeyboard({ items, containerRef, getColumns }: ListKeyboardOptions) {
  // Lo último que se ha pintado, sin volver a registrar el listener en cada cambio
  const latest = useRef({ items, getColumns });
  latest.current = { items, getColumns };

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      // Escribiendo en un campo: las teclas son para él
      const target = e.target as HTMLElement | null;
      if (target?.closest("input, textarea, select, [contenteditable='true']")) return;
      // Una ventana abierta (todas las ventanas de la app son capas «fixed inset-0»)
      if (document.querySelector(".fixed.inset-0")) return;

      const container = containerRef.current;
      const { items, getColumns } = latest.current;
      if (!container || items.length === 0) return;

      const { selectedGame, selectGame, openEditor } = useAppStore.getState();
      const current = selectedGame ? items.findIndex((g) => g.path === selectedGame.path) : -1;

      if (e.key === "Enter") {
        // Con un botón enfocado, Intro es para pulsar ese botón
        if (target?.closest("button, a")) return;
        if (current >= 0) {
          e.preventDefault();
          openEditor();
        }
        return;
      }

      const columns = Math.max(1, getColumns());
      // Cuántos juegos caben en una pantalla (para RePág/AvPág)
      const pageSize = () => {
        const el = current >= 0 ? elementFor(container, items[current].path) : container.querySelector<HTMLElement>("[data-game-path]");
        const rowHeight = el?.offsetHeight || 1;
        return Math.max(1, Math.floor(container.clientHeight / rowHeight)) * columns;
      };

      let next: number;
      switch (e.key) {
        case "ArrowRight":
          if (columns === 1) return;
          next = current + 1;
          break;
        case "ArrowLeft":
          if (columns === 1) return;
          next = current - 1;
          break;
        case "ArrowDown":
          next = current < 0 ? 0 : current + columns;
          break;
        case "ArrowUp":
          next = current < 0 ? 0 : current - columns;
          break;
        case "Home":
          next = 0;
          break;
        case "End":
          next = items.length - 1;
          break;
        case "PageDown":
          next = current + pageSize();
          break;
        case "PageUp":
          next = current - pageSize();
          break;
        default:
          return;
      }
      e.preventDefault();
      // Sin selección, cualquier tecla empieza por el primero
      if (current < 0) next = 0;
      next = Math.min(items.length - 1, Math.max(0, next));
      if (next === current) return;

      const game = items[next];
      selectGame(game);
      // Que se vea, sin mover la lista si ya está a la vista
      elementFor(container, game.path)?.scrollIntoView({ block: "nearest" });
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [containerRef]);
}
