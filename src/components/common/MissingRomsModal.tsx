import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { listMissingRoms, removeGamesWithFiles, type MissingRoms } from "../../commands";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "./TaskProgress";

interface MissingRomsModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
  /** After removing (reload the games) */
  onDone: () => void;
}

/**
 * «Sin ROM»: juegos de la gamelist cuya ROM ya no está en disco. Los marcados se quitan de la
 * gamelist y sus medios van a la papelera (mismo comando que Duplicados; la ROM no existe).
 */
export function MissingRomsModal({ open, systemPath, onClose, onDone }: MissingRomsModalProps) {
  const { t } = useTranslation();
  const [data, setData] = useState<MissingRoms | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  // Buscar los que faltan (con progreso y Cancelar)
  const scan = useTask();
  const scanning = scan.running;
  const [removing, setRemoving] = useState(false);

  useEffect(() => {
    if (!open) {
      setData(null);
      setSelected(new Set());
      return;
    }
    scan
      .run((task) => listMissingRoms(systemPath, task))
      .then((found) => {
        // Cancelado: no hay nada que enseñar
        if (!found) return onClose();
        setData(found);
        // Si faltan todas, mejor no dejarlas marcadas: casi seguro que el disco no está conectado
        setSelected(found.all_missing ? new Set() : new Set(found.games.map((g) => g.path)));
      })
      .catch((e) => showMessage(String(e), "error"));
    // Al cerrar la ventana a medias, la búsqueda se para
    return () => scan.cancel();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, systemPath]);

  if (!open) return null;
  const games = data?.games ?? [];

  function toggle(path: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  async function handleRemove() {
    const paths = [...selected];
    setRemoving(true);
    try {
      const plan = await removeGamesWithFiles(systemPath, paths, true);
      if (!(await askConfirm(t("missingRoms.confirm", { games: plan.removed, files: plan.files.length })))) return;
      const result = await removeGamesWithFiles(systemPath, paths, false);
      if (result.failed.length > 0) {
        await showMessage(t("duplicates.removeFailed", { count: result.failed.length }) + "\n" + result.failed.join("\n"), "error");
      }
      onClose();
      onDone();
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      setRemoving(false);
    }
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[600px] max-h-[80vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("missingRoms.title")}</h3>
          <p className="text-sm text-zinc-400 mt-1">{t("missingRoms.description")}</p>
          {data?.all_missing && (
            <p className="mt-3 px-3 py-2 rounded border border-amber-700/50 bg-amber-600/10 text-xs text-amber-300">
              {t("missingRoms.allMissing")}
            </p>
          )}
          {!scanning && games.length > 0 && (
            <div className="flex items-center gap-4 mt-2 text-xs">
              <span className="text-zinc-500">{t("missingRoms.found", { count: games.length })}</span>
              <button onClick={() => setSelected(new Set(games.map((g) => g.path)))} className="text-indigo-400 hover:text-indigo-300">
                {t("rename.selectAll")}
              </button>
              <button onClick={() => setSelected(new Set())} className="text-indigo-400 hover:text-indigo-300">
                {t("rename.deselectAll")}
              </button>
            </div>
          )}
        </div>

        <div className="flex-1 overflow-y-auto">
          {scanning ? (
            <TaskProgress progress={scan.progress} label={t("newRoms.scanning")} onCancel={scan.cancel} cancelling={scan.cancelling} />
          ) : games.length === 0 ? (
            <div className="py-12 px-8 text-center text-sm text-zinc-500">{t("missingRoms.nothing")}</div>
          ) : (
            <div className="divide-y divide-zinc-700/30">
              {games.map((g) => (
                <label key={g.path} className="flex items-center gap-3 px-5 py-2 cursor-pointer hover:bg-zinc-700/30">
                  <input
                    type="checkbox"
                    checked={selected.has(g.path)}
                    onChange={() => toggle(g.path)}
                    className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
                  />
                  <div className="min-w-0">
                    <p className="text-sm text-zinc-200 truncate">{g.name}</p>
                    <p className="text-xs text-zinc-500 font-mono truncate">{g.path}</p>
                  </div>
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
          {games.length > 0 && (
            <button
              onClick={handleRemove}
              disabled={selected.size === 0 || removing}
              className="px-4 py-2 text-sm bg-red-600 text-white rounded hover:bg-red-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {removing ? "..." : t("missingRoms.remove", { count: selected.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
