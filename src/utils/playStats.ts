import type { Game } from "../types";
import type { TranslationKeys } from "../i18n/en";

// Estadísticas de juego que guarda el frontend en la gamelist: partidas (playcount),
// última vez (lastplayed, AAAAMMDDTHHMMSS) y tiempo jugado (gametime, en segundos).

/** Segundos de juego → «1 h 25 min» / «12 min» */
export function formatPlayTime(seconds?: string): string {
  const s = Number(seconds ?? 0);
  if (!s) return "";
  const h = Math.floor(s / 3600);
  const m = Math.round((s % 3600) / 60);
  return h > 0 ? `${h} h ${m} min` : `${m} min`;
}

const num = (v?: string) => Number(v ?? 0) || 0;

/** Jugado = tiene partidas o una fecha de última vez */
export function isPlayed(game: Game): boolean {
  return num(game.playcount) > 0 || !!game.lastplayed?.trim();
}

/** Nombre para ordenar, como hace ES-DE: «nombre para ordenar» si lo tiene */
const byName = (a: Game, b: Game) =>
  (a.sortname || a.name || a.path).localeCompare(b.sortname || b.name || b.path, undefined, { numeric: true, sensitivity: "base" });

export type GridSort = "name" | "most_played" | "recent" | "playtime" | "release" | "rating";

/**
 * Criterios de orden de la cuadrícula. `key` da el valor (mayor primero); los juegos sin dato
 * van al final, ordenados por nombre.
 */
export const GRID_SORTS: { id: GridSort; label: TranslationKeys; key?: (g: Game) => number | string }[] = [
  { id: "name", label: "sort.name" },
  { id: "most_played", label: "sort.mostPlayed", key: (g) => num(g.playcount) },
  { id: "recent", label: "sort.recent", key: (g) => g.lastplayed ?? "" },
  { id: "playtime", label: "sort.playtime", key: (g) => num(g.gametime) },
  { id: "release", label: "sort.release", key: (g) => g.releasedate ?? "" },
  { id: "rating", label: "sort.rating", key: (g) => Number(g.rating ?? 0) || 0 },
];

export function sortGames(games: Game[], sort: GridSort | undefined): Game[] {
  const key = GRID_SORTS.find((s) => s.id === sort)?.key;
  if (!key) return [...games].sort(byName);
  const empty = (v: number | string) => v === 0 || v === "";
  return [...games].sort((a, b) => {
    const va = key(a);
    const vb = key(b);
    if (empty(va) !== empty(vb)) return empty(va) ? 1 : -1;
    if (va !== vb) return va > vb ? -1 : 1;
    return byName(a, b);
  });
}
