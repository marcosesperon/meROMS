import { useAppStore } from "../store/useAppStore";

/**
 * Ir a un juego de cualquier sistema (búsqueda global, estadísticas de la biblioteca): abre su
 * sistema, espera a que carguen sus juegos, quita búsqueda y filtros (podrían ocultarlo), lo
 * selecciona y, si «Al pulsar un juego» es la ficha completa, la abre.
 */
export async function goToGame(systemPath: string, gamePath: string) {
  const store = useAppStore.getState();
  const system = store.systems.find((s) => s.path === systemPath);
  if (!system) return;
  store.closeFullView();
  store.closeEditor();
  if (store.multiSelect) store.setMultiSelect(false);
  if (store.selectedSystem?.path !== system.path) {
    store.selectSystem(system);
    // Esperar a que estén sus juegos (como mucho unos segundos)
    const end = Date.now() + 8000;
    while (Date.now() < end) {
      const s = useAppStore.getState();
      if (s.games.length > 0 && !s.loading && s.selectedSystem?.path === system.path) break;
      await new Promise((r) => setTimeout(r, 80));
    }
  }
  const s = useAppStore.getState();
  s.setSearchQuery("");
  s.clearFilters();
  const game = s.games.find((g) => g.path === gamePath);
  if (!game) return;
  s.selectGame(game);
  setTimeout(() => {
    document.querySelector(`[data-game-path="${CSS.escape(game.path)}"]`)?.scrollIntoView({ block: "center" });
  }, 150);
  if (s.settings.click_action === "full") s.openFullView();
}
