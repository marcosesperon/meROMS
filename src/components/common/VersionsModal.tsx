import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { findDuplicates, removeGamesWithFiles, type DuplicateGroup } from "../../commands";
import { useTask } from "../../hooks/useTask";
import { useAppStore } from "../../store/useAppStore";
import type { VersionsAction } from "../../types";
import { hasSetAsideDir, keptVersion, removeVersions, versionsToRemove } from "../../utils/collectionActions";
import { collectionRulesFrom } from "../../utils/collectionRules";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { reportWithUndo } from "../../utils/undo";
import { TaskProgress } from "./TaskProgress";
import { useBlockClose } from "../../hooks/useBlockClose";
import { CloseButton } from "./CloseButton";
import { ProcessingLabel } from "./ProcessingLabel";

interface VersionsModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
  /** Tras quitar versiones (recargar juegos y contadores) */
  onDone: () => void;
  /** Solo los grupos de estos juegos («Revisar este juego»); sin él, todos */
  onlyPaths?: string[];
}

const ACTIONS: VersionsAction[] = ["set_aside", "hide", "trash"];

/**
 * «Versiones y duplicados» (antes, Duplicados y Mejor versión por separado): los juegos que son el
 * mismo (mismo ID de ScreenScraper, mismo fichero, mismo juego del DAT o mismo nombre sin
 * etiquetas), cuál se queda y por qué, y qué hacer con los demás según «Mi colección»: apartarlos
 * a la carpeta de apartados (de serie), ocultarlos o mandarlos a la papelera. Al abrir ya salen
 * marcados los que sobran (sin los favoritos ni los jugados, si la regla lo pide).
 */
