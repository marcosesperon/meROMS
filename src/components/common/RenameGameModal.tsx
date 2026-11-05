import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { checkRenameTarget, listRenameCandidates, renameRoms } from "../../commands";
import type { RenameEntry, RenameSource } from "../../types";
import { RENAME_SOURCES, renameSourceText } from "../../utils/renameSources";
import { useDebounce } from "../../hooks/useDebounce";
import { showMessage } from "../../utils/dialogs";
import { useAppStore } from "../../store/useAppStore";

interface RenameGameModalProps {
  open: boolean;
  systemPath: string;
  gamePath: string;
  onClose: () => void;
  /** Se llama con la ruta nueva de la ROM cuando el renombrado ha ido bien */
  onDone: (newPath: string) => void;
}

type Conflict = "invalid" | "exists" | "duplicate" | null;

/** "./sub/Juego (USA).sfc" + "Juego" → "./sub/Juego.sfc" (solo cambia el nombre, no la carpeta) */
function pathWithName(entry: RenameEntry, newName: string): string {
  const slash = entry.current_path.lastIndexOf("/");
  const fileName = entry.current_path.slice(slash + 1);
  const ext = fileName.slice(entry.current_name.length);
  return entry.current_path.slice(0, slash + 1) + newName + ext;
}

/**
 * Renombrar UNA ROM desde el detalle del juego. Usa los mismos comandos que la ventana
 * del sistema; la diferencia es que aquí el nombre escrito a mano se comprueba según
 * escribes, para no dejar aplicar un nombre que choca.
 */
export function RenameGameModal({ open, systemPath, gamePath, onClose, onDone }: RenameGameModalProps) {
  const { t } = useTranslation();
  const [source, setSource] = useState<RenameSource>("title");
  const [entry, setEntry] = useState<RenameEntry | null>(null);
  const [name, setName] = useState("");
  const [conflict, setConflict] = useState<Conflict>(null);
  const [checking, setChecking] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const debouncedName = useDebounce(name, 250);

  // Propuesta inicial (y cada vez que se cambia entre título y reglas)
  useEffect(() => {
    if (!open) {
      setEntry(null);
      setName("");
      setConflict(null);
      return;
    }
    listRenameCandidates(systemPath, source, gamePath)
      .then(([found]) => {
        setEntry(found ?? null);
        setName(found?.proposed_name ?? "");
        setConflict((found?.conflict as Conflict) ?? null);
      })
      .catch((e) => showMessage(t("rename.failed", { error: String(e) }), "error"));
  }, [open, systemPath, gamePath, source]);

  // Comprobación del nombre según se escribe (con una pequeña espera)
  useEffect(() => {
    // Mientras la espera no ha terminado, el nombre comprobado sería uno viejo
    if (!entry || debouncedName !== name) return;
    if (debouncedName === entry.current_name) {
      setConflict(null);
      return;
    }
    let cancelled = false;
    setChecking(true);
    checkRenameTarget(systemPath, gamePath, debouncedName)
      .then((result) => !cancelled && setConflict(result))
      .catch(console.error)
      .finally(() => !cancelled && setChecking(false));
    return () => {
      cancelled = true;
    };
  }, [debouncedName, name, entry]);

  if (!open) return null;

  const unchanged = !!entry && name === entry.current_name;
  // Una carpeta como ROM no tiene extensión: el renombrado del sistema tampoco la toca
  const hasExtension = !!entry && entry.current_path.length > entry.current_name.length + 2;
  const canApply =
    !!entry && hasExtension && !unchanged && !conflict && !checking && name === debouncedName && !renaming;

  async function handleApply() {
    if (!entry) return;
    setRenaming(true);
    try {
      const result = await renameRoms(systemPath, [{ current_path: entry.current_path, new_name: name }]);
      if (result.renamed === 0) {
        await showMessage(t("gameTools.renameSkipped"));
        return;
      }
      onDone(pathWithName(entry, name));
      onClose();
      // Queda en Herramientas → «Deshacer»
      useAppStore.getState().refreshUndo();
    } catch (e) {
      await showMessage(t("rename.failed", { error: String(e) }), "error");
    } finally {
      setRenaming(false);
    }
  }

  const conflictText: Record<Exclude<Conflict, null>, string> = {
    invalid: t("gameTools.conflictInvalid"),
    exists: t("gameTools.conflictExists"),
    duplicate: t("rename.conflictDuplicateHelp"),
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-[60]">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[520px] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("gameTools.renameTitle")}</h3>
          {/* De dónde sale el nombre propuesto */}
          <div className="flex gap-2 mt-3">
            {RENAME_SOURCES.map(({ id: s, label }) => (
              <button
                key={s}
                onClick={() => setSource(s)}
                disabled={renaming}
                className={`px-3 py-1 text-xs rounded transition-colors ${
                  source === s ? "bg-indigo-600 text-white" : "bg-zinc-700 text-zinc-400 hover:bg-zinc-600"
                }`}
              >
                {t(label)}
              </button>
            ))}
          </div>
          <p className="text-xs text-zinc-500 mt-2">
            {t(renameSourceText(source).help)}
          </p>
        </div>

        <div className="px-5 py-4 space-y-3">
          <div>
            <label className="text-xs text-zinc-400 mb-1 block">{t("gameTools.renameCurrent")}</label>
            <p className="text-sm text-zinc-300 font-mono break-all">{entry?.current_path ?? "..."}</p>
          </div>
          <div>
            <label className="text-xs text-zinc-400 mb-1 block">{t("gameTools.renameNew")}</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={!entry || renaming}
              className={`w-full bg-zinc-900 border rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none ${
                conflict ? "border-amber-500" : "border-zinc-600 focus:border-indigo-500"
              }`}
            />
            {conflict ? (
              <p className="text-xs text-amber-400 mt-1">{conflictText[conflict]}</p>
            ) : unchanged ? (
              <p className="text-xs text-zinc-500 mt-1">{t("gameTools.renameSame")}</p>
            ) : (
              <p className="text-xs text-zinc-500 mt-1">{t("gameTools.renameHelp")}</p>
            )}
            {!!entry?.tracks && <p className="text-xs text-zinc-400 mt-1">{t("rename.tracksHelp")} ({t("rename.tracks", { count: entry.tracks })})</p>}
          </div>
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
            disabled={!canApply}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {renaming ? "..." : t("gameTools.renameApply")}
          </button>
        </div>
      </div>
    </div>
  );
}
