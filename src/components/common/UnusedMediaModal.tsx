import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { clearMediaRefs, findBrokenMediaRefs, findPlaceholderMedia, findUnusedMedia, findGameLeftoverMedia, findRepeatedMedia, learnPlaceholders, trashMedia, type PlaceholderGroup } from "../../commands";
import { SWAPPABLE_MEDIA, type BrokenMediaRef, type RepeatedMedia, type UnusedMediaFile } from "../../types";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { resolveImageSrc } from "../../utils/images";
import { MEDIA_FILTER_FIELDS } from "../../utils/mediaFilter";

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

// Selection key of a broken reference (files use their path)
const REF_PREFIX = "ref:";
const refKey = (r: BrokenMediaRef) => `${REF_PREFIX}${r.game_path}|${r.field}`;
// Selection key of a placeholder group (all its files go together)
const PH_PREFIX = "ph:";
const placeholderKey = (g: PlaceholderGroup) => `${PH_PREFIX}${g.md5}`;

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
  // Media fields of the gamelist pointing to files that don't exist (Classic only)
  const [broken, setBroken] = useState<BrokenMediaRef[]>([]);
  // The same file in many different games: a source's placeholder (whole system only)
  const [placeholders, setPlaceholders] = useState<PlaceholderGroup[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scanning, setScanning] = useState(false);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    if (!open) {
      setFiles([]);
      setRepeated([]);
      setBroken([]);
      setPlaceholders([]);
      setSelected(new Set());
      return;
    }
    scan();
  }, [open, systemPath, gamePath]);

  async function scan() {
    setScanning(true);
    try {
      const [result, dupes, refs, fillers] = await Promise.all([
        gamePath ? findGameLeftoverMedia(systemPath, gamePath) : findUnusedMedia(systemPath),
        findRepeatedMedia(systemPath, gamePath),
        findBrokenMediaRefs(systemPath, gamePath),
        // Los rellenos solo se ven mirando el sistema entero
        gamePath ? Promise.resolve([] as PlaceholderGroup[]) : findPlaceholderMedia(systemPath),
      ]);
      setFiles(result);
      setRepeated(dupes);
      setBroken(refs);
      setPlaceholders(fillers);
      setSelected(
        new Set([
          ...result.map((f) => f.path),
          ...dupes.flatMap((r) => r.remove.map((f) => f.path)),
          ...refs.map(refKey),
          ...fillers.map(placeholderKey),
        ])
      );
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
  // Everything that can be cleaned: files (to the trash) and broken references (out of the
  // gamelist). Broken references are keyed with refKey() in the same selection.
  const allItems = [
    ...files.map((f) => ({ path: f.path, size: f.size })),
    ...repeated.flatMap((r) => r.remove.map((f) => ({ path: f.path, size: f.size }))),
    ...broken.map((r) => ({ path: refKey(r), size: 0 })),
    ...placeholders.map((g) => ({ path: placeholderKey(g), size: g.size * g.files.length })),
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
    const refs = broken.filter((r) => selected.has(refKey(r)));
    const fillers = placeholders.filter((g) => selected.has(placeholderKey(g)));
    // Cada relleno marcado aporta todos sus ficheros (sin repetir los que ya estén en otra sección)
    const paths = [
      ...new Set([
        ...Array.from(selected).filter((key) => !key.startsWith(REF_PREFIX) && !key.startsWith(PH_PREFIX)),
        ...fillers.flatMap((g) => g.files.map((f) => f.path)),
      ]),
    ];
    if (!(await askConfirm(t("cleanup.cleanConfirm", { files: paths.length, refs: refs.length })))) return;
    setDeleting(true);
    try {
      const lines: string[] = [];
      let failed = false;
      if (paths.length > 0) {
        const result = await trashMedia(systemPath, paths);
        lines.push(t("cleanup.deleted", { count: result.trashed }));
        if (result.failed.length > 0) {
          failed = true;
          lines.push("", t("cleanup.trashFailed", { count: result.failed.length }), ...result.failed);
        }
      }
      if (refs.length > 0) {
        lines.push(t("cleanup.refsCleared", { count: await clearMediaRefs(systemPath, refs) }));
      }
      // Los rellenos borrados se apuntan: el scraper ya no los vuelve a guardar
      if (fillers.length > 0) {
        await learnPlaceholders(fillers.map((g) => ({ size: g.size, md5: g.md5, field: g.fields[0] ?? "" })));
        lines.push(t("cleanup.placeholdersLearned", { count: fillers.length }));
      }
      await showMessage(lines.join("\n"), failed ? "error" : "info");
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
    if (kind) return t(kind.labelKey);
    // Vídeo, manual…: los nombres del filtro «Le falta»
    const other = MEDIA_FILTER_FIELDS.find((m) => m.field === field);
    return other ? t(other.label) : field;
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

              {/* The same file in many different games: a source's placeholder */}
              {placeholders.length > 0 && (
                <>
                  <div className="px-5 pt-4 pb-1 border-t border-zinc-700/50">
                    <p className="text-xs font-semibold text-zinc-400">{t("cleanup.placeholdersSection")}</p>
                    <p className="text-xs text-zinc-500">{t("cleanup.placeholdersHelp")}</p>
                  </div>
                  <div className="divide-y divide-zinc-700/30">
                    {placeholders.map((g) => {
                      const isVideo = g.fields.includes("video");
                      const names = g.files.slice(0, 3).map((f) => f.game_name).join(", ");
                      return (
                        <label
                          key={placeholderKey(g)}
                          className="flex items-center gap-3 px-5 py-2 cursor-pointer transition-colors hover:bg-zinc-700/30"
                        >
                          <input
                            type="checkbox"
                            checked={selected.has(placeholderKey(g))}
                            onChange={() => toggleFile(placeholderKey(g))}
                            className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
                          />
                          {isVideo ? (
                            <span className="w-10 h-10 flex items-center justify-center bg-zinc-900 rounded shrink-0 text-zinc-500">▶</span>
                          ) : (
                            <img
                              src={resolveImageSrc(g.files[0].path, systemPath)}
                              alt=""
                              className="w-10 h-10 object-contain bg-zinc-900 rounded shrink-0"
                            />
                          )}
                          <div className="min-w-0 flex-1">
                            <p className="text-sm text-zinc-300 truncate">
                              {g.fields.map(kindLabel).join(", ")} · {t("cleanup.placeholderGames", { count: g.games })}
                            </p>
                            <p className="text-xs text-zinc-500 truncate" title={g.files.map((f) => f.game_name).join("\n")}>
                              {names}
                              {g.files.length > 3 ? "…" : ""}
                            </p>
                          </div>
                          <span className="text-xs text-zinc-500 shrink-0">{formatSize(g.size)}</span>
                        </label>
                      );
                    })}
                  </div>
                </>
              )}

              {/* Gamelist fields pointing to files that don't exist: taken out of the XML */}
              {broken.length > 0 && (
                <>
                  <div className="px-5 pt-4 pb-1 border-t border-zinc-700/50">
                    <p className="text-xs font-semibold text-zinc-400">{t("cleanup.brokenSection")}</p>
                    <p className="text-xs text-zinc-500">{t("cleanup.brokenHelp")}</p>
                  </div>
                  <div className="divide-y divide-zinc-700/30">
                    {broken.map((r) => (
                      <label
                        key={refKey(r)}
                        className="flex items-center gap-3 px-5 py-2 cursor-pointer transition-colors hover:bg-zinc-700/30"
                      >
                        <input
                          type="checkbox"
                          checked={selected.has(refKey(r))}
                          onChange={() => toggleFile(refKey(r))}
                          className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
                        />
                        <div className="min-w-0 flex-1">
                          <p className="text-sm text-zinc-300 truncate">
                            {r.game_name} <span className="text-zinc-500">· {kindLabel(r.field)}</span>
                          </p>
                          <p className="text-xs text-zinc-500 font-mono truncate" title={r.value}>
                            {r.value}
                          </p>
                        </div>
                      </label>
                    ))}
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
                : t("cleanup.clean", { count: selected.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
