import { useAppStore } from "../store/useAppStore";
import type { SystemInfo } from "../types";
import { sidebarSystems } from "./systems";

/*
 * Lo que hacen los botones de acción del mando (y sus teclas en el modo sofá): jugar, favorito,
 * cambiar de sistema, solo favoritos y «¿A qué juego?». Lo comparten el modo sofá (por teclas) y
 * useGamepad fuera del modo sofá (llamándolas directamente).
 */

/** Sistema anterior o siguiente de la barra lateral, dando la vuelta (sigue con «solo favoritos») */
export function switchSystem(dir: 1 | -1) {
  const { systems, onlySystemsWithGames, selectedSystem } = useAppStore.getState();
  const list = sidebarSystems(systems, onlySystemsWithGames, selectedSystem?.path);
  if (list.length < 2) return;
  const index = list.findIndex((s) => s.path === selectedSystem?.path);
  goToSystem(list[(index + dir + list.length) % list.length]);
}

/** Ir a otro sistema: cambiarlo borra los filtros, pero «solo favoritos» se mantiene */
export function goToSystem(system: SystemInfo) {
  const { selectSystem, filters, setFilters } = useAppStore.getState();
  const favorites = !!filters.favorite;
  selectSystem(system);
  if (favorites) setFilters({ favorite: true });
}

/** Jugar el juego elegido, sin pasar por la ficha */
export function playSelected() {
  const { selectedGame, playGame } = useAppStore.getState();
  if (selectedGame) playGame(selectedGame);
}

export function toggleFavorite() {
  const { selectedGame, toggleGameFlag } = useAppStore.getState();
  if (selectedGame) toggleGameFlag(selectedGame.path, "favorite");
}

export function toggleOnlyFavorites() {
  const { filters, setFilters } = useAppStore.getState();
  setFilters({ favorite: filters.favorite ? undefined : true });
}

export function openRandom() {
  useAppStore.getState().setRandomOpen(true);
}
