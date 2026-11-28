/**
 * Sistemas cuyas ROMs llevan el nombre corto de MAME (se consultan en ArcadeItalia, que no
 * necesita cuenta). Misma lista que `is_arcade_system` en scraper_adb.rs.
 */
const ARCADE_SYSTEMS = new Set([
  "arcade", "mame", "fba", "fbneo", "cps", "cps1", "cps2", "cps3", "neogeo",
  "naomi", "naomi2", "naomigd", "atomiswave", "model2", "model3", "stv",
]);

export const isArcadeSystem = (folderName: string) => ARCADE_SYSTEMS.has(folderName.toLowerCase());

/** Arcade systems whose sets are checked against the MAME/FBNeo cores ("Emulator per game"): all but
 * Model 2/3, which use other emulators. Same as `is_checked_system` in commands/arcade.rs. */
export const isCheckedArcadeSystem = (folderName: string) => isArcadeSystem(folderName) && !["model2", "model3"].includes(folderName.toLowerCase());
