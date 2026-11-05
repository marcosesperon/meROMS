import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { listSubfolders, moveGames } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { showMessage } from "../../utils/dialogs";
import { reportWithUndo } from "../../utils/undo";

interface MoveGamesModalProps {
  /** ROM folder of the system the games are in */
  systemPath: string;
  gamePaths: string[];
  onClose: () => void;
  /** After moving (the caller reloads, clears the selection, closes the editor…) */
  onDone: () => void;
}

// Valores especiales del selector de subcarpeta
const ROOT = "";
const NEW = "\u0000new";

/**
 * «Mover a…»: lleva los juegos a otro sistema o a una subcarpeta con su ROM, los ficheros
 * que usa, su media, su entrada de la gamelist y sus colecciones. Nunca sobrescribe.
 */
export function MoveGamesModal({ systemPath, gamePaths, onClose, onDone }: MoveGamesModalProps) {
  const { t } = useTranslation();
  const systems = useAppStore((s) => s.systems);
  const [dest, setDest] = useState(systemPath);
  const [subfolders, setSubfolders] = useState<string[]>([]);
  const [subfolder, setSubfolder] = useState(ROOT);
  const [newFolder, setNewFolder] = useState("");
  const [moving, setMoving] = useState(false);

  // Subcarpetas del sistema elegido
  useEffect(() => {
    setSubfolder(ROOT);
    listSubfolders(dest).then(setSubfolders).catch(() => setSubfolders([]));
  }, [dest]);

  // Esc cierra (salvo mientras mueve)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || moving) return;
      e.stopPropagation();
      onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [moving, onClose]);

  const target = subfolder === NEW ? newFolder.trim().replace(/^\/+|\/+$/g, "") : subfolder;
  const badName = subfolder === NEW && (!target || /(^|\/)\.\.(\/|$)|[:*?"<>|\\]/.test(target));
  const destSystem = systems.find((s) => s.path === dest);

  async function move() {
    setMoving(true);
    try {
      const result = await moveGames(systemPath, gamePaths, dest, target || undefined);
      const lines = [t("move.moved", { count: result.moved })];
      if (result.skipped.length > 0) {
        lines.push("", t("move.skipped", { count: result.skipped.length }), ...result.skipped);
      }
      await reportWithUndo(lines.join("\n"));
      onDone();
      onClose();
    } catch (e) {
      await showMessage(t("move.failed", { error: String(e) }), "error");
    } finally {
      setMoving(false);
    }
  }

  const selectClass = "w-full bg-zinc-900 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500";

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-[60]" onClick={() => !moving && onClose()}>
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[480px] max-w-[calc(100vw-2rem)] flex flex-col mx-4" onClick={(e) => e.stopPropagation()}>
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("move.title", { count: gamePaths.length })}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("move.help")}</p>
        </div>
        <div className="px-5 py-4 space-y-4">
          <label className="block">
            <span className="text-xs text-zinc-400">{t("move.system")}</span>
            <select value={dest} onChange={(e) => setDest(e.target.value)} className={`${selectClass} mt-1`}>
              {systems.map((s) => (
                <option key={s.path} value={s.path}>
                  {s.full_name || s.name} ({s.name})
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className="text-xs text-zinc-400">{t("move.subfolder")}</span>
            <select value={subfolder} onChange={(e) => setSubfolder(e.target.value)} className={`${selectClass} mt-1`}>
              <option value={ROOT}>{t("move.root")}</option>
              {subfolders.map((f) => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
              <option value={NEW}>{t("move.newFolder")}</option>
            </select>
          </label>
          {subfolder === NEW && (
            <input
              autoFocus
              value={newFolder}
              onChange={(e) => setNewFolder(e.target.value)}
              placeholder={t("move.newFolderPlaceholder")}
              className={selectClass}
            />
          )}
          <p className="text-sm text-zinc-300">
            {t("move.summary", { count: gamePaths.length, target: `${destSystem?.name ?? ""}${target ? `/${target}` : ""}` })}
          </p>
        </div>
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-end gap-3">
          <button onClick={onClose} disabled={moving} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-50">
            {t("move.cancel")}
          </button>
          <button
            onClick={move}
            disabled={moving || badName}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50"
          >
            {moving ? "..." : t("move.move")}
          </button>
        </div>
      </div>
    </div>
  );
}
