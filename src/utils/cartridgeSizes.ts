/**
 * Aspect ratios for box art / cartridge labels by system.
 * Format: [width, height] — e.g. [3, 4] means 3:4 portrait.
 *
 * Sources: physical cartridge & box art dimensions from various systems.
 * Grouped by form factor for easy maintenance.
 */

// Common aspect ratios
const NTSC_BOX = [3, 4] as const;        // Standard US/JP box art (portrait)
const DVD_BOX = [27, 38] as const;        // DVD-style case (PS2/Wii/Xbox)
const BLURAY_BOX = [27, 33] as const;     // Blu-ray case (PS3/PS4/PS5)
const JEWEL_CASE = [1, 1] as const;       // CD jewel case front (square)
const HANDHELD_BOX = [27, 32] as const;   // Handheld game box
const SQUARE = [1, 1] as const;           // Square (PC Engine, etc.)

const SYSTEM_ASPECT_RATIOS: Record<string, readonly [number, number]> = {
  // === Nintendo Home Consoles ===
  nes: NTSC_BOX,
  famicom: NTSC_BOX,
  fds: NTSC_BOX,
  snes: NTSC_BOX,
  sfc: NTSC_BOX,
  n64: NTSC_BOX,
  gamecube: [3, 4],
  gc: [3, 4],
  wii: DVD_BOX,
  wiiu: DVD_BOX,
  switch: [27, 34],

  // === Nintendo Handhelds ===
  gb: HANDHELD_BOX,
  gameboy: HANDHELD_BOX,
  gba: HANDHELD_BOX,
  gbc: HANDHELD_BOX,
  nds: HANDHELD_BOX,
  "3ds": HANDHELD_BOX,
  n3ds: HANDHELD_BOX,
  virtualboy: NTSC_BOX,

  // === Sega Home Consoles ===
  mastersystem: NTSC_BOX,
  megadrive: NTSC_BOX,
  genesis: NTSC_BOX,
  sega32x: NTSC_BOX,
  "32x": NTSC_BOX,
  segacd: JEWEL_CASE,
  saturn: JEWEL_CASE,
  dreamcast: JEWEL_CASE,

  // === Sega Handhelds ===
  gamegear: NTSC_BOX,
  gg: NTSC_BOX,

  // === Sony ===
  psx: JEWEL_CASE,
  ps1: JEWEL_CASE,
  ps2: DVD_BOX,
  ps3: BLURAY_BOX,
  ps4: BLURAY_BOX,
  ps5: BLURAY_BOX,
  psp: HANDHELD_BOX,
  vita: HANDHELD_BOX,
  psvita: HANDHELD_BOX,

  // === Microsoft ===
  xbox: DVD_BOX,
  xbox360: DVD_BOX,
  xboxone: BLURAY_BOX,

  // === Atari ===
  atari2600: NTSC_BOX,
  atari5200: NTSC_BOX,
  atari7800: NTSC_BOX,
  atarilynx: NTSC_BOX,
  lynx: NTSC_BOX,
  atarijaguar: NTSC_BOX,
  jaguar: NTSC_BOX,
  atarist: NTSC_BOX,

  // === NEC / TurboGrafx ===
  pcengine: SQUARE,
  tg16: SQUARE,
  turbografx16: SQUARE,
  pcecd: JEWEL_CASE,
  pcfx: JEWEL_CASE,
  supergrafx: SQUARE,

  // === SNK / Neo Geo ===
  neogeo: NTSC_BOX,
  neogeocd: JEWEL_CASE,
  ngp: HANDHELD_BOX,
  ngpc: HANDHELD_BOX,

  // === Arcade ===
  arcade: [3, 4],
  mame: [3, 4],
  fbneo: [3, 4],
  fba: [3, 4],
  cps: [3, 4],
  cps1: [3, 4],
  cps2: [3, 4],
  cps3: [3, 4],
  naomi: [3, 4],
  atomiswave: [3, 4],
  daphne: [3, 4],

  // === Computers ===
  msx: NTSC_BOX,
  msx2: NTSC_BOX,
  amiga: NTSC_BOX,
  c64: NTSC_BOX,
  zxspectrum: NTSC_BOX,
  amstradcpc: NTSC_BOX,
  dos: NTSC_BOX,
  pc: NTSC_BOX,
  scummvm: NTSC_BOX,

  // === Misc ===
  "3do": JEWEL_CASE,
  colecovision: NTSC_BOX,
  intellivision: NTSC_BOX,
  vectrex: NTSC_BOX,
  wonderswan: HANDHELD_BOX,
  wonderswancolor: HANDHELD_BOX,
  ws: HANDHELD_BOX,
  wsc: HANDHELD_BOX,
  channelf: NTSC_BOX,
  odyssey2: NTSC_BOX,
  sg1000: NTSC_BOX,
};

