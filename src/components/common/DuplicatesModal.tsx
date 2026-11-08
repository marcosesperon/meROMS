import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { findDuplicates, removeGamesWithFiles } from "../../commands";
import { askConfirm, showMessage } from "../../utils/dialogs";
import type { DuplicateGroup } from "../../commands";
import type { Game } from "../../types";
import type { TranslationKeys } from "../../i18n/en";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "./TaskProgress";

interface DuplicatesModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
  onGamesChanged?: () => void;
}

export function DuplicatesModal({
  open,
  systemPath,
  onClose,
  onGamesChanged,
}: DuplicatesModalProps) {
  const { t } = useTranslation();
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  // Buscar duplicados: con progreso y Cancelar
  const scanTask = useTask();
  const scanning = scanTask.running;
  const [removing, setRemoving] = useState(false);

  useEffect(() => {
    if (!open) {
      setGroups([]);
      setSelected(new Set());
      return;
    }
    scan();
    // Al cerrar la ventana a medias, la búsqueda se para
    return () => scanTask.cancel();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, systemPath]);

  async function scan() {
    try {
      const result = await scanTask.run((task) => findDuplicates(systemPath, task));
      // Cancelado: no hay nada que enseñar
      if (!result) return onClose();
      setGroups(result);
      setSelected(new Set());
    } catch (e) {
      console.error("Duplicate scan failed:", e);
    }
  }

  function toggleGame(path: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  async function handleRemove() {
    if (selected.size === 0) return;
    setRemoving(true);
    try {
      const paths = [...selected];
      // First the list of files, to say what will go to the trash
      const plan = await removeGamesWithFiles(systemPath, paths, true);
      const ok = await askConfirm(t("duplicates.removeConfirm", { games: plan.removed, files: plan.files.length }));
      if (!ok) return;
      const removal = await removeGamesWithFiles(systemPath, paths, false);
      if (removal.failed.length > 0) {
        await showMessage(
          t("duplicates.removeFailed", { count: removal.failed.length }) + "\n" + removal.failed.join("\n"),
          "error"
        );
      }
      // Re-scan after removal
      const result = await findDuplicates(systemPath);
      setGroups(result);
      setSelected(new Set());
      onGamesChanged?.();
    } catch (e) {
      console.error("Remove failed:", e);
      await showMessage(String(e), "error");
    } finally {
      setRemoving(false);
    }
  }

  if (!open) return null;

  const totalDuplicates = groups.reduce((sum, g) => sum + g.games.length, 0);

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[640px] max-h-[80vh] flex flex-col mx-4">
        {/* Header */}
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">
            {t("duplicates.title")}
          </h3>
          {!scanning && groups.length > 0 && (
            <p className="text-sm text-zinc-400 mt-1">
              {t("duplicates.summary", { groups: groups.length, games: totalDuplicates })}
            </p>
          )}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto">
          {scanning ? (
            <TaskProgress progress={scanTask.progress} label={t("duplicates.scanning")} onCancel={scanTask.cancel} cancelling={scanTask.cancelling} />
          ) : groups.length === 0 ? (
            <div className="flex items-center justify-center py-12 text-zinc-500 text-sm px-8 text-center">
              {t("duplicates.noDuplicates")}
            </div>
          ) : (
            <div className="divide-y divide-zinc-700/50">
              {groups.map((group, gi) => (
                <div key={gi} className="px-5 py-3">
                  <div className="flex items-center gap-2 mb-2">
                    <span className="text-xs font-medium text-amber-400 bg-amber-400/10 px-2 py-0.5 rounded">
                      {t(`duplicates.reason_${group.reason}` as TranslationKeys, { id: group.detail ?? "" })}
                    </span>
                    <span className="text-xs text-zinc-500">
                      {group.games.length} {t("header.games")}
                    </span>
                  </div>
                  <div className="space-y-1">
                    {group.games.map((game: Game) => (
                      <label
                        key={game.path}
                        className="flex items-center gap-3 px-3 py-1.5 rounded hover:bg-zinc-700/30 cursor-pointer transition-colors"
                      >
                        <input
                          type="checkbox"
                          checked={selected.has(game.path)}
                          onChange={() => toggleGame(game.path)}
                          className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
                        />
                        <div className="flex-1 min-w-0">
                          <span className="text-sm text-zinc-300 truncate block">
                            {game.name || game.path}
                          </span>
                          <span className="text-xs text-zinc-500 truncate block">
                            {game.path}
                          </span>
                        </div>
                      </label>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("cleanup.close")}
          </button>
          {groups.length > 0 && (
            <button
              onClick={handleRemove}
              disabled={selected.size === 0 || removing}
              className="px-4 py-2 text-sm bg-red-600 text-white rounded hover:bg-red-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {removing
                ? "..."
                : t("duplicates.removeSelected", { count: selected.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
