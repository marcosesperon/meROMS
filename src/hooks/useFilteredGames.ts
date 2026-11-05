import { useMemo } from "react";
import type { Game } from "../types";
import { useAppStore } from "../store/useAppStore";
import { lacksMedia } from "../utils/mediaFilter";
import { isPlayed } from "../utils/playStats";

/** Region codes of a game: the field may hold several («us, eu», «us/eu») */
export function regionsOf(region?: string): string[] {
  return (region ?? "")
    .split(/[,;/|\s]+/)
    .map((r) => r.trim().toLowerCase())
    .filter(Boolean);
}

export function useFilteredGames(): Game[] {
  const games = useAppStore((s) => s.games);
  const searchQuery = useAppStore((s) => s.searchQuery);
  const filters = useAppStore((s) => s.filters);
  const activeFolderPath = useAppStore((s) => s.activeFolderPath);
  const datMarks = useAppStore((s) => s.datMarks);
  const statuses = useAppStore((s) => s.statuses);
  const brokenMedia = useAppStore((s) => s.brokenMedia);

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
    if (filters.region) {
      result = result.filter((g) => regionsOf(g.region).includes(filters.region!));
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
    // DAT: hide clones (BIOS and devices too: they aren't games)
    if (filters.hideClones && datMarks) {
      result = result.filter((g) => !datMarks.has(g.path));
    }

    // «Le falta»: sin ese tipo de media (campo vacío o ruta a un fichero que ya no existe)
    if (filters.missingMedia) {
      result = result.filter((g) => lacksMedia(g, filters.missingMedia!, brokenMedia));
    }

    // Jugados / sin jugar (según las estadísticas que guarda el frontend)
    if (filters.played) {
      const wanted = filters.played === "played";
      result = result.filter((g) => isPlayed(g) === wanted);
    }

    // Estado de juego: un estado de partida, una de las marcas, o «sin estado»
    if (filters.status) {
      const wanted = filters.status;
      result = result.filter((g) => {
        const s = statuses.get(g.path);
        if (wanted === "none") return !s;
        if (wanted === "playing") return !!s?.playing;
        if (wanted === "backlog") return !!s?.backlog;
        return s?.status === wanted;
      });
    }

    return result;
  }, [games, searchQuery, filters, activeFolderPath, datMarks, brokenMedia, statuses]);
}
