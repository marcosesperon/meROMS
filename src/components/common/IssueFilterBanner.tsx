import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { useAppStore } from "../../store/useAppStore";
import { useFilteredGames } from "../../hooks/useFilteredGames";

/**
 * Franja encima de la lista cuando se ha pulsado un número del estado del sistema: dice qué se
 * está viendo («Con problemas de: sin datos obligatorios») y deja quitar ese filtro.
 */
export function IssueFilterBanner() {
  const { t } = useTranslation();
  const issue = useAppStore((s) => s.filters.issue);
  const setFilters = useAppStore((s) => s.setFilters);
  const shown = useFilteredGames().length;
  if (!issue) return null;
  return (
    <div className="px-6 py-1.5 shrink-0 border-b border-indigo-800/50 bg-indigo-600/10 flex items-center gap-3 text-xs">
      <span className="text-indigo-200">
        {t("colstatus.filtering", { what: t(`colstatus.filter_${issue}` as TranslationKeys), count: shown })}
      </span>
      <button onClick={() => setFilters({ issue: undefined })} className="text-indigo-300 hover:text-indigo-100 underline">
        {t("colstatus.clearFilter")}
      </button>
    </div>
  );
}
