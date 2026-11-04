import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { SearchBar } from "../common/SearchBar";
import { ViewToggle } from "../common/ViewToggle";
import { SystemLogo } from "../common/SystemLogo";
import { FilterIcon } from "../common/Icons";
import { FilterBar } from "../common/FilterBar";

export function Header() {
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const games = useAppStore((s) => s.games);
  const filters = useAppStore((s) => s.filters);
  const { t } = useTranslation();
  const [showFilters, setShowFilters] = useState(false);

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
            className={`p-2 rounded transition-colors ${
              showFilters || hasActiveFilters
                ? "bg-indigo-600/20 text-indigo-400"
                : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700"
            }`}
            title="Filters"
          >
            <FilterIcon size={18} />
          </button>
          <ViewToggle />
        </div>
      </header>
      {showFilters && <FilterBar />}
    </div>
  );
}
