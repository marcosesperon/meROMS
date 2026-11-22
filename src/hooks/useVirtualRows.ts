import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import type { ScrollBlock } from "../utils/scrollToGame";

/** Filas de más que se pintan por encima y por debajo de lo visible (el scroll rápido no ve huecos) */
const OVERSCAN = 3;

interface VirtualRowsOptions {
  /** El contenedor que se desplaza */
  containerRef: RefObject<HTMLElement | null>;
  /** Cuántas filas hay en total */
  rowCount: number;
  /** Lo que ocupa cada fila con su separación (px); todas miden lo mismo */
  rowStride: number;
  /** Dónde empieza la primera fila dentro del contenedor (relleno, cabecera…) */
  offset: number;
  /** Lo que queda tapado arriba (cabecera fija) o el margen que se deja al llevar una fila arriba */
  inset: number;
}

/**
 * Pintar solo las filas que se ven (más un margen): con miles de juegos, montar todas las
 * tarjetas o filas de golpe hace la lista lenta y pide todas las imágenes a la vez. Como todas
 * las filas miden lo mismo (misma forma de tarjeta, misma altura de fila), la cuenta es directa:
 * huecos arriba y abajo (`before`/`after`) con lo que ocuparían las que no se pintan.
 */
export function useVirtualRows({ containerRef, rowCount, rowStride, offset, inset }: VirtualRowsOptions) {
  const [view, setView] = useState({ top: 0, height: 0 });

  // Posición del scroll y alto visible (un aviso por fotograma como mucho)
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    let frame = 0;
    const read = () => {
      frame = 0;
      setView((v) => (v.top === el.scrollTop && v.height === el.clientHeight ? v : { top: el.scrollTop, height: el.clientHeight }));
    };
    const onScroll = () => {
      if (!frame) frame = requestAnimationFrame(read);
    };
    read();
    el.addEventListener("scroll", onScroll, { passive: true });
    const observer = new ResizeObserver(onScroll);
    observer.observe(el);
    return () => {
      el.removeEventListener("scroll", onScroll);
      observer.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, [containerRef]);

  const stride = Math.max(1, rowStride);
  // Filas a la vista de verdad (para la barra de letras) y las que se pintan (con margen)
  const visibleFirst = Math.min(rowCount, Math.max(0, Math.floor((view.top - offset) / stride)));
  const visibleLast = Math.min(rowCount, Math.max(visibleFirst, Math.ceil((view.top - offset + view.height) / stride)));
  const first = Math.max(0, visibleFirst - OVERSCAN);
  const last = Math.min(rowCount, visibleLast + OVERSCAN);

  // Llevar una fila a la vista: «nearest» sin mover si ya se ve, «center» o «start».
  // Las medidas (alto de fila, dónde empieza la primera) pueden afinarse al pintar las filas de
  // destino: el último salto se repite una vez con las medidas nuevas.
  const pending = useRef<{ row: number; block: ScrollBlock } | null>(null);
  const scrollToRow = useCallback(
    (row: number, block: ScrollBlock) => {
      const el = containerRef.current;
      if (!el) return;
      pending.current = { row, block };
      requestAnimationFrame(() => requestAnimationFrame(() => (pending.current = null)));
      const y = offset + row * stride;
      let top = el.scrollTop;
      if (block === "start") top = y - inset;
      else if (block === "center") top = y - (inset + el.clientHeight - stride) / 2;
      else if (y - inset < el.scrollTop) top = y - inset;
      else if (y + stride > el.scrollTop + el.clientHeight) top = y + stride - el.clientHeight;
      el.scrollTop = Math.max(0, top);
    },
    [containerRef, offset, inset, stride]
  );
  useLayoutEffect(() => {
    const jump = pending.current;
    if (jump) scrollToRow(jump.row, jump.block);
  }, [scrollToRow]);

  return {
    first,
    last,
    visibleFirst,
    visibleLast,
    /** Hueco de las filas de arriba que no se pintan */
    before: first * stride,
    /** Hueco de las filas de abajo que no se pintan */
    after: Math.max(0, rowCount - last) * stride,
    /** Filas que caben en la pantalla (RePág/AvPág) */
    rowsPerPage: Math.max(1, Math.floor(view.height / stride)),
    scrollToRow,
  };
}
