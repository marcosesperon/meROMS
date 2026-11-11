import type { SystemInfo } from "../types";

/** Juegos de un sistema: los de su gamelist o, si no tiene, las ROMs de la carpeta */
export const systemGameCount = (system: SystemInfo) => (system.has_gamelist ? system.game_count : system.rom_count);

/**
 * Los sistemas que enseña la barra lateral: todos o, con «solo con juegos», los que tienen alguno.
 * El sistema elegido se queda en la lista aunque se quede sin juegos. Lo usan la barra y el modo
 * sofá (L/R pasan por estos, en este orden).
 */
export function sidebarSystems(systems: SystemInfo[], onlyWithGames: boolean, selectedPath?: string): SystemInfo[] {
  return onlyWithGames ? systems.filter((s) => systemGameCount(s) > 0 || s.path === selectedPath) : systems;
}
