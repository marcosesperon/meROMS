import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { listRenameCandidates, renameRoms } from "../../commands";
import type { RenameEntry, RenameSource } from "../../types";
import { RENAME_SOURCES, renameSourceText } from "../../utils/renameSources";
import { useAppStore } from "../../store/useAppStore";
import { showMessage } from "../../utils/dialogs";

interface RenameModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
}

export function RenameModal({ open, systemPath, onClose }: RenameModalProps) {
  const { t } = useTranslation();
  const loadGames = useAppStore((s) => s.loadGames);
  const [entries, setEntries] = useState<RenameEntry[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [editedNames, setEditedNames] = useState<Record<string, string>>({});
  const [scanning, setScanning] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [source, setSource] = useState<RenameSource>("title");

  useEffect(() => {
    if (!open) {
      setEntries([]);
      setSelected(new Set());
      setEditedNames({});
      return;
    }
    scan();
  }, [open, systemPath, source]);

  // A conflicting proposal can't be applied unless the user types another name
  function isBlocked(entry: RenameEntry, names: Record<string, string> = editedNames): boolean {
    return !!entry.conflict && (names[entry.current_path] ?? entry.proposed_name) === entry.proposed_name;
  }

  async function scan() {
    setScanning(true);
    try {
      const result = await listRenameCandidates(systemPath, source);
      setEntries(result);
      // Select those that can be renamed by default (conflicts are left out)
      setSelected(new Set(result.filter((e) => e.can_rename && !e.conflict).map((e) => e.current_path)));
      // Pre-fill edited names with proposed names
      const names: Record<string, string> = {};
      for (const e of result) {
        names[e.current_path] = e.proposed_name;
      }
      setEditedNames(names);
    } catch (e) {
      console.error("Scan failed:", e);
    } finally {
      setScanning(false);
    }
  }

  function toggleEntry(path: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  function toggleAll() {
    const renameable = entries.filter((e) => e.can_rename && !isBlocked(e));
    if (selected.size === renameable.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(renameable.map((e) => e.current_path)));
    }
  }

  function updateName(path: string, name: string) {
    setEditedNames((prev) => ({ ...prev, [path]: name }));
  }

  async function handleRename() {
    if (selected.size === 0) return;
    setRenaming(true);
    try {
      const blocked = new Set(entries.filter((e) => isBlocked(e)).map((e) => e.current_path));
      const renames = Array.from(selected)
        .filter((path) => !blocked.has(path))
        .map((path) => ({
          current_path: path,
          new_name: editedNames[path] || "",
        }))
        .filter((r) => r.new_name.length > 0);

      const result = await renameRoms(systemPath, renames);
      loadGames(systemPath);
      let message = t("rename.done", { count: result.renamed });
      if (result.skipped.length > 0) {
        message += "\n\n" + t("rename.skipped", { count: result.skipped.length }) + "\n" + result.skipped.join("\n");
      }
      await showMessage(message);
      onClose();
    } catch (e) {
      await showMessage(t("rename.failed", { error: String(e) }), "error");
    } finally {
      setRenaming(false);
    }
  }

  if (!open) return null;

  const renameableCount = entries.filter((e) => e.can_rename).length;
  const selectableCount = entries.filter((e) => e.can_rename && !isBlocked(e)).length;
  const conflictCount = entries.filter((e) => e.can_rename && e.conflict).length;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[740px] max-h-[80vh] flex flex-col mx-4">
        {/* Header */}
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">
            {t("rename.title")}
          </h3>
          {/* Where the new name comes from */}
          <div className="flex gap-2 mt-3">
            {RENAME_SOURCES.map(({ id: s, label }) => (
              <button
                key={s}
                onClick={() => setSource(s)}
                disabled={scanning || renaming}
                className={`px-3 py-1 text-xs rounded transition-colors ${
                  source === s
                    ? "bg-indigo-600 text-white"
                    : "bg-zinc-700 text-zinc-400 hover:bg-zinc-600"
                }`}
              >
                {t(label)}
              </button>
            ))}
          </div>
          <p className="text-xs text-zinc-500 mt-2">
            {t(renameSourceText(source).help)}
          </p>
          {!scanning && renameableCount > 0 && (
            <p className="text-sm text-zinc-400 mt-1">
              {t("rename.found", { count: renameableCount })}
              {conflictCount > 0 && (
                <span className="text-amber-400"> · {t("rename.conflicts", { count: conflictCount })}</span>
              )}
            </p>
          )}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto">
          {scanning ? (
            <div className="flex items-center justify-center py-12 text-zinc-500">
              {t("rename.scanning")}
            </div>
          ) : renameableCount === 0 ? (
            <div className="flex items-center justify-center py-12 text-zinc-500 text-sm px-8 text-center">
              {t(renameSourceText(source).empty)}
            </div>
          ) : (
            <>
              {/* Select all */}
              <div className="px-5 py-2 border-b border-zinc-700/50 flex items-center">
                <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={
                      selected.size === selectableCount && selectableCount > 0
                    }
                    onChange={toggleAll}
                    className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                  />
                  {selected.size === selectableCount
                    ? t("rename.deselectAll")
                    : t("rename.selectAll")}
                </label>
              </div>

              {/* Table header */}
              <div className="grid grid-cols-[auto_1fr_1fr] gap-3 px-5 py-2 text-xs text-zinc-500 font-medium border-b border-zinc-700/50">
                <span className="w-5" />
                <span>{t("rename.currentName")}</span>
                <span>{t("rename.newName")}</span>
              </div>

              {/* Entries */}
              <div className="divide-y divide-zinc-700/30">
                {entries.map((entry) => {
                  const isSelected = selected.has(entry.current_path);
                  const hasChanged =
                    (editedNames[entry.current_path] || entry.proposed_name) !==
                    entry.current_name;

                  return (
                    <div
                      key={entry.current_path}
                      className={`grid grid-cols-[auto_1fr_1fr] gap-3 px-5 py-2 items-center ${
                        !entry.can_rename ? "opacity-40" : ""
                      }`}
                    >
                      <input
                        type="checkbox"
                        checked={isSelected && !isBlocked(entry)}
                        disabled={!entry.can_rename || isBlocked(entry)}
                        onChange={() => toggleEntry(entry.current_path)}
                        className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 disabled:opacity-30"
                      />
                      <span
                        className="text-sm text-zinc-400 truncate"
                        title={entry.current_name}
                      >
                        {entry.current_name}
                      </span>
                      {entry.can_rename ? (
                        <div className="flex items-center gap-2 min-w-0">
                        <input
                          type="text"
                          value={editedNames[entry.current_path] || ""}
                          onChange={(e) =>
                            updateName(entry.current_path, e.target.value)
                          }
                          className={`w-full bg-zinc-900 border rounded px-2 py-1 text-sm focus:outline-none focus:border-indigo-500 ${
                            hasChanged
                              ? "border-indigo-500/50 text-indigo-300"
                              : "border-zinc-600 text-zinc-300"
                          }`}
                        />
                        {isBlocked(entry) && (
                          <span
                            className="shrink-0 text-[10px] px-1.5 py-0.5 rounded bg-amber-600/20 text-amber-300"
                            title={t(entry.conflict === "duplicate" ? "rename.conflictDuplicateHelp" : "rename.conflictExistsHelp")}
                          >
                            {t(entry.conflict === "duplicate" ? "rename.conflictDuplicate" : "rename.conflictExists")}
                          </span>
                        )}
                        </div>
                      ) : (
                        <span className="text-sm text-zinc-500 italic">
                          {entry.current_name}
                        </span>
                      )}
                    </div>
                  );
                })}
              </div>
            </>
          )}
        </div>

        {/* Footer */}
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("rename.close")}
          </button>
          {renameableCount > 0 && (
            <button
              onClick={handleRename}
              disabled={selected.size === 0 || renaming}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {renaming
                ? "..."
                : t("rename.apply", { count: selected.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
