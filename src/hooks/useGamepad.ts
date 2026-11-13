import { useEffect } from "react";
import { useAppStore } from "../store/useAppStore";
import { openRandom, playSelected, switchSystem, toggleFavorite, toggleOnlyFavorites } from "../utils/padActions";

/*
 * Mando de juego (API Gamepad del navegador, distribución «standard»: la de los mandos de Xbox,
 * PlayStation, 8BitDo… en macOS y Windows). Cada botón se convierte en la tecla equivalente, así
 * el carrusel, la ficha completa y el modo sofá responden igual con mando que con teclado. Fuera
 * del modo sofá los botones de acción (X, Y, L/R, Select, R3) no se simulan como teclas (RePág o
 * F significan otra cosa en la lista), sino que hacen su acción directamente (`PAD_ACTIONS`).
 */

/** Botón (índice de la distribución «standard») → tecla que se simula */
const BUTTON_KEYS: Record<number, string> = {
  0: "Enter", // A: abrir la ficha
  1: "Escape", // B: volver / salir
  2: "x", // X: jugar (modo sofá)
  3: "f", // Y: favorito sí/no
  4: "PageUp", // L: sistema anterior
  5: "PageDown", // R: sistema siguiente
  6: "[", // LT: letra anterior
  7: "]", // RT: letra siguiente
  8: "s", // Select: todos / solo favoritos
  11: "r", // stick derecho pulsado (R3): «¿A qué juego?»
  12: "ArrowUp", // cruceta ↑
  13: "ArrowDown", // cruceta ↓
  14: "ArrowLeft", // cruceta ←
  15: "ArrowRight", // cruceta →
};
const START = 9;
/** Las teclas de moverse (se repiten al mantenerlas) */
const MOVES = new Set(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"]);
/** Fuera del modo sofá: la acción de cada tecla de botón de acción (las demás se simulan) */
const PAD_ACTIONS: Record<string, () => void> = {
  x: playSelected,
  f: toggleFavorite,
  PageUp: () => switchSystem(-1),
  PageDown: () => switchSystem(1),
  s: toggleOnlyFavorites,
  r: openRandom,
};
/** Las direcciones se repiten al mantenerlas: primera espera y luego cada cuánto */
const REPEAT_DELAY = 400;
const REPEAT_EVERY = 120;
/** Lo que hay que inclinar el stick para que cuente */
const STICK = 0.5;

/** Simula la tecla (en el body, para que la vean los que escuchan en window y miran el destino) */
export function pressKey(key: string) {
  document.body.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
}

/** Avisa de cualquier uso del mando (el modo demostración lo usa para saber que hay alguien) */
export const GAMEPAD_ACTIVITY = "meroms-gamepad-activity";

/**
 * Lee los mandos mientras haya alguno conectado (fotograma a fotograma; sin mando, nada). Start
 * entra o sale del modo sofá; el resto de botones, dentro y fuera de él.
 */
export function useGamepad() {
  useEffect(() => {
    let frame = 0;
    // Lo pulsado en la lectura anterior y cuándo toca repetir cada dirección
    const pressed = new Set<string>();
    const repeatAt = new Map<string, number>();

    function read(now: number) {
      const pads = Array.from(navigator.getGamepads?.() ?? []).filter((p): p is Gamepad => !!p && p.connected);
      if (pads.length === 0) {
        frame = 0;
        return;
      }
      // Lo pulsado ahora en cualquier mando (botones y stick izquierdo como cruceta)
      const down = new Set<string>();
      for (const pad of pads) {
        pad.buttons.forEach((b, i) => {
          if (b.pressed) down.add(i === START ? "start" : BUTTON_KEYS[i] ?? "");
        });
        if ((pad.axes[0] ?? 0) <= -STICK) down.add("ArrowLeft");
        if ((pad.axes[0] ?? 0) >= STICK) down.add("ArrowRight");
        if ((pad.axes[1] ?? 0) <= -STICK) down.add("ArrowUp");
        if ((pad.axes[1] ?? 0) >= STICK) down.add("ArrowDown");
      }
      down.delete("");

      for (const key of down) {
        const first = !pressed.has(key);
        const isMove = MOVES.has(key);
        if (first || (isMove && now >= (repeatAt.get(key) ?? Infinity))) {
          repeatAt.set(key, now + (first ? REPEAT_DELAY : REPEAT_EVERY));
          window.dispatchEvent(new Event(GAMEPAD_ACTIVITY));
          act(key);
        }
      }
      pressed.clear();
      down.forEach((k) => pressed.add(k));
      frame = requestAnimationFrame(read);
    }

    function act(key: string) {
      const { couchMode, setCouchMode, closeFullView, runningGame } = useAppStore.getState();
      // Jugando, el mando es del emulador: meROMS no se mueve por detrás
      if (runningGame) return;
      if (key === "start") {
        if (couchMode) closeFullView();
        setCouchMode(!couchMode);
      } else if (couchMode || !(key in PAD_ACTIONS)) {
        pressKey(key);
      } else if (!document.querySelector(".fixed.inset-0:not([data-full-view])")) {
        // Fuera del modo sofá, sin ninguna ventana encima (la ficha completa no cuenta)
        PAD_ACTIONS[key]();
      }
    }

    // Se empieza a leer al conectar un mando (o al pulsar algo, si ya estaba conectado)
    const start = () => {
      if (!frame) frame = requestAnimationFrame(read);
    };
    window.addEventListener("gamepadconnected", start);
    start();
    return () => {
      window.removeEventListener("gamepadconnected", start);
      cancelAnimationFrame(frame);
    };
  }, []);
}
