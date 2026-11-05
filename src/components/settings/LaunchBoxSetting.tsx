import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../i18n";
import { launchboxDelete, launchboxDownload, launchboxStatus, type LaunchBoxStatus } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { askConfirm, showMessage } from "../../utils/dialogs";

type Progress = { phase: "download" | "parse"; current: number; total: number };

const mb = (bytes: number) => Math.round(bytes / (1024 * 1024));

/**
 * Base de datos de LaunchBox: descargarla (~110 MB, con progreso), actualizarla o borrarla.
 * Sin descargar, LaunchBox no se usa al scrapear.
 */
export function LaunchBoxSetting() {
  const { t, locale } = useTranslation();
  const refreshScraperStatus = useAppStore((s) => s.refreshScraperStatus);
  const [status, setStatus] = useState<LaunchBoxStatus | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);

  useEffect(() => {
    launchboxStatus().then(setStatus).catch(() => setStatus(null));
  }, []);

  async function download() {
    setProgress({ phase: "download", current: 0, total: 0 });
    const unlisten = await listen<Progress>("launchbox-progress", (e) => setProgress(e.payload));
    try {
      setStatus(await launchboxDownload());
      refreshScraperStatus();
    } catch (e) {
      await showMessage(t("launchbox.failed", { error: String(e) }), "error");
    } finally {
      unlisten();
      setProgress(null);
    }
  }

  async function remove() {
    if (!(await askConfirm(t("launchbox.deleteConfirm")))) return;
    await launchboxDelete();
    setStatus(await launchboxStatus());
    refreshScraperStatus();
  }

  const percent = progress && progress.total ? Math.round((progress.current / progress.total) * 100) : 0;

  return (
    <div className="space-y-2">
      <p className="text-xs text-zinc-500">{t("launchbox.help")}</p>
      {progress ? (
        <div>
          <div className="w-full bg-zinc-700 rounded-full h-1.5">
            <div className="bg-indigo-500 h-1.5 rounded-full transition-all" style={{ width: `${percent}%` }} />
          </div>
          <p className="text-xs text-zinc-500 mt-1">
            {progress.phase === "download"
              ? t("launchbox.downloading", { current: mb(progress.current), total: mb(progress.total) })
              : t("launchbox.reading", { percent })}
          </p>
        </div>
      ) : (
        <div className="flex items-center gap-3 text-xs">
          <p className="flex-1 text-zinc-400">
            {status?.downloaded
              ? t("launchbox.current", {
                  games: status.games.toLocaleString(locale),
                  date: status.downloaded_at ? new Date(status.downloaded_at * 1000).toLocaleDateString(locale) : "?",
                })
              : t("launchbox.notDownloaded")}
          </p>
          <button onClick={download} className="px-3 py-1.5 bg-indigo-600 text-white rounded hover:bg-indigo-500">
            {status?.downloaded ? t("launchbox.update") : t("launchbox.download")}
          </button>
          {status?.downloaded && (
            <button onClick={remove} className="px-3 py-1.5 bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
              {t("launchbox.delete")}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
