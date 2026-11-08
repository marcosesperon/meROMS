import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import type { TaskProgressInfo } from "../../hooks/useTask";

interface TaskProgressProps {
  /** Lo último que ha avisado el proceso (null = aún nada) */
  progress: TaskProgressInfo | null;
  /** Texto cuando el proceso no ha dicho todavía en qué fase está */
  label?: string;
  /** Sin él no sale el botón Cancelar */
  onCancel?: () => void;
  cancelling?: boolean;
  /** Más apretado (dentro de una lista o de un botón, no en el centro de una ventana) */
  compact?: boolean;
}

/**
 * Progreso de un proceso largo dentro de la ventana que lo lanzó: fase, barra, «N de M», el
 * fichero o juego en curso y Cancelar. Sin total (aún no se sabe cuántos hay) la barra se mueve
 * sola, para que se vea que la app sigue trabajando.
 */
export function TaskProgress({ progress, label, onCancel, cancelling, compact }: TaskProgressProps) {
  const { t } = useTranslation();
  const phaseKey = progress?.phase ? (`task.phase.${progress.phase}` as TranslationKeys) : null;
  const phaseText = phaseKey ? t(phaseKey) : null;
  // Una clave sin traducir vuelve tal cual: entonces mejor el texto general
  const title = phaseText && phaseText !== phaseKey ? phaseText : label ?? t("task.working");
  const known = !!progress && progress.total > 0;
  const percent = known ? Math.min(100, (progress!.current / progress!.total) * 100) : 0;

  return (
    <div className={`flex flex-col gap-2 w-full ${compact ? "" : "max-w-md mx-auto py-8 px-4"}`} role="status" aria-live="polite">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-sm text-zinc-200">{cancelling ? t("task.cancelling") : title}</span>
        {known && (
          <span className="text-xs text-zinc-500 tabular-nums shrink-0">
            {t("task.count", { current: progress!.current, total: progress!.total })}
          </span>
        )}
      </div>
      <div className="h-1.5 w-full rounded-full bg-zinc-800 overflow-hidden">
        {known ? (
          <div className="h-full rounded-full bg-indigo-500 transition-[width] duration-150" style={{ width: `${percent}%` }} />
        ) : (
          // Sin total: un trozo que va y viene
          <div className="h-full w-1/3 rounded-full bg-indigo-500/80 animate-[task-slide_1.2s_ease-in-out_infinite]" />
        )}
      </div>
      <p className="text-[11px] text-zinc-500 font-mono truncate min-h-[1.25em]" title={progress?.item}>
        {progress?.item || " "}
      </p>
      {onCancel && (
        <div className="flex justify-end">
          <button
            onClick={onCancel}
            disabled={cancelling}
            className="px-3 py-1.5 text-xs rounded border border-zinc-600 text-zinc-300 hover:bg-zinc-700 disabled:opacity-50"
          >
            {cancelling ? t("task.cancelling") : t("task.cancel")}
          </button>
        </div>
      )}
    </div>
  );
}
