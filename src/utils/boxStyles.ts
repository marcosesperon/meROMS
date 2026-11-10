/*
 * Cómo es la caja de verdad de cada sistema, para las cajas 3D de la cuadrícula. No se dibuja
 * una por sistema: hay unas pocas familias (cartón, estuche DVD, Blu-ray, CD jewel, clamshell
 * de Mega Drive, UMD) con su grosor y su material, y cada sistema añade sus colores (el
 * plástico y la banda de arriba del lomo, donde va el logo). Un sistema que no está en la
 * tabla usa la caja de cartón.
 */

export type BoxFamily = "cardboard" | "dvd" | "bluray" | "jewel" | "clamshell" | "umd";

export interface BoxStyle {
  family: BoxFamily;
  /** Grosor de la caja, en partes del ancho del frente (algo exagerado para que el lomo se lea) */
  depth: number;
  /** Plástico de la caja: el canto alrededor del frente; null = cartón, sin canto */
  plastic: string | null;
  /** Banda de arriba del lomo con el logo; null = el lomo entero del color de la carátula */
  band: string | null;
  /** El logo de la banda en oscuro (los logos son blancos; en una banda clara no se verían) */
  darkLogo?: boolean;
  /** Esquinas redondeadas, en % del ancho */
  radius: number;
  /** Brillo de plástico sobre el frente */
  gloss: boolean;
}

/** Lo propio de cada familia; los colores los pone cada sistema */
const FAMILIES: Record<BoxFamily, Omit<BoxStyle, "family" | "plastic" | "band" | "darkLogo">> = {
  cardboard: { depth: 0.22, radius: 1.5, gloss: false },
  dvd: { depth: 0.13, radius: 4, gloss: true },
  bluray: { depth: 0.12, radius: 4, gloss: true },
  jewel: { depth: 0.09, radius: 1.5, gloss: true },
  clamshell: { depth: 0.16, radius: 3, gloss: true },
  umd: { depth: 0.15, radius: 5, gloss: true },
};

type SystemBox = { family: BoxFamily; plastic?: string; band?: string; darkLogo?: boolean };

/** Estuches de plástico blanco con el logo oscuro en la banda */
const WHITE = { plastic: "#eef0f2", band: "#f4f5f6", darkLogo: true };

/** Sistemas con caja propia (por carpeta de ES-DE); el resto, cartón */
const SYSTEM_BOXES: Record<string, SystemBox> = {
  // Estuches tipo DVD
  ps2: { family: "dvd", plastic: "#141414", band: "#0d0d0d" },
  xbox: { family: "dvd", plastic: "#161616", band: "#4f9a1c" },
  xbox360: { family: "dvd", plastic: "#5fae2e", band: "#6cbf32" },
  wii: { family: "dvd", ...WHITE },
  wiiu: { family: "dvd", plastic: "#1288c9", band: "#0e7dbb" },
  gamecube: { family: "dvd", plastic: "#1a1a1a", band: "#5b3f9e" },
  gc: { family: "dvd", plastic: "#1a1a1a", band: "#5b3f9e" },
  // Estuches pequeños de portátil (forma de DVD, más finos)
  nds: { family: "dvd", plastic: "#d9dcdf", band: "#eceeef", darkLogo: true },
  n3ds: { family: "dvd", ...WHITE },
  "3ds": { family: "dvd", ...WHITE },
  psvita: { family: "dvd", plastic: "#8fb8e3", band: "#1b5fae" },
  vita: { family: "dvd", plastic: "#8fb8e3", band: "#1b5fae" },
  // Blu-ray y los de ahora
  ps3: { family: "bluray", plastic: "#2f6cc0", band: "#111111" },
  ps4: { family: "bluray", plastic: "#1f4fa3", band: "#0b3a8c" },
  ps5: { family: "bluray", ...WHITE },
  xboxone: { family: "bluray", plastic: "#107c10", band: "#0f6e0f" },
  xboxseries: { family: "bluray", plastic: "#107c10", band: "#0f6e0f" },
  switch: { family: "bluray", plastic: "#e60012", band: "#e60012" },
  // CD en caja jewel
  psx: { family: "jewel", plastic: "#1b1b1b" },
  ps1: { family: "jewel", plastic: "#1b1b1b" },
  saturn: { family: "jewel", plastic: "#1b1b1b" },
  segacd: { family: "jewel", plastic: "#1b1b1b" },
  megacd: { family: "jewel", plastic: "#1b1b1b" },
  dreamcast: { family: "jewel", plastic: "#d8dee4" },
  pcenginecd: { family: "jewel", plastic: "#1b1b1b" },
  tg16cd: { family: "jewel", plastic: "#1b1b1b" },
  "3do": { family: "jewel", plastic: "#1b1b1b" },
  neogeocd: { family: "jewel", plastic: "#1b1b1b" },
  pcfx: { family: "jewel", plastic: "#1b1b1b" },
  cdi: { family: "jewel", plastic: "#1b1b1b" },
  amigacd32: { family: "jewel", plastic: "#1b1b1b" },
  // Cajas de plástico negro de Sega (las europeas de Mega Drive y Master System)
  megadrive: { family: "clamshell", plastic: "#111111", band: "#111111" },
  genesis: { family: "clamshell", plastic: "#111111", band: "#111111" },
  mastersystem: { family: "clamshell", plastic: "#111111", band: "#111111" },
  sega32x: { family: "clamshell", plastic: "#111111", band: "#111111" },
  // UMD de PSP: estrecha, de plástico transparente con la banda negra
  psp: { family: "umd", plastic: "#cfd6dc", band: "#111111" },
};