export function VersionsModal({ open, systemPath, onClose, onDone, onlyPaths }: VersionsModalProps) {
  const { t } = useTranslation();
  const settings = useAppStore((s) => s.settings);
  const rules = collectionRulesFrom(settings.collection_rules);
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [action, setAction] = useState<VersionsAction>(rules.versions_action);
  const scanTask = useTask();
  const applyTask = useTask();
  // Mientras se apartan, ocultan o borran no se puede salir de la ventana
  useBlockClose(applyTask.running);
  const [preparing, setPreparing] = useState(false);
  // Borrar de verdad en lugar de mandar a la papelera; desmarcado cada vez que se abre
  const [permanent, setPermanent] = useState(false);
  const deleting = action === "trash" && permanent;
  const canSetAside = hasSetAsideDir();

  useEffect(() => {
    if (!open) {
      setGroups([]);
      setSelected(new Set());
      setPermanent(false);
      return;
    }
    // La acción de «Mi colección»; apartar sin carpeta no se puede: entonces, ocultar
    setAction(rules.versions_action === "set_aside" && !canSetAside ? "hide" : rules.versions_action);
    scan();
    return () => scanTask.cancel();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, systemPath]);

  async function scan() {
    try {
      const found = await scanTask.run((task) => findDuplicates(systemPath, task));
      if (!found) return onClose();
      const only = onlyPaths ? new Set(onlyPaths) : null;
      const list = only ? found.filter((g) => g.games.some((x) => only.has(x.path))) : found;
      setGroups(list);
      setSelected(new Set(versionsToRemove(list, rules.protect_played)));
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  /** Marca o desmarca el juego con todos los discos de su versión (se van o se quedan juntos) */
  function toggle(group: DuplicateGroup, index: number) {
    const path = group.games[index].path;
    const together = group.games.filter((_, i) => group.versions[i] === group.versions[index]).map((g) => g.path);
    setSelected((prev) => {
      const next = new Set(prev);
      const on = !next.has(path);
      for (const p of together) {
        if (on) next.add(p);
        else next.delete(p);
      }
      return next;
    });
  }

  // Por qué se queda: los motivos de la mejor versión y los dos propios (más medios, más datos)
  function keepReason(group: DuplicateGroup): string {
    const key = group.keep_reason === "media" || group.keep_reason === "data" ? `duplicates.keep_${group.keep_reason}` : `best.reason_${group.keep_reason}`;
    return t("duplicates.keeps", { reason: t(key as TranslationKeys) });
  }

  async function apply() {
    if (selected.size === 0 || preparing) return;
    const paths = [...selected];
    setPreparing(true);
    let ok = false;
    try {
      // A la papelera: antes se dice cuántos ficheros se van
      if (action === "trash") {
        const plan = await removeGamesWithFiles(systemPath, paths, true);
        const key = permanent ? "versions.deleteConfirm" : "duplicates.removeConfirm";
        ok = await askConfirm(t(key, { games: plan.removed, files: plan.files.length }));
      } else {
        ok = await askConfirm(t(`versions.confirm_${action}` as TranslationKeys, { count: paths.length }));
      }
    } finally {
      setPreparing(false);
    }
    if (!ok) return;
    try {
      const outcome = await applyTask.run((task) => removeVersions(systemPath, paths, action, task, deleting));
      if (!outcome) return;
      const lines = [outcome.line];
      if (applyTask.wasCancelled()) lines.unshift(t("task.stopped", { done: outcome.done, total: paths.length }), "");
      if (outcome.failed.length) lines.push("", t("duplicates.removeFailed", { count: outcome.failed.length }), ...outcome.failed);
      onDone();
      await reportWithUndo(lines.join("\n"));
      await scan();
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  if (!open) return null;
  const scanning = scanTask.running;
  const total = groups.reduce((n, g) => n + g.games.length, 0);

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[760px] max-w-[calc(100vw-2rem)] h-[85vh] flex flex-col mx-4" data-shot="versions-modal">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("versions.title")}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("versions.help")}</p>
          {!scanning && groups.length > 0 && (
            <div className="flex items-center gap-4 mt-2 text-xs">
              <span className="text-zinc-400">{t("versions.summary", { groups: groups.length, games: total, marked: selected.size })}</span>
              <button onClick={() => setSelected(new Set(versionsToRemove(groups, rules.protect_played)))} title={t("duplicates.autoMarkHint")} className="text-indigo-400 hover:text-indigo-300">
                {t("duplicates.autoMark")}
              </button>
              <button onClick={() => setSelected(new Set())} className="text-indigo-400 hover:text-indigo-300">
                {t("rename.deselectAll")}
              </button>
            </div>
          )}
        </div>

        <div className="flex-1 overflow-y-auto">
          {applyTask.running ? (
            <TaskProgress progress={applyTask.progress} onCancel={applyTask.cancel} cancelling={applyTask.cancelling} />
          ) : scanning ? (
            <TaskProgress progress={scanTask.progress} label={t("duplicates.scanning")} onCancel={scanTask.cancel} cancelling={scanTask.cancelling} />
          ) : groups.length === 0 ? (
            <p className="py-12 px-8 text-center text-sm text-zinc-500">{t("versions.none")}</p>
          ) : (
            <div className="divide-y divide-zinc-700/50">
              {groups.map((group, gi) => (
                <div key={gi} className="px-5 py-3">
                  <div className="flex items-center gap-2 mb-2">
                    <span className="text-xs font-medium text-amber-400 bg-amber-400/10 px-2 py-0.5 rounded">
                      {t(`duplicates.reason_${group.reason}` as TranslationKeys, { id: group.detail ?? "" })}
                    </span>
                    {group.discs && <span className="text-xs text-sky-400 bg-sky-400/10 px-2 py-0.5 rounded">{t("duplicates.discs")}</span>}
                  </div>
                  <div className="space-y-1">
                    {group.games.map((game, i) => (
                      <label key={game.path} className="flex items-center gap-3 px-3 py-1.5 rounded hover:bg-zinc-700/30 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={selected.has(game.path)}
                          onChange={() => toggle(group, i)}
                          className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
                        />
                        <div className="flex-1 min-w-0">
                          <span className={`text-sm truncate block ${selected.has(game.path) ? "text-zinc-400 line-through" : "text-zinc-200"}`}>
                            {game.name || game.path}
                          </span>
                          <span className="text-xs text-zinc-500 truncate block">{game.path}</span>
                        </div>
                        {keptVersion(group, i) && (
                          <span className="text-xs text-emerald-400 bg-emerald-400/10 px-2 py-0.5 rounded shrink-0">{keepReason(group)}</span>
                        )}
                        {group.protected[i] && <span className="text-xs text-sky-400 bg-sky-400/10 px-2 py-0.5 rounded shrink-0">{t("duplicates.protected")}</span>}
                        <span className="text-xs text-zinc-500 tabular-nums shrink-0">{t("duplicates.mediaCount", { count: group.media[i] })}</span>
                      </label>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {groups.length > 0 && (
          <div className="px-5 py-3 border-t border-zinc-700 space-y-1.5">
            <p className="text-xs text-zinc-400">{t("versions.whatToDo", { count: selected.size })}</p>
            <div className="flex flex-wrap gap-x-5 gap-y-1 text-sm text-zinc-300">
              {ACTIONS.map((a) => (
                <label key={a} className={`flex items-center gap-2 ${a === "set_aside" && !canSetAside ? "opacity-50" : "cursor-pointer"}`}>
                  <input
                    type="radio"
                    checked={action === a}
                    disabled={a === "set_aside" && !canSetAside}
                    onChange={() => setAction(a)}
                    className="text-indigo-600"
                  />
                  {t(`collection.versions_${a}` as TranslationKeys)}
                </label>
              ))}
            </div>
            {action === "trash" && (
              <label className="flex items-center gap-2 text-sm text-zinc-300 cursor-pointer w-fit">
                <input type="checkbox" checked={permanent} onChange={(e) => setPermanent(e.target.checked)} className="accent-red-600" />
                <span className={permanent ? "text-red-400" : ""}>{t("versions.permanent")}</span>
              </label>
            )}
            {!canSetAside && <p className="text-[11px] text-amber-400">{t("versions.noFolder")}</p>}
          </div>
        )}

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <CloseButton onClick={onClose} />
          {groups.length > 0 && (
            <button
              onClick={apply}
              disabled={selected.size === 0 || applyTask.running || preparing}
              className={`px-4 py-2 text-sm text-white rounded disabled:opacity-50 disabled:cursor-not-allowed ${action === "trash" ? "bg-red-600 hover:bg-red-500" : "bg-indigo-600 hover:bg-indigo-500"}`}
            >
              {applyTask.running ? (
                <ProcessingLabel />
              ) : preparing
                ? t("duplicates.preparing")
                : deleting
                  ? t("versions.apply_delete", { count: selected.size })
                  : t(`versions.apply_${action}` as TranslationKeys, { count: selected.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
