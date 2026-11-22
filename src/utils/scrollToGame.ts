/**
 * Llevar la lista hasta un juego. La cuadrícula y la tabla solo pintan las filas que se ven
 * (`hooks/useVirtualRows`), así que la tarjeta de un juego lejano no existe y no se puede hacer
 * `scrollIntoView` sobre ella: la lista que está a la vista se apunta aquí con una función que
 * sabe en qué fila va cada juego. Sin lista apuntada (el carrusel sigue solo a la selección),
 * se busca el elemento en la página como antes.
 */

export type ScrollBlock = "nearest" | "center" | "start";

/** Devuelve false si el juego no está en esa lista */
type GameScroller = (path: string, block: ScrollBlock) => boolean;

let scroller: GameScroller | null = null;

/** La lista a la vista se apunta al montarse; devuelve la función para borrarse */
export function registerGameScroller(fn: GameScroller): () => void {
  scroller = fn;
  return () => {
    if (scroller === fn) scroller = null;
  };
}

export function scrollToGame(path: string, block: ScrollBlock = "nearest") {
  if (scroller?.(path, block)) return;
  document.querySelector(`[data-game-path="${CSS.escape(path)}"]`)?.scrollIntoView({ block });
}
