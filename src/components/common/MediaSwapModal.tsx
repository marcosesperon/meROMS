import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { swapMedia } from "../../commands";
import { SWAPPABLE_MEDIA, type Game } from "../../types";
import { askConfirm, showMessage } from "../../utils/dialogs";

interface MediaSwapModalProps {
  open: boolean;
  systemPath: string;
  /** Todos los juegos del sistema (se intercambia en todos) */
  games: Game[];
  onClose: () => void;
  onDone: () => void;
}

/** «Intercambiar tipos de medio»: dos tipos elegidos a mano, en todos los juegos del sistema. */
export function MediaSwapModal({ open, systemPath, games, onClose, onDone }: MediaSwapModalProps) {
  const { t } = useTranslation();
  const [fieldA, setFieldA] = useState<string>("image");
  const [fieldB, setFieldB] = useState<string>("thumbnail");
  const [applying, setApplying] = useState(false);

  useEffect(() => {
    if (open) {
      setFieldA("image");
      setFieldB("thumbnail");
    }
  }, [open]);

  if (!open) return null;

  const labelOf = (field: string) => t(SWAPPABLE_MEDIA.find((m) => m.field === field)!.labelKey);

  async function handleApply() {
    const ok = await askConfirm(t("mediaKinds.swapConfirm", { a: labelOf(fieldA), b: labelOf(fieldB), count: games.length }));
    if (!ok) return;
    setApplying(true);
    try {
      const result = await swapMedia(systemPath, games.map((g) => g.path), fieldA, fieldB);
      let message = t("mediaKinds.done", { swapped: result.swapped });
      if (result.skipped.length > 0) {
        message += "\n\n" + t("mediaKinds.skipped", { count: result.skipped.length }) + "\n" + result.skipped.join("\n");
      }
      await showMessage(message);
      onClose();
      onDone();
    } catch (e) {
      await showMessage(t("mediaKinds.failed", { error: String(e) }), "error");
    } finally {
      setApplying(false);
    }
  }

  const select = (value: string, onChange: (v: string) => void) => (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="flex-1 bg-zinc-900 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
    >
      {SWAPPABLE_MEDIA.map((m) => (
        <option key={m.field} value={m.field}>
          {t(m.labelKey)}
        </option>
      ))}
    </select>
  );

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[480px] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("mediaKinds.swapTitle")}</h3>
          <p className="text-sm text-zinc-400 mt-1">{t("mediaKinds.swapDescription")}</p>
        </div>
        <div className="px-5 py-5 flex items-center gap-3">
          {select(fieldA, setFieldA)}
          <span className="text-zinc-400 text-lg">⇄</span>
          {select(fieldB, setFieldB)}
        </div>
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("rename.close")}
          </button>
          <button
            onClick={handleApply}
            disabled={fieldA === fieldB || applying || games.length === 0}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {applying ? "..." : t("mediaKinds.swapApply")}
          </button>
        </div>
      </div>
    </div>
  );
}
