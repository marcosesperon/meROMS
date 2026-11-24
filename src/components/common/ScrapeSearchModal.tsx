import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../../i18n";
import { applyScrape, scrapeThumbnail, searchScrape } from "../../commands";
import type { Game, ScrapeCandidate } from "../../types";
import { showMessage } from "../../utils/dialogs";
import { useAppStore } from "../../store/useAppStore";
import { isArcadeSystem } from "../../utils/arcade";
import { SearchIcon } from "./Icons";

interface ScrapeSearchModalProps {
  systemPath: string;
  game: Game;
  onClose: () => void;
  /** El juego con lo que trajo el resultado elegido (sin guardar: lo decide quien abre la ventana) */
  onApplied: (game: Game) => void | Promise<void>;
}

/** Texto de búsqueda inicial: el título o, si no hay, el nombre del fichero sin extensión ni etiquetas */
function initialQuery(game: Game): string {
  if (game.name?.trim()) return game.name.trim();
  const file = game.path.split(/[\\/]/).pop() ?? game.path;
  return file
    .replace(/\.[^.]+$/, "") // extensión
    .replace(/[([][^)\]]*[)\]]/g, " ") // (USA), [!], (Rev 1)…
    .replace(/[_.]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

const SOURCE_LABEL: Record<ScrapeCandidate["source"], string> = {
  screenscraper: "ScreenScraper",
  thegamesdb: "TheGamesDB",
  launchbox: "LaunchBox",
  arcadeitalia: "ArcadeItalia",
};

const candidateKey = (c: ScrapeCandidate) => `${c.source}:${c.id}`;

/** Segundos desde que algo empezó (se pone a 0 cuando `running` vuelve a ser verdad) */
function useElapsed(running: boolean): number {
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    if (!running) return;
    setSeconds(0);
    const start = Date.now();
    const timer = setInterval(() => setSeconds(Math.floor((Date.now() - start) / 1000)), 1000);
    return () => clearInterval(timer);
  }, [running]);
  return seconds;
}

/** Barra fina que se mueve sin fin (algo está en marcha y no se sabe cuánto falta) */
function MovingBar() {
  return (
    <div className="h-0.5 w-full overflow-hidden bg-zinc-700/40">
      <div className="h-full w-1/3 bg-indigo-500" style={{ animation: "task-slide 1.1s ease-in-out infinite" }} />
    </div>
  );
}

/**
 * Mientras se espera al scraper: qué fuentes se están consultando (con un punto que late), los
 * segundos que lleva y unas tarjetas «fantasma» con la forma de los resultados que van a llegar.
 * También al descargar el elegido (`applying`), con su propio texto.
 */
