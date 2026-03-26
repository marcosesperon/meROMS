import { useMemo } from "react";
import type { Game } from "../types";
import { useAppStore } from "../store/useAppStore";

export function useFilteredGames(): Game[] {
  const games = useAppStore((s) => s.games);
  const searchQuery = useAppStore((s) => s.searchQuery);
  const filters = useAppStore((s) => s.filters);
  const activeFolderPath = useAppStore((s) => s.activeFolderPath);

  return useMemo(() => {
    let result = games;

    // Filter by active folder path
    if (activeFolderPath) {
      result = result.filter((g) => g.path.startsWith(activeFolderPath));
    }

    // Apply search query
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      result = result.filter((g) => {
        const name = (g.name || g.path).toLowerCase();
        const genre = (g.genre || "").toLowerCase();
        const dev = (g.developer || "").toLowerCase();
        const pub = (g.publisher || "").toLowerCase();
        const desc = (g.desc || "").toLowerCase();
        return (
          name.includes(q) ||
          genre.includes(q) ||
          dev.includes(q) ||
          pub.includes(q) ||
          desc.includes(q)
        );
      });
    }

    // Apply filters
    if (filters.genre) {
      result = result.filter((g) => g.genre === filters.genre);
    }
    if (filters.developer) {
      result = result.filter((g) => g.developer === filters.developer);
    }
    if (filters.favorite) {
      result = result.filter((g) => g.favorite === "true");
    }
    if (filters.hidden) {
      result = result.filter((g) => g.hidden === "true");
    }
    if (filters.kidgame) {
      result = result.filter((g) => g.kidgame === "true");
    }

    return result;
  }, [games, searchQuery, filters, activeFolderPath]);
}
