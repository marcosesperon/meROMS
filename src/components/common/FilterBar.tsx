import { useMemo } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useFilteredGames } from "../../hooks/useFilteredGames";
import { useTranslation } from "../../i18n";
import { StarIcon, EyeSlashIcon, ChildIcon } from "./Icons";

// Los selectores, con el mismo aspecto que los botones de al lado (alto, borde, fondo).
// El aspecto nativo de macOS no deja cambiar el alto, así que se quita y se pinta la flecha.
const SELECT_CLASS =
  "appearance-none h-8 bg-zinc-700 text-zinc-200 text-xs rounded border border-zinc-600 pl-2.5 pr-7 cursor-pointer hover:text-zinc-100 focus:outline-none focus:border-indigo-500 transition-colors";
const SELECT_ARROW = {
  backgroundImage:
    "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 24 24' fill='none' stroke='%23a59c96' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E\")",
  backgroundRepeat: "no-repeat",
  backgroundPosition: "right 0.6rem center",
};

export function FilterBar() {
  const games = useAppStore((s) => s.games);
  const filters = useAppStore((s) => s.filters);
  const setFilters = useAppStore((s) => s.setFilters);
  const clearFilters = useAppStore((s) => s.clearFilters);
  const multiSelect = useAppStore((s) => s.multiSelect);
  const setMultiSelect = useAppStore((s) => s.setMultiSelect);
  const selectedCount = useAppStore((s) => s.selectedPaths.size);
  const setSelectedPaths = useAppStore((s) => s.setSelectedPaths);
  const visibleGames = useFilteredGames();
  const hasDat = useAppStore((s) => s.datMarks !== null);
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
    filters.kidgame ||
    filters.hideClones;

  return (
    <div className="flex items-center gap-3 px-6 py-2.5 bg-zinc-800/50 border-b border-zinc-700 flex-wrap">
      {/* Genre dropdown */}
      <select
        value={filters.genre || ""}
        onChange={(e) =>
          setFilters({ genre: e.target.value || undefined })
        }
        className={SELECT_CLASS}
        style={SELECT_ARROW}
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
        className={SELECT_CLASS}
        style={SELECT_ARROW}
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
        className={`flex items-center gap-1 h-8 text-xs px-2.5 rounded border transition-colors ${
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
        className={`flex items-center gap-1 h-8 text-xs px-2.5 rounded border transition-colors ${
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
        className={`flex items-center gap-1 h-8 text-xs px-2.5 rounded border transition-colors ${
          filters.kidgame
            ? "bg-green-500/20 border-green-500/50 text-green-300"
            : "bg-zinc-700 border-zinc-600 text-zinc-400 hover:text-zinc-200"
        }`}
      >
        <ChildIcon size={12} />
        {t("filter.kidgames")}
      </button>

      {/* Solo si el sistema tiene DAT: ocultar clones (y BIOS/dispositivos) */}
      {hasDat && (
        <button
          onClick={() => setFilters({ hideClones: !filters.hideClones })}
          className={`flex items-center gap-1 h-8 text-xs px-2.5 rounded border transition-colors ${
            filters.hideClones
              ? "bg-sky-500/20 border-sky-500/50 text-sky-300"
              : "bg-zinc-700 border-zinc-600 text-zinc-400 hover:text-zinc-200"
          }`}
        >
          {t("dat.hideClones")}
        </button>
      )}

      {/* Selección múltiple: el clic marca juegos y el panel derecho pasa a la edición en bloque */}
      <div className="flex items-center gap-2 h-8 pl-3 border-l border-zinc-700">
        <button
          role="switch"
          aria-checked={multiSelect}
          onClick={() => setMultiSelect(!multiSelect)}
          className="flex items-center gap-2 text-xs text-zinc-300"
        >
          <span className={`relative w-8 h-[18px] rounded-full transition-colors ${multiSelect ? "bg-indigo-600" : "bg-zinc-600"}`}>
            <span
              className={`absolute top-[3px] w-3 h-3 rounded-full bg-white transition-all ${multiSelect ? "left-[17px]" : "left-[3px]"}`}
            />
          </span>
          {t("bulk.multiSelect")}
        </button>
        {multiSelect && (
          <>
            <span className="text-xs text-zinc-500">{t("bulk.selected", { count: selectedCount })}</span>
            <button
              onClick={() => setSelectedPaths(new Set(visibleGames.map((g) => g.path)))}
              className="text-xs text-indigo-400 hover:text-indigo-300"
            >
              {t("bulk.selectVisible")}
            </button>
            <button onClick={() => setSelectedPaths(new Set())} className="text-xs text-indigo-400 hover:text-indigo-300">
              {t("bulk.selectNone")}
            </button>
          </>
        )}
      </div>

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