function Waiting({ sources, applying }: { sources: string[]; applying: string | null }) {
  const { t } = useTranslation();
  const seconds = useElapsed(true);
  return (
    <div className="px-2 py-2" role="status" aria-live="polite">
      <div className="flex flex-wrap items-center gap-2 px-1 pb-3">
        <span className="text-sm text-zinc-300">{applying ? t("scrapeSearch.downloading", { name: applying }) : t("scrapeSearch.asking")}</span>
        {!applying &&
          sources.map((s, i) => (
            <span key={s} className="flex items-center gap-1.5 text-[11px] px-2 py-0.5 rounded-full bg-zinc-700/60 text-zinc-300">
              <span className="w-1.5 h-1.5 rounded-full bg-indigo-400 animate-pulse" style={{ animationDelay: `${i * 200}ms` }} />
              {s}
            </span>
          ))}
        {seconds >= 2 && <span className="ml-auto text-[11px] text-zinc-500 tabular-nums">{t("scrapeSearch.seconds", { count: seconds })}</span>}
      </div>
      <div className="space-y-1">
        {[0, 1, 2, 3, 4].map((i) => (
          <div key={i} className="flex items-center gap-3 p-2 rounded" style={{ opacity: 1 - i * 0.16 }}>
            <div className="shimmer w-12 h-16 shrink-0 rounded bg-zinc-700/50" />
            <div className="flex-1 space-y-2">
              <div className="shimmer h-3.5 rounded bg-zinc-700/50" style={{ width: `${70 - i * 8}%` }} />
              <div className="shimmer h-2.5 rounded bg-zinc-700/40" style={{ width: `${45 - i * 4}%` }} />
              <div className="shimmer h-3 w-20 rounded bg-zinc-700/40" />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

/**
 * Búsqueda manual para scrapear un juego: busca un texto en ScreenScraper y TheGamesDB,
 * enseña los resultados como fichas y, al elegir uno, descarga sus datos y su media.
 */
export function ScrapeSearchModal({ systemPath, game, onClose, onApplied }: ScrapeSearchModalProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState(() => initialQuery(game));
  const [allSystems, setAllSystems] = useState(false);
  const [candidates, setCandidates] = useState<ScrapeCandidate[]>([]);
  const [sourceErrors, setSourceErrors] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [searching, setSearching] = useState(false);
  const [applying, setApplying] = useState(false);
  const [searched, setSearched] = useState(false);
  // Cada búsqueda lleva un número: así las miniaturas de una búsqueda vieja no pisan la nueva
  const searchRun = useRef(0);

  async function runSearch(text: string, everywhere: boolean) {
    const run = ++searchRun.current;
    setSearching(true);
    setSelected(null);
    try {
      const result = await searchScrape(systemPath, game.path, text, everywhere);
      if (run !== searchRun.current) return;
      setCandidates(result.candidates);
      setSourceErrors(result.errors);
      // Sin ningún resultado (y sin fallos) el backend lo ha marcado «no encontrado»: a la vista
      if (result.candidates.length === 0 && result.errors.length === 0) useAppStore.getState().refreshScrapeMisses();
      // La que coincide con la ROM sale marcada; si no, la primera
      const first = result.candidates.find((c) => c.rom_match) ?? result.candidates[0];
      setSelected(first ? candidateKey(first) : null);
      loadThumbnails(run, result.candidates);
    } catch (e) {
      if (run !== searchRun.current) return;
      setCandidates([]);
      setSourceErrors([String(e)]);
    } finally {
      if (run === searchRun.current) {
        setSearching(false);
        setSearched(true);
      }
    }
  }

  /** Las carátulas de ScreenScraper se piden de una en una (las cuentas gratuitas solo admiten una petición a la vez) */
  async function loadThumbnails(run: number, list: ScrapeCandidate[]) {
    for (const c of list) {
      if (c.source !== "screenscraper" || c.thumbnail) continue;
      const thumbnail = await scrapeThumbnail(c.id).catch(() => null);
      if (run !== searchRun.current) return;
      if (thumbnail) {
        setCandidates((prev) => prev.map((p) => (candidateKey(p) === candidateKey(c) ? { ...p, thumbnail } : p)));
      }
    }
  }

  // Al abrirse busca sola con el título
  useEffect(() => {
    runSearch(query, allSystems);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Esc cierra (salvo mientras descarga)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || applying) return;
      e.stopPropagation();
      onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [applying, onClose]);

  async function apply(candidate: ScrapeCandidate) {
    if (applying) return;
    setApplying(true);
    try {
      const result = await applyScrape(systemPath, game.path, candidate.source, candidate.id);
      // Encontrado: el backend ha quitado la marca de «no encontrado»
      useAppStore.getState().refreshScrapeMisses();
      await onApplied(result.game);
      onClose();
    } catch (e) {
      await showMessage(t("detail.scrapeFailed", { error: String(e) }), "error");
    } finally {
      setApplying(false);
    }
  }

  const selectedCandidate = candidates.find((c) => candidateKey(c) === selected) ?? null;
  const busy = searching || applying;
  // Las fuentes que se consultan (las que hay configuradas; ArcadeItalia, solo en arcade)
  const scraperStatus = useAppStore((s) => s.scraperStatus);
  const systemName = systemPath.split(/[\\/]/).pop() ?? "";
  const sources = [
    scraperStatus.screenscraper && "ScreenScraper",
    isArcadeSystem(systemName) && "ArcadeItalia",
    scraperStatus.launchbox && "LaunchBox",
    scraperStatus.tgdb && "TheGamesDB",
  ].filter(Boolean) as string[];

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-[60]" onClick={() => !applying && onClose()}>
      <div
        className="bg-zinc-800 border border-zinc-600 rounded-lg w-[640px] max-w-[calc(100vw-2rem)] h-[80vh] flex flex-col mx-4 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-zinc-700 space-y-3">
          <h3 className="text-base font-semibold text-zinc-100 truncate">
            {t("scrapeSearch.title", { name: game.name || game.path })}
          </h3>
          <form
            className="flex items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (!busy) runSearch(query, allSystems);
            }}
          >
            <input
              autoFocus
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("scrapeSearch.placeholder")}
              className="flex-1 min-w-0 px-3 py-1.5 text-sm bg-zinc-900 border border-zinc-600 rounded text-zinc-100 focus:outline-none focus:border-indigo-500"
            />
            <button
              type="submit"
              disabled={busy || !query.trim()}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50"
            >
              <SearchIcon size={14} />
              {searching ? t("scrapeSearch.searching") : t("scrapeSearch.search")}
            </button>
          </form>
          <label className="flex items-center gap-2 text-xs text-zinc-400">
            <input
              type="checkbox"
              checked={allSystems}
              disabled={busy}
              onChange={(e) => {
                // Cambiar el alcance repite la búsqueda en el momento
                setAllSystems(e.target.checked);
                runSearch(query, e.target.checked);
              }}
              className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
            />
            {t("scrapeSearch.allSystems")}
          </label>
        </div>

        {sourceErrors.length > 0 && (
          <div className="px-5 py-2 text-xs text-amber-400 border-b border-zinc-700 break-words">
            {sourceErrors.map((e, i) => (
              <p key={i}>{t("scrapeSearch.sourceFailed", { error: e })}</p>
            ))}
          </div>
        )}

        {/* Buscando otra vez con resultados en pantalla: una barra que se mueve encima de la lista */}
        {searching && candidates.length > 0 && <MovingBar />}
        <div className={`flex-1 overflow-y-auto px-3 py-3 space-y-1 ${searching && candidates.length > 0 ? "opacity-50" : ""}`}>
          {/* Primera búsqueda (o descargando el elegido): fuentes, segundos y tarjetas fantasma */}
          {((searching && candidates.length === 0) || applying) && (
            <Waiting sources={sources} applying={applying ? selectedCandidate?.name ?? "" : null} />
          )}
          {searched && !searching && candidates.length === 0 && (
            <p className="px-2 py-8 text-sm text-zinc-500 text-center">{t("scrapeSearch.noResults")}</p>
          )}
          {!applying && candidates.map((c) => {
            const key = candidateKey(c);
            const isSelected = key === selected;
            const details = [c.system, c.year, c.developer].filter(Boolean).join(" · ");
            return (
              <button
                key={key}
                type="button"
                onClick={() => setSelected(key)}
                onDoubleClick={() => apply(c)}
                disabled={applying}
                className={`w-full flex items-center gap-3 p-2 rounded text-left border transition-colors ${
                  isSelected ? "bg-indigo-600/20 border-indigo-500" : "border-transparent hover:bg-zinc-700/60"
                }`}
              >
                <div className="w-12 h-16 shrink-0 rounded bg-zinc-900 overflow-hidden flex items-center justify-center">
                  {c.thumbnail && <img src={c.thumbnail} alt="" className="max-w-full max-h-full object-contain" />}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="text-sm text-zinc-100 truncate">{c.name}</p>
                  {details && <p className="text-xs text-zinc-400 truncate">{details}</p>}
                  <div className="mt-1 flex items-center gap-1.5">
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-zinc-700 text-zinc-300">{SOURCE_LABEL[c.source]}</span>
                    {c.rom_match && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-600/30 text-emerald-300">
                        {t("scrapeSearch.romMatch")}
                      </span>
                    )}
                  </div>
                </div>
              </button>
            );
          })}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex items-center gap-3">
          <p className="flex-1 text-xs text-zinc-500">{t("scrapeSearch.hint")}</p>
          <button
            onClick={onClose}
            disabled={applying}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-50"
          >
            {t("scrapeSearch.cancel")}
          </button>
          <button
            onClick={() => selectedCandidate && apply(selectedCandidate)}
            disabled={!selectedCandidate || busy}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50"
          >
            {applying ? t("scrapeSearch.applying") : t("scrapeSearch.use")}
          </button>
        </div>
      </div>
    </div>
  );
}
