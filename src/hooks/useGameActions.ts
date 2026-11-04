import { useState } from "react";
import { useAppStore } from "../store/useAppStore";
import { useTranslation } from "../i18n";
import { removeGame, saveGamelist, scrapeGame } from "../commands";
import type { Game } from "../types";
import { askConfirm, showMessage } from "../utils/dialogs";

/**
 * Scrapear y eliminar un juego directamente desde la ficha (sin pasar por el editor).
 * A diferencia del editor, aquí el resultado del scrapeo se guarda en el acto.
 */
export function useGameActions() {
  const { t } = useTranslation();
  const [scraping, setScraping] = useState(false);

  async function scrape(game: Game) {
    const { selectedSystem, games, folders, provider, loadGames, selectGame } = useAppStore.getState();
    if (!selectedSystem) return;
    setScraping(true);
    try {
      const result = await scrapeGame(selectedSystem.path, game.path);
      if (!result.found) {
        await showMessage(t("detail.scrapeNotFound"));
        return;
      }
      const updated = games.map((g) => (g.path === game.path ? result.game : g));
      await saveGamelist(selectedSystem.path, { games: updated, folders: folders || [], provider });
      // Recargar para que los medios nuevos se vean (en ES-DE se encuentran al leer)
      await loadGames(selectedSystem.path);
      selectGame(useAppStore.getState().games.find((g) => g.path === game.path) ?? null);
    } catch (e) {
      await showMessage(t("detail.scrapeFailed", { error: String(e) }), "error");
    } finally {
      setScraping(false);
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

  return { scrape, remove, scraping };
}
