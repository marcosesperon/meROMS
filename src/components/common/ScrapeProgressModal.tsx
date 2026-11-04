import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ScrapeProgress } from "../../types";
import { useTranslation } from "../../i18n";

interface ScrapeProgressModalProps {
  open: boolean;
  onClose: () => void;
}

export function ScrapeProgressModal({
  open,
  onClose,
}: ScrapeProgressModalProps) {
  const [progress, setProgress] = useState<ScrapeProgress | null>(null);
  const [log, setLog] = useState<{ name: string; status: string }[]>([]);
  const [done, setDone] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);
  const { t } = useTranslation();

  useEffect(() => {
    if (!open) {
      setProgress(null);
      setLog([]);
      setDone(false);
      return;
    }

    const unlisten = listen<ScrapeProgress>("scrape-progress", (event) => {
      const p = event.payload;
      setProgress(p);

      if (p.status !== "scraping") {
        setLog((prev) => [...prev, { name: p.game_name, status: p.status }]);
      }

      if (p.current === p.total && p.status !== "scraping") {
        setDone(true);
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

  const successCount = log.filter((l) => l.status === "success").length;
  const notFoundCount = log.filter((l) => l.status === "not_found").length;
  const errorCount = log.filter((l) => l.status === "error").length;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[520px] max-h-[80vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">
            {done ? t("scrapeProgress.complete") : t("scrapeProgress.inProgress")}
          </h3>
          {progress && (
            <p className="text-sm text-zinc-400 mt-1 truncate">
              {progress.current} / {progress.total}
              {!done && progress.status === "scraping" && (
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

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-end">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {done ? t("scrapeProgress.close") : t("scrapeProgress.running")}
          </button>
        </div>
      </div>
    </div>
  );
}
