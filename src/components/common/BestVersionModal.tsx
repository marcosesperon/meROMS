import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { bestVersions, moveGames, removeGamesWithFiles, saveGamelist, type VersionGroup } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { reportWithUndo } from "../../utils/undo";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "./TaskProgress";

interface BestVersionModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
  /** After applying (reload games and counters) */
  onDone: () => void;
}

type Action = "hide" | "move" | "trash";

const REASON_KEY: Record<string, TranslationKeys> = {
  not_unwanted: "best.reason_not_unwanted",
  region: "best.reason_region",
  language: "best.reason_language",
  revision: "best.reason_revision",
  verified: "best.reason_verified",
  shortest: "best.reason_shortest",
  only: "best.reason_shortest",
};

/**
 * «Mejor versión» (1G1R): las versiones de cada juego con la preferida propuesta (se puede
 * cambiar) y qué hacer con las demás: ocultarlas, moverlas a una subcarpeta o a la papelera.
 */
export function BestVersionModal({ open, systemPath, onClose, onDone }: BestVersionModalProps) {
  const { t } = useTranslation();
  const [groups, setGroups] = useState<VersionGroup[]>([]);
  // Elegida de cada grupo (índice del grupo → ruta del juego)
  const [chosen, setChosen] = useState<Record<number, string>>({});
  // La búsqueda: con progreso y Cancelar
  const scanTask = useTask();
  const scanning = scanTask.running;
  const [busy, setBusy] = useState(false);
  const [action, setAction] = useState<Action>("hide");
  const [folder, setFolder] = useState(() => t("best.defaultFolder"));

  useEffect(() => {
    if (!open) {
      setGroups([]);
      setChosen({});
      return;
    }
    scanTask
      .run((task) => bestVersions(systemPath, task))
      .then((g) => {
        // Cancelado: no hay nada que enseñar
        if (!g) return onClose();
        setGroups(g);
        setChosen(Object.fromEntries(g.map((group, i) => [i, group.versions.find((v) => v.chosen)?.game_path ?? ""])));
      })
      .catch((e) => showMessage(String(e), "error"));
    // Al cerrar la ventana a medias, la búsqueda se para
    return () => scanTask.cancel();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, systemPath]);

  // Las que sobran: todas menos la elegida de cada grupo
  const others = useMemo(
    () => groups.flatMap((g, i) => g.versions.filter((v) => v.game_path !== chosen[i]).map((v) => v.game_path)),
    [groups, chosen]
  );

  if (!open) return null;

  async function apply() {
    if (others.length === 0) return;
    const confirmKey: TranslationKeys = action === "hide" ? "best.confirmHide" : action === "move" ? "best.confirmMove" : "best.confirmTrash";
    if (!(await askConfirm(t(confirmKey, { count: others.length, folder })))) return;
    setBusy(true);
    try {
      let message = "";
      if (action === "hide") {
        const { games, folders, provider } = useAppStore.getState();
        const hide = new Set(others);
        await saveGamelist(systemPath, {
          games: games.map((g) => (hide.has(g.path) ? { ...g, hidden: "true" } : g)),
          folders: folders || [],
          provider,
        });
        message = t("best.doneHide", { count: hide.size });
      } else if (action === "move") {
        const result = await moveGames(systemPath, others, systemPath, folder.trim());
        message = [t("move.moved", { count: result.moved }), ...(result.skipped.length ? ["", t("move.skipped", { count: result.skipped.length }), ...result.skipped] : [])].join("\n");
      } else {
        const result = await removeGamesWithFiles(systemPath, others, false);
        message = t("best.doneTrash", { count: result.removed });
        if (result.failed.length > 0) message += "\n\n" + result.failed.join("\n");
      }
      await reportWithUndo(message);
      onDone();
      onClose();
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  const tagLine = (v: VersionGroup["versions"][number]) =>
    [
      v.tags.regions.join(", ").toUpperCase(),
      v.tags.languages.join(", "),
      v.tags.revision ? `Rev ${v.tags.revision}` : "",
      v.tags.verified ? "[!]" : "",
      v.tags.unwanted ? t("best.unwanted") : "",
    ]
      .filter(Boolean)
      .join(" · ");

  const badFolder = action === "move" && (!folder.trim() || /(^|\/)\.\.(\/|$)|[:*?"<>|\\]/.test(folder.trim()));

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[720px] max-w-[calc(100vw-2rem)] h-[85vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("best.title")}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("best.help")}</p>
          {!scanning && groups.length > 0 && (
            <p className="text-sm text-zinc-400 mt-2">{t("best.summary", { groups: groups.length, others: others.length })}</p>
          )}
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-3 space-y-3">
          {scanning ? (
            <TaskProgress progress={scanTask.progress} label={t("best.loading")} onCancel={scanTask.cancel} cancelling={scanTask.cancelling} />
          ) : groups.length === 0 ? (
            <p className="py-12 text-center text-sm text-zinc-500">{t("best.none")}</p>
          ) : (
            groups.map((g, i) => (
              <div key={i} className="rounded border border-zinc-700 bg-zinc-900/40">
                <div className="px-3 py-2 border-b border-zinc-700/60 flex items-center gap-2">
                  <span className="text-sm font-medium text-zinc-200 truncate flex-1">{g.title}</span>
                  {chosen[i] === g.versions.find((v) => v.chosen)?.game_path && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-indigo-600/30 text-indigo-200 shrink-0">
                      {t(REASON_KEY[g.reason] ?? "best.reason_shortest")}
                    </span>
                  )}
                </div>
                {g.versions.map((v) => (
                  <label key={v.game_path} className="flex items-start gap-2 px-3 py-1.5 cursor-pointer hover:bg-zinc-700/30">
                    <input
                      type="radio"
                      name={`group-${i}`}
                      checked={chosen[i] === v.game_path}
                      onChange={() => setChosen((prev) => ({ ...prev, [i]: v.game_path }))}
                      className="mt-1 text-indigo-600"
                    />
                    <span className="min-w-0">
                      <span className={`block text-sm truncate ${chosen[i] === v.game_path ? "text-zinc-100" : "text-zinc-400"}`}>{v.file}</span>
                      <span className="block text-[11px] text-zinc-500">{tagLine(v)}</span>
                    </span>
                  </label>
                ))}
              </div>
            ))
          )}
        </div>

        {groups.length > 0 && (
          <div className="px-5 py-3 border-t border-zinc-700 space-y-2">
            <p className="text-xs text-zinc-400">{t("best.othersAction", { count: others.length })}</p>
            <div className="flex flex-wrap items-center gap-x-5 gap-y-2 text-sm text-zinc-300">
              {(["hide", "move", "trash"] as Action[]).map((a) => (
                <label key={a} className="flex items-center gap-2 cursor-pointer">
                  <input type="radio" checked={action === a} onChange={() => setAction(a)} className="text-indigo-600" />
                  {t(a === "hide" ? "best.actionHide" : a === "move" ? "best.actionMove" : "best.actionTrash")}
                </label>
              ))}
              {action === "move" && (
                <input
                  value={folder}
                  onChange={(e) => setFolder(e.target.value)}
                  className="flex-1 min-w-[160px] bg-zinc-900 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
                />
              )}
            </div>
          </div>
        )}

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button onClick={onClose} disabled={busy} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-50">
            {t("rename.close")}
          </button>
          {groups.length > 0 && (
            <button
              onClick={apply}
              disabled={busy || others.length === 0 || badFolder}
              className={`px-4 py-2 text-sm text-white rounded disabled:opacity-50 ${action === "trash" ? "bg-red-600 hover:bg-red-500" : "bg-indigo-600 hover:bg-indigo-500"}`}
            >
              {busy ? "..." : t("best.apply", { count: others.length })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
