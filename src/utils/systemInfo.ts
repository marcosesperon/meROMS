import data from "../data/systemInfo.json";

/** Texto en los dos idiomas, o el mismo para los dos (nombres propios, cifras…) */
export type Localized = string | { es: string; en: string };

export type SystemType = "console" | "handheld" | "computer" | "arcade" | "engine" | "software";

export interface SystemInfoEntry {
  manufacturer?: Localized;
  year?: number | null;
  type?: SystemType;
  generation?: number;
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

/** Elige el texto del idioma de la app */
export function localize(value: Localized | undefined, locale: string): string | undefined {
  if (value === undefined) return undefined;
  return typeof value === "string" ? value : locale === "es" ? value.es : value.en;
}
