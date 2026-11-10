import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { SearchBar } from "../common/SearchBar";
import { ViewToggle } from "../common/ViewToggle";
import { SystemLogo } from "../common/SystemLogo";
import { FilterIcon, SlidersIcon } from "../common/Icons";
import { SystemViewModal } from "../common/SystemViewModal";
import { FilterBar } from "../common/FilterBar";
import { GRID_SORTS, type GridSort } from "../../utils/playStats";

export function Header() {
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const games = useAppStore((s) => s.games);
  const filters = useAppStore((s) => s.filters);
  const viewMode = useAppStore((s) => s.viewMode);
  const gridSort = useAppStore((s) => s.settings.grid_sort);
  const patchSettings = useAppStore((s) => s.patchSettings);
  const { t } = useTranslation();
  const [showFilters, setShowFilters] = useState(false);
  // Ajustes de vista de este sistema (cuadrícula y detalle del juego)
  const [showView, setShowView] = useState(false);

  if (!selectedSystem) return null;

  const multiSelect = useAppStore((s) => s.multiSelect);
  const hasActiveFilters =
    multiSelect ||
    filters.genre ||
    filters.developer ||
    filters.region ||
    filters.favorite ||
    filters.hidden ||
    filters.kidgame ||
    filters.hideClones;

  return (
    <div>
      {/* Si no cabe en una línea, los controles bajan a una segunda fila (el nombre no se parte) */}
      <header className="flex flex-wrap items-center justify-between gap-x-4 gap-y-3 px-6 py-3 border-b border-zinc-800 bg-zinc-900">
        <div className="flex items-center gap-4 min-w-0 max-w-full">
          <SystemLogo name={selectedSystem.name} width={110} maxHeight={34} />
          <div className="min-w-0">
            <h2 className="text-lg font-bold tracking-tight text-zinc-100 whitespace-nowrap truncate">
              {selectedSystem.full_name}
            </h2>
            <p className="text-xs text-zinc-500">
              {games.length} {t("header.games")}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-3 ml-auto">
          <SearchBar />
          <button
            onClick={() => setShowFilters((v) => !v)}
            data-shot="filters-toggle"
            className={`p-2 rounded transition-colors ${
              showFilters || hasActiveFilters
                ? "bg-indigo-600/20 text-indigo-400"
                : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700"
            }`}
            title="Filters"
          >
            <FilterIcon size={18} />
          </button>
          {/* Orden de la cuadrícula y del carrusel (la tabla ordena por sus columnas) */}
          {viewMode !== "table" && (
            <select
              value={gridSort ?? "name"}
              onChange={(e) => patchSettings({ grid_sort: e.target.value as GridSort })}
              title={t("sort.title")}
              className="h-8 bg-zinc-700 text-zinc-200 text-xs rounded border border-zinc-600 px-2 cursor-pointer focus:outline-none focus:border-indigo-500"
            >
              {GRID_SORTS.map((s) => (
                <option key={s.id} value={s.id}>
                  {t(s.label)}
                </option>
              ))}
            </select>
          )}
          <button
            onClick={() => setShowView(true)}
            data-shot="system-view"
            className="p-2 rounded text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700 transition-colors"
            title={t("systemView.button")}
          >
            <SlidersIcon size={18} />
          </button>
          <ViewToggle />
        </div>
      </header>
      {showFilters && <FilterBar />}
      <SystemViewModal open={showView} system={selectedSystem} onClose={() => setShowView(false)} />
    </div>
  );
}
