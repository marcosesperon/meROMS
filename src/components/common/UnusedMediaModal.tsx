import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { findUnusedMedia, findGameLeftoverMedia, findRepeatedMedia, trashMedia } from "../../commands";
import { SWAPPABLE_MEDIA, type RepeatedMedia, type UnusedMediaFile } from "../../types";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { resolveImageSrc } from "../../utils/images";

interface UnusedMediaModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
  /** Only this game's leftover media (game detail panel); without it, the whole system */
  gamePath?: string;
  gameName?: string;
  /** After sending files to the trash (to reload the games) */
  onDone?: () => void;
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
  onDone,
}: UnusedMediaModalProps) {
  const { t } = useTranslation();
  const [files, setFiles] = useState<UnusedMediaFile[]>([]);
  // Same file in more than one media type: the extra copies
  const [repeated, setRepeated] = useState<RepeatedMedia[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scanning, setScanning] = useState(false);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    if (!open) {
      setFiles([]);
      setRepeated([]);
      setSelected(new Set());
      return;
    }
    scan();
  }, [open, systemPath, gamePath]);

  async function scan() {
    setScanning(true);
    try {
      const [result, dupes] = await Promise.all([
        gamePath ? findGameLeftoverMedia(systemPath, gamePath) : findUnusedMedia(systemPath),
        findRepeatedMedia(systemPath, gamePath),
      ]);
      setFiles(result);
      setRepeated(dupes);
      setSelected(new Set([...result.map((f) => f.path), ...dupes.flatMap((r) => r.remove.map((f) => f.path))]));
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

  // Everything that can be sent to the trash: unused files + extra copies of repeated media
  const allItems = [
    ...files.map((f) => ({ path: f.path, size: f.size })),
    ...repeated.flatMap((r) => r.remove.map((f) => ({ path: f.path, size: f.size }))),
  ];

  function toggleAll() {
    if (selected.size === allItems.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(allItems.map((f) => f.path)));
    }
  }

  async function handleDelete() {
    if (selected.size === 0) return;
    if (!(await askConfirm(t("cleanup.trashConfirm", { count: selected.size })))) return;
    setDeleting(true);
    try {
      const result = await trashMedia(systemPath, Array.from(selected));
      let message = t("cleanup.deleted", { count: result.trashed });
      if (result.failed.length > 0) {
        message += "\n\n" + t("cleanup.trashFailed", { count: result.failed.length }) + "\n" + result.failed.join("\n");
      }
      await showMessage(message, result.failed.length > 0 ? "error" : "info");
      onDone?.();
      await scan();
    } catch (e) {
      await showMessage(t("cleanup.deleteFailed", { error: String(e) }), "error");
    } finally {
      setDeleting(false);
    }
  }

  if (!open) return null;

  const totalSize = allItems.reduce((sum, f) => sum + f.size, 0);
  const selectedSize = allItems.filter((f) => selected.has(f.path)).reduce((sum, f) => sum + f.size, 0);
  const kindLabel = (field: string) => {
    // "image" is labelled "Cover / Screenshot" in the editor (Classic): here it's just the cover
    if (field === "image") return t("mediaKinds.cover");
    if (field === "thumbnail") return t("mediaKinds.screenshot");
    const kind = SWAPPABLE_MEDIA.find((m) => m.field === field);
    return kind ? t(kind.labelKey) : field;
  };

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
          {!scanning && allItems.length > 0 && (
            <p className="text-sm text-zinc-400 mt-1">
              {t("cleanup.found", {
                count: allItems.length,
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
          ) : allItems.length === 0 ? (
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
                    checked={selected.size === allItems.length}
                    onChange={toggleAll}
                    className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                  />
                  {selected.size === allItems.length
                    ? t("cleanup.deselectAll")
                    : t("cleanup.selectAll")}
                </label>
                {selected.size > 0 && (
                  <span className="text-xs text-zinc-500">
                    {formatSize(selectedSize)}
                  </span>
                )}
              </div>

              {/* Unused files */}
              {files.length > 0 && (
                <p className="px-5 pt-3 pb-1 text-xs font-semibold text-zinc-400">{t("cleanup.unusedSection")}</p>
              )}
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

              {/* Same file in several media types: the extra copies (the one that stays is named) */}
              {repeated.length > 0 && (
                <>
                  <div className="px-5 pt-4 pb-1 border-t border-zinc-700/50">
                    <p className="text-xs font-semibold text-zinc-400">{t("cleanup.repeatedSection")}</p>
                    <p className="text-xs text-zinc-500">{t("cleanup.repeatedHelp")}</p>
                  </div>
                  <div className="divide-y divide-zinc-700/30">
                    {repeated.flatMap((r) =>
                      r.remove.map((file) => (
                        <label
                          key={file.path}
                          className="flex items-center gap-3 px-5 py-2 cursor-pointer transition-colors hover:bg-zinc-700/30"
                        >
                          <input
                            type="checkbox"
                            checked={selected.has(file.path)}
                            onChange={() => toggleFile(file.path)}
                            className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
                          />
                          <img
                            src={resolveImageSrc(file.path, systemPath)}
                            alt=""
                            className="w-10 h-10 object-contain bg-zinc-900 rounded shrink-0"
                          />
                          <div className="min-w-0 flex-1">
                            <p className="text-sm text-zinc-300 truncate">{r.game_name}</p>
                            <p className="text-xs text-zinc-500 truncate">
                              {t("cleanup.repeatedRow", { kind: kindLabel(file.field), keep: kindLabel(r.keep_field) })}
                            </p>
                          </div>
                          <span className="text-xs text-zinc-500 shrink-0">{formatSize(file.size)}</span>
                        </label>
                      ))
                    )}
                  </div>
                </>
              )}
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
          {allItems.length > 0 && (
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
