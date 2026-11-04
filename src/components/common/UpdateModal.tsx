import { useEffect } from "react";
import { useTranslation } from "../../i18n";
import { useUpdateStore } from "../../store/useUpdateStore";
import { RELEASES_URL, openExternal } from "../../utils/links";

/** Ventana de la versión nueva: sus notas, «Actualizar ahora» con el avance y, si falla, la página de descarga */
export function UpdateModal() {
  const { t } = useTranslation();
  const open = useUpdateStore((s) => s.modalOpen);
  const update = useUpdateStore((s) => s.update);
  const phase = useUpdateStore((s) => s.phase);
  const progress = useUpdateStore((s) => s.progress);
  const error = useUpdateStore((s) => s.error);
  const install = useUpdateStore((s) => s.install);
  const closeModal = useUpdateStore((s) => s.closeModal);

  const busy = phase === "downloading" || phase === "installing";

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && closeModal();
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, closeModal]);

  if (!open || !update) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onClick={closeModal}>
      <div
        className="bg-zinc-850 border border-zinc-700 rounded-xl p-6 max-w-lg w-full mx-4 shadow-2xl flex flex-col max-h-[80vh]"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-lg font-semibold text-zinc-100">{t("update.title", { version: update.version })}</h3>
        <p className="text-sm text-zinc-500 mb-4">{t("update.current", { version: update.currentVersion })}</p>

        {/* Notas de la release, tal cual se escribieron en GitHub */}
        <div className="flex-1 min-h-0 overflow-y-auto rounded-lg bg-zinc-900 border border-zinc-800 p-4 text-sm text-zinc-300 whitespace-pre-wrap select-text">
          {update.body?.trim() || t("update.noNotes")}
        </div>

        {busy && (
          <div className="mt-4">
            <p className="text-sm text-zinc-400 mb-2">
              {phase === "installing"
                ? t("update.installing")
                : progress === null
                  ? t("update.downloading")
                  : t("update.downloadingPct", { pct: Math.round(progress * 100) })}
            </p>
            <div className="h-2 rounded-full bg-zinc-800 overflow-hidden">
              <div
                className={`h-full bg-indigo-500 transition-all ${progress === null ? "w-1/3 animate-pulse" : ""}`}
                style={progress === null ? undefined : { width: `${Math.round(progress * 100)}%` }}
              />
            </div>
          </div>
        )}

        {phase === "error" && (
          <div className="mt-4 p-3 rounded-lg bg-red-600/10 border border-red-800/60 text-sm text-red-300">
            <p className="mb-1">{t("update.failed")}</p>
            <p className="text-xs text-red-400/80 break-words select-text">{error}</p>
          </div>
        )}

        <div className="mt-5 flex items-center gap-3">
          {phase === "error" && (
            <button onClick={() => openExternal(RELEASES_URL)} className="text-sm text-indigo-400 hover:text-indigo-300">
              {t("update.openDownloads")}
            </button>
          )}
          <div className="flex-1" />
          <button
            onClick={closeModal}
            disabled={busy}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors disabled:opacity-40"
          >
            {t("update.later")}
          </button>
          <button
            onClick={install}
            disabled={busy}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-40"
          >
            {phase === "error" ? t("update.retry") : t("update.installNow")}
          </button>
        </div>
      </div>
    </div>
  );
}
