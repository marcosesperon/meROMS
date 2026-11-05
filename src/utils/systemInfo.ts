import data from "../data/systemInfo.json";

/** Texto en los dos idiomas, o el mismo para los dos (nombres propios, cifras…) */
export type Localized = string | { es: string; en: string };

export type SystemType = "console" | "handheld" | "computer" | "arcade" | "engine" | "software";

export interface SystemInfoEntry {
  manufacturer?: Localized;
  year?: number | null;
  type?: SystemType;
  generation?: number;
  /** Dos colores (#rrggbb) para el degradado de la cabecera del sistema */
  gradient?: string[];
  cpu?: Localized;
  memory?: Localized;
  resolution?: Localized;
  colors?: Localized;
  sound?: Localized;
  media?: Localized;
  unitsSold?: Localized;
  description?: { es: string; en: string };
}

// Ficha de cada sistema (por el nombre de su carpeta). `sameAs` = mismo sistema con otro nombre.
const DB = data as Record<string, SystemInfoEntry & { sameAs?: string }>;

/** Ficha del sistema, o undefined si no hay. Sigue los alias (genesis → megadrive…). */
export function getSystemInfo(name: string): SystemInfoEntry | undefined {
  let entry = DB[name.toLowerCase()];
  if (entry?.sameAs) entry = DB[entry.sameAs];
  return entry;
}

// Degradado para los sistemas sin colores propios: grises con un toque violeta
const NEUTRAL_GRADIENT = ["#4c4458", "#2a2622"];

/**
 * Fondo de la cabecera del sistema: diagonal con sus dos colores (algo transparentes para que no
 * deslumbren), sobre el fondo del panel. Sin colores propios, el degradado neutro.
 */
export function systemGradient(name: string): string {
  const colors = getSystemInfo(name)?.gradient;
  const [from, to] = colors?.length === 2 ? colors : NEUTRAL_GRADIENT;
  return `linear-gradient(135deg, ${from}cc, ${to}99)`;
}

/** Elige el texto del idioma de la app */
export function localize(value: Localized | undefined, locale: string): string | undefined {
  if (value === undefined) return undefined;
  return typeof value === "string" ? value : locale === "es" ? value.es : value.en;
}