/** Default aspect ratio if system is not mapped */
const DEFAULT_RATIO: readonly [number, number] = [3, 4];

/**
 * Get the built-in default aspect ratio for a system.
 * Returns [width, height] tuple.
 */
export function getDefaultSystemRatio(systemName: string): readonly [number, number] {
  const name = systemName.toLowerCase().replace(/[^a-z0-9]/g, "");
  return SYSTEM_ASPECT_RATIOS[name] ?? DEFAULT_RATIO;
}

/**
 * Get the aspect ratio CSS string for a system's grid cards.
 * Uses custom override if provided, otherwise falls back to built-in defaults.
 * Returns a string like "3/4" for use in style={{ aspectRatio: "3/4" }}
 */
export function getSystemAspectRatio(
  systemName: string,
  customRatios?: Record<string, string>,
): string {
  const name = systemName.toLowerCase().replace(/[^a-z0-9]/g, "");

  // Check custom override
  if (customRatios?.[name]) {
    return customRatios[name];
  }

  const ratio = SYSTEM_ASPECT_RATIOS[name] ?? DEFAULT_RATIO;
  return `${ratio[0]}/${ratio[1]}`;
}

/**
 * Forma de las tarjetas de la cuadrícula de un sistema, en este orden: la proporción puesta a
 * mano en Ajustes → Sistemas, la forma que más se repite en el medio que enseña la cuadrícula
 * (medida en los ficheros del sistema: `usual_media_shape`) y, si no hay, la tabla de arriba.
 */
export function cardAspectRatio(
  systemName: string,
  customRatios?: Record<string, string>,
  measured?: { width: number; height: number } | null,
): string {
  const name = systemName.toLowerCase().replace(/[^a-z0-9]/g, "");
  if (customRatios?.[name]) return customRatios[name];
  if (measured && measured.width > 0 && measured.height > 0) return `${measured.width}/${measured.height}`;
  return getSystemAspectRatio(systemName);
}

/**
 * Get the minimum card width (in px) for the grid based on aspect ratio.
 * Wider aspect ratios get wider cards, taller ones stay narrow.
 */
export function getSystemCardWidth(
  systemName: string,
  customRatios?: Record<string, string>,
): number {
  return cardWidthFor(getSystemAspectRatio(systemName, customRatios));
}

/** Ancho mínimo (px) de la tarjeta para una proporción «ancho/alto» */
export function cardWidthFor(ratioStr: string): number {
  const parts = ratioStr.split("/").map(Number);
  const ar = parts.length === 2 ? parts[0] / parts[1] : 0.75;

  if (ar >= 2) return 260;        // very wide (marquee/wheel)
  if (ar >= 1.3) return 240;      // landscape (16:9)
  if (ar >= 1) return 220;        // square-ish
  if (ar >= 0.8) return 180;      // nearly square (jewel cases)
  return 160;                     // tall portrait (standard boxes)
}

/** Extract a flat aspect ratio map from SystemConfig records */
export function extractCustomRatios(
  configs?: Record<string, { aspect_ratio?: string }>,
): Record<string, string> | undefined {
  if (!configs) return undefined;
  const map: Record<string, string> = {};
  for (const [k, v] of Object.entries(configs)) {
    if (v.aspect_ratio) map[k] = v.aspect_ratio;
  }
  return Object.keys(map).length > 0 ? map : undefined;
}
