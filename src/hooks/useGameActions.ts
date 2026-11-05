import { useState } from "react";
import { useAppStore } from "../store/useAppStore";
import { useTranslation } from "../i18n";
import { removeGame, saveGamelist } from "../commands";
import type { Game } from "../types";
import { askConfirm, showMessage } from "../utils/dialogs";

/**
 * Guarda en la gamelist un juego recién scrapeado y recarga, para que la media nueva se vea
 * (en ES-DE se encuentra al leer). Lo usan la ficha y la lista de no encontrados de «Scrapear todo».
 */
export async function saveScrapedGame(scraped: Game) {
  const { selectedSystem, games, folders, provider, loadGames, selectedGame, selectGame } = useAppStore.getState();
  if (!selectedSystem) return;
  const updated = games.map((g) => (g.path === scraped.path ? scraped : g));
  await saveGamelist(selectedSystem.path, { games: updated, folders: folders || [], provider });
  await loadGames(selectedSystem.path);
  // Si era el juego seleccionado, se vuelve a seleccionar con los datos nuevos
  if (selectedGame?.path === scraped.path) {
    selectGame(useAppStore.getState().games.find((g) => g.path === scraped.path) ?? null);
  }
}

/**
 * Scrapear y eliminar un juego directamente desde la ficha (sin pasar por el editor).
 * Scrapear abre la búsqueda manual (`scrapeTarget`); el resultado elegido se guarda en el acto.
 */
export function useGameActions() {
  const { t } = useTranslation();
  // Juego con la ventana de búsqueda abierta (null = cerrada)
  const [scrapeTarget, setScrapeTarget] = useState<Game | null>(null);

  function scrape(game: Game) {
    setScrapeTarget(game);
  }

  async function applyScraped(game: Game) {
    try {
      await saveScrapedGame(game);
    } catch (e) {
      await showMessage(t("detail.saveFailed", { error: String(e) }), "error");
    }
  }

  async function remove(game: Game) {
    const { selectedSystem, setGames, selectGame } = useAppStore.getState();
    if (!selectedSystem) return;
    if (!(await askConfirm(t("detail.deleteConfirm", { name: game.name || game.path })))) return;
    try {
      const result = await removeGame(selectedSystem.path, game.path);
      setGames(result.games);
      selectGame(null);
    } catch (e) {
      await showMessage(t("detail.deleteFailed", { error: String(e) }), "error");
    }
  }

  return { scrape, remove, scrapeTarget, closeScrape: () => setScrapeTarget(null), applyScraped };
}
