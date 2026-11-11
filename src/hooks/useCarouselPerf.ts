import { useEffect, useRef } from "react";
import { useAppStore } from "../store/useAppStore";

/** Lo que dura cada medida: más o menos lo que tarda el carrusel en moverse */
const SAMPLE_MS = 400;
/** Por debajo de estos fotogramas por segundo, la medida cuenta como lenta */
const SLOW_FPS = 30;
/** Movimientos que se miden y cuántos lentos hacen falta para apagar los efectos */
const SAMPLES = 6;
const SLOW_NEEDED = 4;

// Lo medido en esta sesión (fuera del componente: salir y volver al carrusel no empieza de cero)
let results: boolean[] = [];
let finished = false;

/** Vuelve a medir desde cero (al elegir otra vez «Automático» en Ajustes) */
export function resetCarouselPerf() {
  results = [];
  finished = false;
}

/** Apunta una medida y decide si ya se sabe cómo va el equipo */
function record(slow: boolean) {
  results.push(slow);
  const slowCount = results.filter(Boolean).length;
  if (slowCount >= SLOW_NEEDED) {
    finished = true;
    useAppStore.getState().patchSettings({ carousel_effects_slow: true });
  } else if (results.length - slowCount > SAMPLES - SLOW_NEEDED) {
    // Ya no pueden salir bastantes lentas: el equipo va bien
    finished = true;
  }
}

/**
 * Medición del modo «Automático» de los efectos del carrusel: cada vez que el carrusel se mueve
 * (cambia `trigger`) cuenta los fotogramas durante la animación. Si de los primeros movimientos
 * bastantes van a tirones, marca el equipo como lento y se guarda (los efectos se apagan solos y
 * no se vuelve a medir en otros arranques). Si van bien, deja de medir hasta el próximo arranque.
 */
export function useCarouselPerf(trigger: unknown, measuring: boolean) {
  // Fotograma pedido de la medida en marcha (0 = ninguna)
  const frame = useRef(0);
  // Solo se mide cuando `trigger` cambia de verdad: al entrar no (pintar no es moverse)
  const lastTrigger = useRef(trigger);

  useEffect(() => {
    if (lastTrigger.current === trigger) return;
    lastTrigger.current = trigger;
    // Moviéndose seguido, la medida que ya va sigue con el nuevo movimiento
    if (!measuring || finished || frame.current) return;
    let frames = 0;
    const start = performance.now();
    // Si la ventana se oculta a medias, la medida no vale (el sistema frena los fotogramas)
    let hidden = document.hidden;

    function tick(now: number) {
      frames++;
      hidden ||= document.hidden;
      if (now - start < SAMPLE_MS) {
        frame.current = requestAnimationFrame(tick);
        return;
      }
      frame.current = 0;
      if (!hidden && !finished) record((frames * 1000) / (now - start) < SLOW_FPS);
    }
    frame.current = requestAnimationFrame(tick);
  }, [trigger, measuring]);

  // Al salir del carrusel, la medida a medias se tira
  useEffect(
    () => () => {
      cancelAnimationFrame(frame.current);
      frame.current = 0;
    },
    [],
  );
}
