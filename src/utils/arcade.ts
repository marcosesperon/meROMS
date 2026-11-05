/**
 * Sistemas cuyas ROMs llevan el nombre corto de MAME (se consultan en ArcadeItalia, que no
 * necesita cuenta). Misma lista que `is_arcade_system` en scraper_adb.rs.
 */
const ARCADE_SYSTEMS = new Set([
  "arcade", "mame", "fba", "fbneo", "cps", "cps1", "cps2", "cps3", "neogeo",
  "naomi", "naomi2", "naomigd", "atomiswave", "model2", "model3", "stv",
]);

export const isArcadeSystem = (folderName: string) => ARCADE_SYSTEMS.has(folderName.toLowerCase());
