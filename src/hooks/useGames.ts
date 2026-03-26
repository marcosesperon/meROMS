import { useEffect } from "react";
import { useAppStore } from "../store/useAppStore";

export function useGames() {
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const games = useAppStore((s) => s.games);
  const searchQuery = useAppStore((s) => s.searchQuery);
  const loading = useAppStore((s) => s.loading);

  useEffect(() => {
    if (selectedSystem) {
      useAppStore.getState().loadGames(selectedSystem.path);
    }
  }, [selectedSystem?.path]);

  const filteredGames = searchQuery
    ? games.filter(
        (g) =>
          g.name?.toLowerCase().includes(searchQuery.toLowerCase()) ||
          g.path.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : games;

  return { games: filteredGames, totalCount: games.length, loading };
}
