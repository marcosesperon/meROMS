import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { detectSwappedMedia, swapMedia } from "../../commands";
import type { DetectResult, MediaFileInfo, MediaProfile, SwapProposal } from "../../types";
import { resolveImageSrc } from "../../utils/images";
import { showMessage } from "../../utils/dialogs";
import { reportWithUndo } from "../../utils/undo";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "./TaskProgress";

interface MediaReviewModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
  /** Tras aplicar (para recargar la lista de juegos) */
  onDone: () => void;
  /** Open "Swap media types" (when the whole system looks the other way round) */
  onOpenSwap: () => void;
}

/**
 * «Revisar carátulas y capturas»: juegos cuya carátula y captura parecen estar al revés,
 * comparadas con lo habitual del sistema (el tamaño que más se repite en cada carpeta).
 * Se ven las dos miniaturas y se intercambian las marcadas.
 */
export function MediaReviewModal({ open, systemPath, onClose, onDone, onOpenSwap }: MediaReviewModalProps) {
  const { t } = useTranslation();
  const [items, setItems] = useState<SwapProposal[]>([]);
  const [result, setResult] = useState<DetectResult | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  // La búsqueda: con progreso y Cancelar
  const scanTask = useTask();
  const scanning = scanTask.running;
  // Intercambiar: con progreso y Cancelar (se para entre juegos; lo hecho se guarda)
  const applyTask = useTask();
  const applying = applyTask.running;

  useEffect(() => {
    if (!open) {
      setItems([]);
      setResult(null);
      setSelected(new Set());
      return;
    }
    scanTask
      .run((task) => detectSwappedMedia(systemPath, task))
      .then((detected) => {
        // Cancelado: no hay nada que enseñar
        if (!detected) return onClose();
        setResult(detected);
        setItems(detected.proposals);
        setSelected(new Set(detected.proposals.map((i) => i.game_path)));
      })
      .catch((e) => showMessage(t("mediaKinds.failed", { error: String(e) }), "error"));
    // Al cerrar la ventana a medias, la búsqueda se para
    return () => scanTask.cancel();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, systemPath]);

  if (!open) return null;

  function toggle(path: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  async function handleApply() {
    try {
      const chosen = [...selected];
      const result = await applyTask.run((task) => swapMedia(systemPath, chosen, "image", "thumbnail", task));
      if (!result) return;
      let message = t("mediaKinds.done", { swapped: result.swapped });
      if (applyTask.wasCancelled()) message = t("task.stopped", { done: result.swapped, total: chosen.length }) + "\n\n" + message;
      if (result.skipped.length > 0) {
        message += "\n\n" + t("mediaKinds.skipped", { count: result.skipped.length }) + "\n" + result.skipped.join("\n");
      }
      await reportWithUndo(message);
      onClose();
      onDone();
    } catch (e) {
      await showMessage(t("mediaKinds.failed", { error: String(e) }), "error");
    }
  }

  const sizeText = (p: MediaProfile | null) => (p ? `${p.width}×${p.height}` : t("mediaKinds.unknown"));

  // Miniatura con su tamaño debajo (o «no hay»)
  const thumb = (label: string, media: MediaFileInfo | null) => (
    <div className="w-28 shrink-0">
      <div className="h-24 bg-zinc-900 rounded flex items-center justify-center overflow-hidden">
        {media ? (
          <img src={resolveImageSrc(media.path, systemPath)} alt={label} className="max-w-full max-h-full object-contain" />
        ) : (
          <span className="text-xs text-zinc-600">{t("mediaKinds.none")}</span>
        )}
      </div>
      <p className="text-[10px] text-zinc-500 mt-1 text-center">
        {label}
        {media && ` · ${media.width}×${media.height}`}
      </p>
    </div>
  );

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[640px] max-h-[85vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("mediaKinds.reviewTitle")}</h3>
          <p className="text-sm text-zinc-400 mt-1">{t("mediaKinds.reviewDescription")}</p>
          {/* Lo habitual del sistema, que es con lo que se compara */}
          {result && (
            <p className="text-xs text-zinc-500 mt-2">
              {t("mediaKinds.usualCover")}: {sizeText(result.cover_profile)} · {t("mediaKinds.usualScreenshot")}:{" "}
              {sizeText(result.screenshot_profile)}
            </p>
          )}
          {result?.looks_swapped && (
            <div className="mt-3 px-3 py-2 rounded border border-amber-700/50 bg-amber-600/10 flex items-center justify-between gap-3">
              <p className="text-xs text-amber-300">{t("mediaKinds.looksSwapped")}</p>
              <button
                onClick={() => {
                  onClose();
                  onOpenSwap();
                }}
                className="shrink-0 px-3 py-1 text-xs bg-amber-600/30 text-amber-200 rounded hover:bg-amber-600/40"
              >
                {t("mediaKinds.swapButton")}
              </button>
            </div>
          )}
          {!scanning && items.length > 0 && (
            <div className="flex items-center gap-4 mt-2 text-xs">
              <span className="text-zinc-500">{t("mediaKinds.found", { count: items.length })}</span>
              <button onClick={() => setSelected(new Set(items.map((i) => i.game_path)))} className="text-indigo-400 hover:text-indigo-300">
                {t("rename.selectAll")}
              </button>
              <button onClick={() => setSelected(new Set())} className="text-indigo-400 hover:text-indigo-300">
                {t("rename.deselectAll")}
              </button>
            </div>
          )}
        </div>

        <div className="flex-1 overflow-y-auto">
          {applying ? (
            <TaskProgress progress={applyTask.progress} onCancel={applyTask.cancel} cancelling={applyTask.cancelling} />
          ) : scanning ? (
            <TaskProgress progress={scanTask.progress} label={t("mediaKinds.scanning")} onCancel={scanTask.cancel} cancelling={scanTask.cancelling} />
          ) : items.length === 0 ? (
            <div className="py-12 px-8 text-center text-sm text-zinc-500">{t("mediaKinds.nothing")}</div>
          ) : (
            <div className="divide-y divide-zinc-700/30">
              {items.map((item) => (
                <label key={item.game_path} className="flex items-center gap-4 px-5 py-3 cursor-pointer hover:bg-zinc-700/30">
                  <input
                    type="checkbox"
                    checked={selected.has(item.game_path)}
                    onChange={() => toggle(item.game_path)}
                    className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                  />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm text-zinc-200 truncate">{item.game_name}</p>
                    <p className="text-xs text-zinc-500 font-mono truncate">{item.game_path}</p>
                  </div>
                  {thumb(t("mediaKinds.cover"), item.cover)}
                  <span className="text-zinc-500 text-lg">⇄</span>
                  {thumb(t("mediaKinds.screenshot"), item.screenshot)}
                </label>
              ))}
            </div>
          )}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("rename.close")}
          </button>
          {items.length > 0 && (
            <button
              onClick={handleApply}
              disabled={selected.size === 0 || applying}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {applying ? "..." : t("mediaKinds.apply", { count: selected.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
