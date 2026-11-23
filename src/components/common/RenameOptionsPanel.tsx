import { useTranslation } from "../../i18n";
import type { ArticleMode, NameSource, RenameOptions, TagKeep } from "../../types";
import { nameSourceText, usableNameScrapers } from "../../utils/renameSources";
import { useAppStore } from "../../store/useAppStore";

interface RenameOptionsPanelProps {
  options: RenameOptions;
  onChange: (options: RenameOptions) => void;
  /** ROMs que el scraper aún no ha contestado (sin preguntar, paradas o sin respuesta) */
  scraperPending?: number;
  /** «Buscar» del scraper; sin él (Ajustes) no sale el botón */
  onSearch?: () => void;
  disabled: boolean;
}

const TAG_KINDS: (keyof TagKeep)[] = ["region", "language", "revision", "status", "dump", "other"];
const ARTICLES: ArticleMode[] = ["keep", "end", "front"];

/** Columna de opciones de Renombrar: fuentes en cadena, reglas, etiquetas y título */
export function RenameOptionsPanel({ options, onChange, scraperPending = 0, onSearch, disabled }: RenameOptionsPanelProps) {
  const { t } = useTranslation();
  const scraperStatus = useAppStore((s) => s.scraperStatus);
  const scrapers = usableNameScrapers(scraperStatus);
  const set = (patch: Partial<RenameOptions>) => onChange({ ...options, ...patch });

  // Las activas arriba, por prioridad; las apagadas debajo, en su orden
  const rows: { id: NameSource; on: boolean }[] = [
    ...options.sources.map((id) => ({ id, on: true })),
    ...options.disabled.map((id) => ({ id, on: false })),
  ];

  function toggle(id: NameSource) {
    if (options.sources.includes(id)) {
      set({ sources: options.sources.filter((s) => s !== id), disabled: [id, ...options.disabled] });
    } else {
      set({ sources: [...options.sources, id], disabled: options.disabled.filter((s) => s !== id) });
    }
  }

  // Subir o bajar una fuente activa (cambia la prioridad)
  function move(id: NameSource, step: -1 | 1) {
    const list = [...options.sources];
    const i = list.indexOf(id);
    const j = i + step;
    if (i < 0 || j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    set({ sources: list });
  }

  return (
    <div className="space-y-5 text-sm">
      {/* Fuentes del nombre de fichero */}
      <section>
        <h4 className="text-xs font-semibold text-zinc-300 uppercase tracking-wide mb-1">{t("rename.chainTitle")}</h4>
        <p className="text-xs text-zinc-500 mb-2">{t("rename.chainHelp")}</p>
        <div className="space-y-1">
          {rows.map(({ id, on }) => {
            const position = options.sources.indexOf(id);
            const noScraper = id === "scraper" && scrapers.length === 0;
            return (
              <div key={id} className={`rounded px-2 py-1.5 ${on ? "bg-zinc-700/50" : "bg-zinc-900/40"}`}>
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={on && !noScraper}
                    disabled={disabled || noScraper}
                    onChange={() => toggle(id)}
                    className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                  />
                  <span className={`w-4 text-xs tabular-nums ${on ? "text-indigo-300" : "text-zinc-600"}`}>
                    {on ? position + 1 : ""}
                  </span>
                  <span className={`flex-1 ${on ? "text-zinc-200" : "text-zinc-500"}`} title={t(nameSourceText(id).help)}>
                    {t(nameSourceText(id).label)}
                  </span>
                  {on && (
                    <>
                      <button
                        onClick={() => move(id, -1)}
                        disabled={disabled || position === 0}
                        title={t("rename.moveUp")}
                        className="px-1.5 text-zinc-400 hover:text-zinc-100 disabled:opacity-20"
                      >
                        ▲
                      </button>
                      <button
                        onClick={() => move(id, 1)}
                        disabled={disabled || position === options.sources.length - 1}
                        title={t("rename.moveDown")}
                        className="px-1.5 text-zinc-400 hover:text-zinc-100 disabled:opacity-20"
                      >
                        ▼
                      </button>
                    </>
                  )}
                </div>
                {/* El scraper: cuál y «Buscar» (lo único que pregunta por internet y gasta cupo) */}
                {id === "scraper" && noScraper && <p className="text-xs text-zinc-500 mt-1 ml-6">{t("rename.scraperNotConfigured")}</p>}
                {id === "scraper" && on && !noScraper && (
                  <div className="flex items-center gap-2 mt-1.5 ml-6">
                    <select
                      value={options.scraper}
                      onChange={(e) => set({ scraper: e.target.value as RenameOptions["scraper"] })}
                      disabled={disabled}
                      className="bg-zinc-900 border border-zinc-600 rounded px-2 py-0.5 text-xs text-zinc-200"
                    >
                      {scrapers.map((s) => (
                        <option key={s.id} value={s.id}>
                          {s.label}
                        </option>
                      ))}
                    </select>
                    {onSearch && (
                    <button
                      onClick={onSearch}
                      disabled={disabled || scraperPending === 0}
                      data-shot="rename-scraper-search"
                      title={t("rename.sourceScraperHelp")}
                      className="px-2 py-0.5 text-xs bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-40"
                    >
                      {t("rename.scraperSearch")}
                      {scraperPending > 0 ? ` (${scraperPending})` : ""}
                    </button>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </section>

      {/* Reglas de limpieza */}
      <section>
        <h4 className="text-xs font-semibold text-zinc-300 uppercase tracking-wide mb-1">{t("rename.cleanupTitle")}</h4>
        <label className="flex items-start gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={options.rules}
            disabled={disabled}
            onChange={() => set({ rules: !options.rules })}
            className="mt-0.5 rounded border-zinc-600 bg-zinc-700 text-indigo-600"
          />
          <span className="text-zinc-300">{t("rename.rulesToggle")}</span>
        </label>
        <p className="text-xs text-zinc-500 mt-1 ml-6">{t("rename.rulesHelp")}</p>
      </section>

      {/* Etiquetas que se quedan en el fichero */}
      <section>
        <h4 className="text-xs font-semibold text-zinc-300 uppercase tracking-wide mb-1">{t("rename.tagsTitle")}</h4>
        <div className="grid grid-cols-2 gap-x-3 gap-y-1">
          {TAG_KINDS.map((kind) => (
            <label key={kind} className="flex items-center gap-2 cursor-pointer" title={t(`rename.tagHelp_${kind}`)}>
              <input
                type="checkbox"
                checked={options.keep[kind]}
                disabled={disabled}
                onChange={() => set({ keep: { ...options.keep, [kind]: !options.keep[kind] } })}
                className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
              />
              <span className="text-zinc-300">{t(`rename.tag_${kind}`)}</span>
            </label>
          ))}
        </div>
        <p className="text-xs text-zinc-500 mt-1">{t("rename.tagsDisc")}</p>
      </section>

      {/* Título de la gamelist */}
      <section>
        <h4 className="text-xs font-semibold text-zinc-300 uppercase tracking-wide mb-1">{t("rename.titleSection")}</h4>
        <p className="text-xs text-zinc-500 mb-2">{t("rename.titleHelp")}</p>
        <select
          value={options.article}
          onChange={(e) => set({ article: e.target.value as ArticleMode })}
          disabled={disabled}
          className="w-full bg-zinc-900 border border-zinc-600 rounded px-2 py-1 text-xs text-zinc-200"
        >
          {ARTICLES.map((a) => (
            <option key={a} value={a}>
              {t(`rename.article_${a}`)}
            </option>
          ))}
        </select>
      </section>
    </div>
  );
}