/** La caja de un sistema (por su carpeta) */
export function boxStyleFor(systemName: string): BoxStyle {
  const own = SYSTEM_BOXES[systemName.toLowerCase()] ?? { family: "cardboard" as BoxFamily };
  return {
    family: own.family,
    ...FAMILIES[own.family],
    plastic: own.plastic ?? null,
    band: own.band ?? null,
    darkLogo: own.darkLogo,
  };
}

/** Color de fondo del lomo cuando no se puede sacar de la carátula */
export const SPINE_FALLBACK = "#2a2a2e";

/** Color del texto encima de un fondo: blanco sobre oscuro, casi negro sobre claro */
export function textColorOn(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? "#18181b" : "#ffffff";
}

/*
 * Color del lomo: la media de una tira del borde izquierdo de la carátula. Se lee una vez por
 * imagen (se guarda en memoria) cargándola aparte con CORS, para que el canvas deje leerla
 * (el protocolo media:// manda la cabecera); si falla, el lomo usa el color de reserva.
 */
const spineColors = new Map<string, string | null>();
const pending = new Map<string, Promise<string | null>>();

export function cachedSpineColor(src: string): string | null | undefined {
  return spineColors.get(src);
}

export function spineColor(src: string): Promise<string | null> {
  if (spineColors.has(src)) return Promise.resolve(spineColors.get(src) ?? null);
  const running = pending.get(src);
  if (running) return running;
  const job = new Promise<string | null>((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(edgeColor(img));
    img.onerror = () => resolve(null);
    img.src = src;
  }).then((color) => {
    spineColors.set(src, color);
    pending.delete(src);
    return color;
  });
  pending.set(src, job);
  return job;
}

/** Media de la tira izquierda (un 6 % del ancho) de una imagen ya cargada */
function edgeColor(img: HTMLImageElement): string | null {
  try {
    const canvas = document.createElement("canvas");
    canvas.width = 4;
    canvas.height = 32;
    const ctx = canvas.getContext("2d");
    if (!ctx || !img.naturalWidth) return null;
    ctx.drawImage(img, 0, 0, Math.max(1, img.naturalWidth * 0.06), img.naturalHeight, 0, 0, 4, 32);
    const data = ctx.getImageData(0, 0, 4, 32).data;
    let [r, g, b] = [0, 0, 0];
    const count = data.length / 4;
    for (let i = 0; i < data.length; i += 4) {
      r += data[i];
      g += data[i + 1];
      b += data[i + 2];
    }
    const hex = (v: number) => Math.round(v / count).toString(16).padStart(2, "0");
    return `#${hex(r)}${hex(g)}${hex(b)}`;
  } catch {
    // Canvas «contaminado» (sin CORS): no se puede leer
    return null;
  }
}
