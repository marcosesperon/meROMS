import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "../../i18n";
import { applyMediaNormalization, planMediaNormalization } from "../../commands";
import type { NormalizeItem } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { getActiveProfile } from "../../utils/profiles";
import { showMessage } from "../../utils/dialogs";
import { reportWithUndo } from "../../utils/undo";

interface NormalizeMediaModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
  /** Solo este juego (desde el detalle del juego); sin él, todo el sistema */
  gamePath?: string;
  gameName?: string;
  /** Se llama tras aplicar en lugar de recargar la lista (el detalle recarga y reselecciona) */
  onDone?: () => void;
}

// Clave única de cada cambio propuesto (un juego puede tener varios campos)
const itemKey = (item: NormalizeItem) => `${item.game_path}|${item.field}`;

/**
 * Normalizador de medios: enseña qué ficheros se van a mover a `<carpeta>/<nombre ROM>.<ext>`
 * y deja elegir cuáles aplicar. Los conflictos (destino ocupado por otro fichero) se enseñan
 * pero no se pueden marcar. Con perfil ES-DE sirve para recuperar los medios que se quedaron
 * en la carpeta de ROMs con la estructura clásica.
 */
export function NormalizeMediaModal({ open, systemPath, onClose, gamePath, gameName, onDone }: NormalizeMediaModalProps) {
  const { t } = useTranslation();
  const loadGames = useAppStore((s) => s.loadGames);
  const isEsde = useAppStore((s) => getActiveProfile(s.settings)?.kind === "esde");
  const [items, setItems] = useState<NormalizeItem[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scanning, setScanning] = useState(false);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setItems([]);
      setSelected(new Set());
      setError(null);
      return;
    }
    scan();
  }, [open, systemPath, gamePath]);

  async function scan() {
    setScanning(true);
    setError(null);
    try {
      const plan = await planMediaNormalization(systemPath, gamePath);
      setItems(plan);
      setSelected(new Set(plan.filter((i) => i.status !== "conflict").map(itemKey)));
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }

  const applicable = useMemo(() => items.filter((i) => i.status !== "conflict"), [items]);
  const conflicts = items.length - applicable.length;

  function toggle(key: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  function toggleAll() {
    setSelected(selected.size === applicable.length ? new Set() : new Set(applicable.map(itemKey)));
  }

  async function handleApply() {
    setApplying(true);
    try {
      const chosen = applicable.filter((i) => selected.has(itemKey(i)));
      const result = await applyMediaNormalization(systemPath, chosen);
      if (onDone) onDone();
      else loadGames(systemPath);
      let message = t("normalize.done", { moved: result.moved, linked: result.linked });
      if (result.skipped.length > 0) {
        message += "\n\n" + t("normalize.skipped", { count: result.skipped.length }) + "\n" + result.skipped.join("\n");
      }
      await reportWithUndo(message);
      onClose();
    } catch (e) {
      await showMessage(t("normalize.failed", { error: String(e) }), "error");
    } finally {
      setApplying(false);
    }
  }

  if (!open) return null;

  const statusStyle: Record<NormalizeItem["status"], string> = {
    move: "bg-indigo-600/20 text-indigo-300",
    link: "bg-emerald-600/20 text-emerald-300",
    conflict: "bg-amber-600/20 text-amber-300",
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[860px] max-h-[80vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">
            {gamePath ? t("gameTools.normalizeTitle", { name: gameName || gamePath }) : t("normalize.title")}
          </h3>
          <p className="text-sm text-zinc-400 mt-1">{t(isEsde ? "normalize.descriptionEsde" : "normalize.description")}</p>
          {!scanning && items.length > 0 && (
            <p className="text-xs text-zinc-500 mt-2">
              {t("normalize.found", { count: applicable.length })}
              {conflicts > 0 && ` · ${t("normalize.conflicts", { count: conflicts })}`}
            </p>
          )}
        </div>

        <div className="flex-1 overflow-y-auto">
          {scanning ? (
            <div className="flex items-center justify-center py-12 text-zinc-500">{t("normalize.scanning")}</div>
          ) : error ? (
            <div className="py-12 px-8 text-center text-sm text-red-400">{error}</div>
          ) : items.length === 0 ? (
            <div className="py-12 px-8 text-center text-sm text-zinc-500">{t(isEsde ? "normalize.nothingEsde" : "normalize.nothing")}</div>
          ) : (
            <>
              <div className="px-5 py-2 border-b border-zinc-700/50">
                <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={selected.size === applicable.length && applicable.length > 0}
                    onChange={toggleAll}
                    className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                  />
                  {selected.size === applicable.length ? t("rename.deselectAll") : t("rename.selectAll")}
                </label>
              </div>
              <div className="divide-y divide-zinc-700/30">
                {items.map((item) => {
                  const key = itemKey(item);
                  const isConflict = item.status === "conflict";
                  return (
                    <label
                      key={key}
                      className={`grid grid-cols-[auto_1fr_auto] gap-3 px-5 py-2 items-center ${
                        isConflict ? "opacity-60" : "cursor-pointer"
                      }`}
                    >
                      <input
                        type="checkbox"
                        checked={selected.has(key)}
                        disabled={isConflict}
                        onChange={() => toggle(key)}
                        className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 disabled:opacity-30"
                      />
                      <div className="min-w-0">
                        <p className="text-sm text-zinc-200 truncate">
                          {item.game_name} <span className="text-zinc-500">· {item.field}</span>
                        </p>
                        <p className="text-xs text-zinc-500 font-mono truncate" title={`${item.from_relative} → ${item.to_relative}`}>
                          {item.from_relative === item.to_relative
                            ? item.to_relative
                            : `${item.from_relative} → ${item.to_relative}`}
                        </p>
                      </div>
                      <span className={`text-[10px] px-1.5 py-0.5 rounded ${statusStyle[item.status]}`}>
                        {t(`normalize.status_${item.status}` as const)}
                      </span>
                    </label>
                  );
                })}
              </div>
            </>
          )}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("rename.close")}
          </button>
          {applicable.length > 0 && (
            <button
              onClick={handleApply}
              disabled={selected.size === 0 || applying}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {applying ? "..." : t("normalize.apply", { count: selected.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
