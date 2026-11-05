import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { Game, ScrapeProgress } from "../../types";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import { saveScrapedGame } from "../../hooks/useGameActions";
import { cancelScrape } from "../../commands";
import { showMessage } from "../../utils/dialogs";
import { ScrapeSearchModal } from "./ScrapeSearchModal";

interface ScrapeProgressModalProps {
  open: boolean;
  /** The batch ended and the games were reloaded (until then the gamelist isn't written yet) */
  finished: boolean;
  /** Ended by «Stop» (the rest can be continued from Scrape All) */
  stopped: boolean;
  /** Error that cut the run (what was done until then is saved) */
  error: string | null;
  systemPath: string;
  onClose: () => void;
}

export function ScrapeProgressModal({
  open,
  finished,
  stopped,
  error,
  systemPath,
  onClose,
}: ScrapeProgressModalProps) {
  const [progress, setProgress] = useState<ScrapeProgress | null>(null);
  const [log, setLog] = useState<{ name: string; path: string; status: string }[]>([]);
  // Juegos no encontrados que ya se resolvieron a mano, y el que tiene la búsqueda abierta
  const [resolved, setResolved] = useState<Set<string>>(new Set());
  const [searchGame, setSearchGame] = useState<Game | null>(null);
  // «Detener» pulsado: se espera a que acabe el juego en curso
  const [stopping, setStopping] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);
  const { t } = useTranslation();

  useEffect(() => {
    if (!open) {
      setProgress(null);
      setLog([]);
      setResolved(new Set());
      setSearchGame(null);
      setStopping(false);
      return;
    }

    const unlisten = listen<ScrapeProgress>("scrape-progress", (event) => {
      const p = event.payload;
      setProgress(p);

      if (p.status !== "scraping") {
        setLog((prev) => [...prev, { name: p.game_name, path: p.game_path, status: p.status }]);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [open]);

  // La lista tiene alto fijo: según llegan juegos, se baja sola hasta el último
  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [log]);

  if (!open) return null;

  const percent = progress
    ? Math.round((progress.current / progress.total) * 100)
    : 0;

  // Terminado de verdad = App ya tiene la respuesta y recargó los juegos (también al detener o si falla)
  const ended = finished;
  const remaining = progress ? progress.total - log.length : 0;

  async function stop() {
    setStopping(true);
    await cancelScrape().catch(() => setStopping(false));
  }

  const successCount = log.filter((l) => l.status === "success").length;
  const notFoundCount = log.filter((l) => l.status === "not_found").length;
  const errorCount = log.filter((l) => l.status === "error").length;
  // Los que no se encontraron (o fallaron): al terminar se pueden buscar a mano uno a uno
  const unresolved = log.filter((l) => l.status !== "success");

  function openSearch(path: string) {
    const game = useAppStore.getState().games.find((g) => g.path === path);
    if (game) setSearchGame(game);
  }

  async function applySearched(game: Game) {
    try {
      await saveScrapedGame(game);
      setResolved((prev) => new Set(prev).add(game.path));
    } catch (e) {
      await showMessage(t("detail.saveFailed", { error: String(e) }), "error");
    }
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[520px] max-h-[80vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">
            {!ended
              ? t("scrapeProgress.inProgress")
              : error
                ? t("scrapeProgress.failed")
                : stopped
                  ? t("scrapeProgress.stopped")
                  : t("scrapeProgress.complete")}
          </h3>
          {progress && (
            <p className="text-sm text-zinc-400 mt-1 truncate">
              {progress.current} / {progress.total}
              {!ended && progress.status === "scraping" && (
                <span className="ml-2 text-zinc-500">
                  — {progress.game_name}
                </span>
              )}
            </p>
          )}
        </div>

        <div className="px-5 py-3">
          <div className="w-full bg-zinc-700 rounded-full h-2">
            <div
              className="bg-indigo-500 h-2 rounded-full transition-all duration-300"
              style={{ width: `${percent}%` }}
            />
          </div>
        </div>

        {/* Siempre ocupa su sitio (aunque aún no haya resultados) para que la ventana no cambie de alto */}
        <div className="px-5 pb-2 flex gap-4 text-xs h-6 items-center">
          <span className="text-emerald-400">
            {successCount} {t("scrapeProgress.found")}
          </span>
          <span className="text-yellow-400">
            {notFoundCount} {t("scrapeProgress.notFound")}
          </span>
          {errorCount > 0 && (
            <span className="text-red-400">
              {errorCount} {t("scrapeProgress.errors")}
            </span>
          )}
        </div>

        <div className="px-5 pb-3">
          <div ref={logRef} className="h-60 overflow-y-auto rounded border border-zinc-700 bg-zinc-900/50 px-3 py-2 space-y-1">
            {log.map((entry, i) => (
              <div key={i} className="flex items-center gap-2 text-xs">
                <span
                  className={
                    entry.status === "success"
                      ? "text-emerald-400"
                      : entry.status === "not_found"
                        ? "text-yellow-400"
                        : "text-red-400"
                  }
                >
                  {entry.status === "success"
                    ? "✓"
                    : entry.status === "not_found"
                      ? "—"
                      : "✗"}
                </span>
                <span className="text-zinc-400 truncate">{entry.name}</span>
              </div>
            ))}
          </div>
        </div>

        {/* Al terminar: los no encontrados, cada uno con su búsqueda manual */}
        {ended && unresolved.length > 0 && (
          <div className="px-5 pb-3">
            <p className="text-xs text-zinc-400 mb-1.5">{t("scrapeProgress.unresolved", { count: unresolved.length })}</p>
            <div className="max-h-40 overflow-y-auto rounded border border-zinc-700 bg-zinc-900/50 px-3 py-2 space-y-1">
              {unresolved.map((entry) => (
                <div key={entry.path} className="flex items-center gap-2 text-xs">
                  <span className="flex-1 min-w-0 text-zinc-300 truncate">{entry.name}</span>
                  {resolved.has(entry.path) ? (
                    <span className="text-emerald-400">✓ {t("scrapeProgress.resolved")}</span>
                  ) : (
                    <button
                      onClick={() => openSearch(entry.path)}
                      className="px-2 py-0.5 rounded bg-zinc-700 text-zinc-200 hover:bg-zinc-600"
                    >
                      {t("scrapeProgress.search")}
                    </button>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Detenido o cortado: lo que falta se puede continuar desde «Scrapear todo» */}
        {ended && (error || (stopped && remaining > 0)) && (
          <p className="px-5 pb-3 text-xs text-amber-300 break-words">
            {error && <span className="block text-red-400 mb-1">{error}</span>}
            {remaining > 0 && t("scrapeProgress.canContinue", { count: remaining })}
          </p>
        )}

        {/* Mientras corre no se puede cerrar: solo detener (guarda lo hecho) */}
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-end">
          {ended ? (
            <button
              onClick={onClose}
              className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
            >
              {t("scrapeProgress.close")}
            </button>
          ) : (
            <button
              onClick={stop}
              disabled={stopping}
              className="px-4 py-2 text-sm bg-red-600/20 text-red-400 rounded hover:bg-red-600/30 transition-colors disabled:opacity-50"
            >
              {stopping ? t("scrapeProgress.stopping") : t("scrapeProgress.stop")}
            </button>
          )}
        </div>
      </div>
      {searchGame && (
        <ScrapeSearchModal
          systemPath={systemPath}
          game={searchGame}
          onClose={() => setSearchGame(null)}
          onApplied={applySearched}
        />
      )}
    </div>
  );
}
