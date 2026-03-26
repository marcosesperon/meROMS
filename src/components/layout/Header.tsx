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

  const hasActiveFilters =
    filters.genre ||
    filters.developer ||
    filters.favorite ||
    filters.hidden ||
    filters.kidgame;

  return (
    <div>
      <header className="flex items-center justify-between px-6 py-4 border-b border-zinc-700 bg-zinc-900/50">
        <div className="flex items-center gap-4">
          <SystemLogo name={selectedSystem.name} width={140} maxHeight={40} />
          <div>
            <h2 className="text-xl font-semibold text-zinc-100">
              {selectedSystem.full_name}
            </h2>
            <p className="text-xs text-zinc-500">
              {games.length} {t("header.games")}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-4">
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
