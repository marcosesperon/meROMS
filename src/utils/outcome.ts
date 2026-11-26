import type { TranslationKeys } from "../i18n/en";

/**
 * Una pieza del resumen de lo hecho («30 ficheros movidos»): la clave del texto y su número
 * aparte, para que el resumen de Ponerlo todo en orden pueda destacar la cifra y dejar fuera
 * lo que queda a 0. Sin `count`, es un texto sin número («Scrapeo parado…»).
 */
export interface OutcomeItem {
  key: TranslationKeys;
  /** El número que va en `{count}` */
  count?: number;
  /** Otros valores del texto (p. ej. la carpeta de apartados) */
  params?: Record<string, string | number>;
}

export const outcomeItem = (key: TranslationKeys, count?: number, params?: Record<string, string | number>): OutcomeItem => ({ key, count, params });

/** Fuera las piezas que se han quedado a 0 */
export const withoutZeros = (items: OutcomeItem[]) => items.filter((i) => i.count === undefined || i.count > 0);
