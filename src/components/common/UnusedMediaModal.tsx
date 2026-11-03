import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { findUnusedMedia, findGameLeftoverMedia, deleteFiles } from "../../commands";
import type { UnusedMediaFile } from "../../types";

interface UnusedMediaModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
  /** Only this game's leftover media (game detail panel); without it, the whole system */
  gamePath?: string;
  gameName?: string;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function UnusedMediaModal({
  open,
  systemPath,
  onClose,
  gamePath,
  gameName,
}: UnusedMediaModalProps) {
  const { t } = useTranslation();
  const [files, setFiles] = useState<UnusedMediaFile[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scanning, setScanning] = useState(false);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    if (!open) {
      setFiles([]);
      setSelected(new Set());
      return;
    }
    scan();
  }, [open, systemPath, gamePath]);

  async function scan() {
    setScanning(true);
    try {
      const result = gamePath
        ? await findGameLeftoverMedia(systemPath, gamePath)
        : await findUnusedMedia(systemPath);
      setFiles(result);
      setSelected(new Set(result.map((f) => f.path)));
    } catch (e) {
      console.error("Scan failed:", e);
    } finally {
      setScanning(false);
    }
  }

  function toggleFile(path: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  function toggleAll() {
    if (selected.size === files.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(files.map((f) => f.path)));
    }
  }

  async function handleDelete() {
    if (selected.size === 0) return;
    setDeleting(true);
    try {
      const count = await deleteFiles(Array.from(selected));
      // Remove deleted files from list
      setFiles((prev) => prev.filter((f) => !selected.has(f.path)));
      setSelected(new Set());
      if (count > 0) {
        alert(t("cleanup.deleted", { count }));
      }
    } catch (e) {
      alert(t("cleanup.deleteFailed", { error: String(e) }));
    } finally {
      setDeleting(false);
    }
  }

  if (!open) return null;

  const totalSize = files.reduce((sum, f) => sum + f.size, 0);
  const selectedSize = files
    .filter((f) => selected.has(f.path))
    .reduce((sum, f) => sum + f.size, 0);

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[640px] max-h-[80vh] flex flex-col mx-4">
        {/* Header */}
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">
            {gamePath
              ? t("gameTools.cleanTitle", { name: gameName || gamePath })
              : t("cleanup.title")}
          </h3>
          {gamePath && (
            <p className="text-xs text-zinc-500 mt-1">{t("gameTools.cleanDescription")}</p>
          )}
          {!scanning && files.length > 0 && (
            <p className="text-sm text-zinc-400 mt-1">
              {t("cleanup.found", {
                count: files.length,
                size: formatSize(totalSize),
              })}
            </p>
          )}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto">
          {scanning ? (
            <div className="flex items-center justify-center py-12 text-zinc-500">
              {t("cleanup.scanning")}
            </div>
          ) : files.length === 0 ? (
            <div className="flex items-center justify-center py-12 text-zinc-500 text-sm px-8 text-center">
              {gamePath ? t("gameTools.cleanNothing") : t("cleanup.noUnused")}
            </div>
          ) : (
            <>
              {/* Select all toggle */}
              <div className="px-5 py-2 border-b border-zinc-700/50 flex items-center justify-between">
                <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={selected.size === files.length}
                    onChange={toggleAll}
                    className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                  />
                  {selected.size === files.length
                    ? t("cleanup.deselectAll")
                    : t("cleanup.selectAll")}
                </label>
                {selected.size > 0 && (
                  <span className="text-xs text-zinc-500">
                    {formatSize(selectedSize)}
                  </span>
                )}
              </div>

              {/* File list */}
              <div className="divide-y divide-zinc-700/30">
                {files.map((file) => (
                  <label
                    key={file.path}
                    className="flex items-center gap-3 px-5 py-2 hover:bg-zinc-750 cursor-pointer transition-colors hover:bg-zinc-700/30"
                  >
                    <input
                      type="checkbox"
                      checked={selected.has(file.path)}
                      onChange={() => toggleFile(file.path)}
                      className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
                    />
                    <span className="text-sm text-zinc-300 truncate flex-1" title={file.relative}>
                      {file.relative}
                    </span>
                    <span className="text-xs text-zinc-500 shrink-0">
                      {formatSize(file.size)}
                    </span>
                  </label>
                ))}
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
            {t("cleanup.close")}
          </button>
          {files.length > 0 && (
            <button
              onClick={handleDelete}
              disabled={selected.size === 0 || deleting}
              className="px-4 py-2 text-sm bg-red-600 text-white rounded hover:bg-red-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {deleting
                ? "..."
                : t("cleanup.delete", { count: selected.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
