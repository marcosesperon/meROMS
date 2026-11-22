import { useTranslation } from "../../i18n";
import type { NameScraper } from "../../types";
import { usableNameScrapers } from "../../utils/renameSources";
import { useAppStore } from "../../store/useAppStore";

interface NameScraperPickerProps {
  scraper: NameScraper | null;
  onChange: (scraper: NameScraper) => void;
  /** Lanza la búsqueda: hasta entonces no se pregunta nada al scraper */
  onSearch: () => void;
  disabled?: boolean;
}

/**
 * Fuente «Scraper» de Renombrar: qué scraper da el nombre de fichero y el botón «Buscar».
 * Se usa en la ventana del sistema y en la de un juego; las consultas gastan cupo, así que
 * nada se pide hasta pulsar «Buscar».
 */
export function NameScraperPicker({ scraper, onChange, onSearch, disabled }: NameScraperPickerProps) {
  const { t } = useTranslation();
  const scrapers = usableNameScrapers(useAppStore((s) => s.scraperStatus));
  return (
    <div className="flex items-center gap-2 mt-2">
      <label className="text-xs text-zinc-400">{t("rename.scraperPick")}</label>
      <select
        value={scraper ?? ""}
        onChange={(e) => onChange(e.target.value as NameScraper)}
        disabled={disabled}
        className="bg-zinc-900 border border-zinc-600 rounded px-2 py-1 text-xs text-zinc-200 focus:outline-none focus:border-indigo-500"
      >
        {scrapers.map((s) => (
          <option key={s.id} value={s.id}>
            {s.label}
          </option>
        ))}
      </select>
      <button
        onClick={onSearch}
        disabled={disabled || !scraper}
        data-shot="rename-scraper-search"
        className="px-3 py-1 text-xs bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {t("rename.scraperSearch")}
      </button>
    </div>
  );
}
