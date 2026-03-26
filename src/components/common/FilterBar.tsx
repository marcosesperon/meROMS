import { useMemo } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { StarIcon, EyeSlashIcon, ChildIcon } from "./Icons";

export function FilterBar() {
  const games = useAppStore((s) => s.games);
  const filters = useAppStore((s) => s.filters);
  const setFilters = useAppStore((s) => s.setFilters);
  const clearFilters = useAppStore((s) => s.clearFilters);
  const { t } = useTranslation();

  const uniqueGenres = useMemo(() => {
    const set = new Set<string>();
    games.forEach((g) => {
      if (g.genre) set.add(g.genre);
    });
    return Array.from(set).sort();
  }, [games]);

  const uniqueDevelopers = useMemo(() => {
    const set = new Set<string>();
    games.forEach((g) => {
      if (g.developer) set.add(g.developer);
    });
    return Array.from(set).sort();
  }, [games]);

  const hasActiveFilters =
    filters.genre ||
    filters.developer ||
    filters.favorite ||
    filters.hidden ||
    filters.kidgame;

  return (
    <div className="flex items-center gap-3 px-6 py-2.5 bg-zinc-800/50 border-b border-zinc-700 flex-wrap">
      {/* Genre dropdown */}
      <select
        value={filters.genre || ""}
        onChange={(e) =>
          setFilters({ genre: e.target.value || undefined })
        }
        className="bg-zinc-700 text-zinc-200 text-xs rounded px-2 py-1.5 border border-zinc-600 focus:outline-none focus:border-indigo-500"
      >
        <option value="">{t("filter.genre")}: {t("filter.all")}</option>
        {uniqueGenres.map((g) => (
          <option key={g} value={g}>
            {g}
          </option>
        ))}
      </select>

      {/* Developer dropdown */}
      <select
        value={filters.developer || ""}
        onChange={(e) =>
          setFilters({ developer: e.target.value || undefined })
        }
        className="bg-zinc-700 text-zinc-200 text-xs rounded px-2 py-1.5 border border-zinc-600 focus:outline-none focus:border-indigo-500"
      >
        <option value="">{t("filter.developer")}: {t("filter.all")}</option>
        {uniqueDevelopers.map((d) => (
          <option key={d} value={d}>
            {d}
          </option>
        ))}
      </select>

      {/* Toggle buttons */}
      <button
        onClick={() => setFilters({ favorite: !filters.favorite })}
        className={`flex items-center gap-1 text-xs px-2.5 py-1.5 rounded border transition-colors ${
          filters.favorite
            ? "bg-yellow-500/20 border-yellow-500/50 text-yellow-300"
            : "bg-zinc-700 border-zinc-600 text-zinc-400 hover:text-zinc-200"
        }`}
      >
        <StarIcon size={12} />
        {t("filter.favorites")}
      </button>

      <button
        onClick={() => setFilters({ hidden: !filters.hidden })}
        className={`flex items-center gap-1 text-xs px-2.5 py-1.5 rounded border transition-colors ${
          filters.hidden
            ? "bg-red-500/20 border-red-500/50 text-red-300"
            : "bg-zinc-700 border-zinc-600 text-zinc-400 hover:text-zinc-200"
        }`}
      >
        <EyeSlashIcon size={12} />
        {t("filter.hidden")}
      </button>

      <button
        onClick={() => setFilters({ kidgame: !filters.kidgame })}
        className={`flex items-center gap-1 text-xs px-2.5 py-1.5 rounded border transition-colors ${
          filters.kidgame
            ? "bg-green-500/20 border-green-500/50 text-green-300"
            : "bg-zinc-700 border-zinc-600 text-zinc-400 hover:text-zinc-200"
        }`}
      >
        <ChildIcon size={12} />
        {t("filter.kidgames")}
      </button>

      {/* Clear all */}
      {hasActiveFilters && (
        <button
          onClick={clearFilters}
          className="text-xs text-zinc-500 hover:text-zinc-300 ml-auto transition-colors"
        >
          {t("filter.clearAll")}
        </button>
      )}
    </div>
  );
}
