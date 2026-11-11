import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { raCheckSystem, type RaCheckSummary } from "../../commands";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "./TaskProgress";
import { TrophyIcon } from "./Icons";

/**
 * Herramientas → «Comprobar logros»: calcula la huella de RetroAchievements de cada ROM del
 * sistema y la cruza con sus juegos y con tu progreso. El resultado se guarda y da la insignia de
 * la lista y el filtro. Volver a pasarlo solo recalcula las ROMs que han cambiado.
 */
export function RaCheckModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const system = useAppStore((s) => s.selectedSystem);
  const hasAccount = useAppStore((s) => !!s.settings.ra_user && !!s.settings.ra_apikey);
  const refreshRaMarks = useAppStore((s) => s.refreshRaMarks);
  const task = useTask();
  const [summary, setSummary] = useState<RaCheckSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  if (!system) return null;

  const start = async () => {
    setError(null);
    setSummary(null);
    try {
      const result = await task.run((id) => raCheckSystem(system.path, id));
      if (result) setSummary(result);
      refreshRaMarks();
    } catch (e) {
      setError(String(e));
    }
  };

  const row = (label: string, value: number, cls: string) => (
    <div className="flex items-center justify-between py-1 border-t border-white/5 text-sm">
      <span className={cls}>{label}</span>
      <span className="font-semibold text-zinc-100 tabular-nums">{value}</span>
    </div>
  );

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onMouseDown={() => !task.running && onClose()}>
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[460px] max-w-[calc(100vw-2rem)] mx-4" onMouseDown={(e) => e.stopPropagation()}>
        <div className="px-5 py-4 border-b border-zinc-700 flex items-center gap-2">
          <TrophyIcon size={16} className="text-amber-300" />
          <h3 className="text-base font-semibold text-zinc-100">{t("ra.checkTitle", { system: system.full_name })}</h3>
        </div>
        <div className="px-5 py-4 space-y-3">
          <p className="text-xs text-zinc-400">{t("ra.checkHelp")}</p>
          {!hasAccount && <p className="text-sm text-amber-300">{t("ra.noAccount")}</p>}
          {task.running && <TaskProgress progress={task.progress} label={t("ra.checking")} onCancel={task.cancel} cancelling={task.cancelling} />}
          {error && <p className="text-sm text-red-300 break-words">{error}</p>}
          {summary && (
            <div data-ra-summary>
              {row(t("ra.sumCompatible"), summary.compatible, "text-amber-300")}
              {row(t("ra.sumOther"), summary.other, "text-zinc-300")}
              {row(t("ra.sumNone"), summary.none, "text-zinc-400")}
              {summary.unsupported > 0 && row(t("ra.sumUnsupported"), summary.unsupported, "text-zinc-500")}
            </div>
          )}
        </div>
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-end gap-2">
          <button onClick={onClose} disabled={task.running} className="px-4 py-2 text-sm text-zinc-300 rounded border border-zinc-600 hover:bg-zinc-700 disabled:opacity-40">
            {t("play.close")}
          </button>
          <button
            data-ra-start
            onClick={start}
            disabled={task.running || !hasAccount}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-40"
          >
            {summary ? t("ra.checkAgain") : t("ra.checkStart")}
          </button>
        </div>
      </div>
    </div>
  );
}
