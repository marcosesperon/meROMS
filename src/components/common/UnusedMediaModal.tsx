import { useEffect, useState } from "react";
import { ModalShell } from "./ModalShell";
import { useTranslation } from "../../i18n";
import { clearMediaRefs, findBrokenMediaRefs, findDamagedMedia, findPlaceholderMedia, findUnusedMedia, findGameLeftoverMedia, findRepeatedMedia, learnPlaceholders, type DamagedMedia, type PlaceholderGroup } from "../../commands";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "./TaskProgress";
import type { ScrapePolicy } from "../../types";
import { DATA_FIELDS } from "./ScrapePolicyEditor";
import { SWAPPABLE_MEDIA, type BrokenMediaRef, type RepeatedMedia, type UnusedMediaFile } from "../../types";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { resolveImageSrc } from "../../utils/images";
import { MEDIA_FILTER_FIELDS } from "../../utils/mediaFilter";
import { mediaAction, removeMedia } from "../../utils/collectionActions";
import { collectionRulesFrom } from "../../utils/collectionRules";
import { useAppStore } from "../../store/useAppStore";

interface UnusedMediaModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
  /** Only this game's leftover media (game detail panel); without it, the whole system */
  gamePath?: string;
  gameName?: string;
  /** After sending files to the trash (to reload the games) */
  onDone?: () => void;
  /** Download again the media of these games (whole-system mode): Scrape All with this policy */
  onRescrape?: (gamePaths: string[], policy: ScrapePolicy) => void;
  /** Dentro de la ventana «Medios» (una pestaña): sin marco propio */
  embedded?: boolean;
  /** Qué se busca: todo (de serie), solo dañados y placeholders, o solo sobrantes */
  sections?: "all" | "damaged" | "unused";
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

/** Para volver a descargar los dañados: datos sin tocar y solo sus tipos de media, «si falta» */
function rescrapePolicy(gone: DamagedMedia[]): ScrapePolicy {
  return {
    data: Object.fromEntries(DATA_FIELDS.map(({ field }) => [field, "keep"])),
    media: Object.fromEntries(gone.flatMap((d) => d.scraper_types).map((type) => [type, "if_empty"])),
  };
}

export function UnusedMediaModal({
  open,
  systemPath,
  onClose,
  gamePath,
  gameName,
  onDone,
  onRescrape,
  embedded,
  sections = "all",
}: UnusedMediaModalProps) {
  const { t } = useTranslation();
  // Papelera (de serie) o carpeta de apartados (Ajustes → Mi colección, si hay carpeta)
  const action = mediaAction(collectionRulesFrom(useAppStore((s) => s.settings.collection_rules)));
  const [files, setFiles] = useState<UnusedMediaFile[]>([]);
  // Same file in more than one media type: the extra copies
  const [repeated, setRepeated] = useState<RepeatedMedia[]>([]);
  // Media fields of the gamelist pointing to files that don't exist (Classic only)
  const [broken, setBroken] = useState<BrokenMediaRef[]>([]);
  // The same file in many different games: a source's placeholder (whole system only)
  const [placeholders, setPlaceholders] = useState<PlaceholderGroup[]>([]);
  // Empty, not a real image/video, or cut short (a download left halfway)
  const [damaged, setDamaged] = useState<DamagedMedia[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  // Las cinco búsquedas van una detrás de otra con el mismo progreso (y un solo Cancelar)
  const scanTask = useTask();
  const scanning = scanTask.running;
  const [deleting, setDeleting] = useState(false);
  // Mandar a la papelera: con progreso y Cancelar (se para entre tandas; lo enviado, enviado queda)
  const trashTask = useTask();

  useEffect(() => {
    if (!open) {
      setFiles([]);
      setRepeated([]);
      setBroken([]);
      setPlaceholders([]);
      setDamaged([]);
      setSelected(new Set());
      return;
    }
    // Una búsqueda cancelada al desmontar (o por React en desarrollo, que repite el efecto) no
    // cierra la ventana: solo el Cancelar del usuario
    let alive = true;
    scan(() => alive);
    // Al cerrar la ventana a medias, la búsqueda se para
    return () => {
      alive = false;
      scanTask.cancel();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, systemPath, gamePath, sections]);

  async function scan(alive: () => boolean = () => true) {
    try {
      // Una detrás de otra: así la barra cuenta lo que se está haciendo, no cinco cosas a la vez
      // En la ventana «Medios» cada pestaña busca solo lo suyo: sobrantes (y copias repetidas) o
      // dañados (y placeholders y rutas rotas)
      const unusedPart = sections !== "damaged";
      const damagedPart = sections !== "unused";
      const found = await scanTask.run(async (task) => {
        const result = !unusedPart ? [] : gamePath ? await findGameLeftoverMedia(systemPath, gamePath) : await findUnusedMedia(systemPath, task);
        const dupes = unusedPart ? await findRepeatedMedia(systemPath, gamePath, task) : [];
        const refs = damagedPart ? await findBrokenMediaRefs(systemPath, gamePath, task) : [];
        // Los rellenos solo se ven mirando el sistema entero
        const fillers = gamePath || !damagedPart ? ([] as PlaceholderGroup[]) : await findPlaceholderMedia(systemPath, task);
        const broken_files = damagedPart ? await findDamagedMedia(systemPath, gamePath, task) : [];
        return [result, dupes, refs, fillers, broken_files] as const;
      });
      // Cancelado: no hay nada que enseñar
      if (!found) {
        if (alive()) onClose();
        return;
      }
      const [result, dupes, refs, fillers, broken_files] = found;
      setFiles(result);
      setRepeated(dupes);
      setBroken(refs);
      setPlaceholders(fillers);
      setDamaged(broken_files);
      setSelected(
        new Set([
          // Los sobrantes que «Normalizar» recuperaría empiezan sin marcar
          ...result.filter((f) => !f.recoverable).map((f) => f.path),
          ...dupes.flatMap((r) => r.remove.map((f) => f.path)),
          ...refs.map(refKey),
          ...fillers.map(placeholderKey),
          ...broken_files.map((d) => d.path),
        ])
      );
    } catch (e) {
      console.error("Scan failed:", e);
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
    ...damaged.map((d) => ({ path: d.path, size: d.size })),
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
    const confirmKey = action === "set_aside" ? "cleanup.setAsideConfirm" : "cleanup.cleanConfirm";
    if (!(await askConfirm(t(confirmKey, { files: paths.length, refs: refs.length })))) return;
    setDeleting(true);
    try {
      const lines: string[] = [];
      let failed = false;
      if (paths.length > 0) {
        // Papelera o carpeta de apartados, según «Mi colección»
        const result = await trashTask.run((task) => removeMedia(systemPath, paths, action, [], task));
        if (!result) return;
        if (trashTask.wasCancelled()) lines.push(t("task.stopped", { done: result.done, total: paths.length }), "");
        lines.push(result.line);
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
      // Dañados que se han borrado: se ofrece volver a descargarlos (solo esa media, sin tocar datos)
      const gone = damaged.filter((d) => paths.includes(d.path));
      if (gone.length > 0 && onRescrape) {
        onDone?.();
        const games = [...new Set(gone.map((d) => d.game_path))];
        if (await askConfirm(lines.join("\n") + "\n\n" + t("cleanup.rescrapeAsk", { count: games.length }))) {
          onRescrape(games, rescrapePolicy(gone));
          onClose();
          return;
        }
        await scan();
        return;
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

  const damageLabel = (d: DamagedMedia) =>
    t(d.damage === "empty" ? "cleanup.damageEmpty" : d.damage === "not_media" ? "cleanup.damageNotMedia" : "cleanup.damageTruncated");

  const totalSize = allItems.reduce((sum, f) => sum + f.size, 0);
  // Los ficheros sin usar, en tres grupos: los de siempre y (ES-DE) los sobrantes de la
  // estructura clásica en la carpeta de ROMs, sin ROM o de una ROM
  const plainFiles = files.filter((f) => !f.leftover && !f.recoverable);
  // Clásica: con nombre antiguo (o con el título del juego) que «Normalizar» coloca: no sobran
  const classicRecoverable = files.filter((f) => !f.leftover && f.recoverable);
  const orphanLeftovers = files.filter((f) => f.leftover && !f.owner);
  // De una ROM: copias de algo que ES-DE ya tiene (sobran) o medios que «Normalizar» recupera
  const copyLeftovers = files.filter((f) => f.leftover && f.owner && !f.recoverable);
  const ownedLeftovers = files.filter((f) => f.leftover && f.recoverable);
  const fileRow = (file: UnusedMediaFile) => (
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
      <span className="min-w-0 flex-1">
        <span className="block text-sm text-zinc-300 truncate" title={file.relative}>
          {file.relative}
        </span>
        {file.owner && <span className="block text-xs text-zinc-500 font-mono truncate">{t("cleanup.leftoverOwner", { rom: file.owner })}</span>}
      </span>
      <span className="text-xs text-zinc-500 shrink-0">{formatSize(file.size)}</span>
    </label>
  );
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
    <ModalShell embedded={embedded} className="w-[640px] max-h-[80vh]">
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
          {trashTask.running ? (
            <TaskProgress progress={trashTask.progress} onCancel={trashTask.cancel} cancelling={trashTask.cancelling} />
          ) : scanning ? (
            <TaskProgress progress={scanTask.progress} label={t("cleanup.scanning")} onCancel={scanTask.cancel} cancelling={scanTask.cancelling} />
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
              {plainFiles.length > 0 && (
                <p className="px-5 pt-3 pb-1 text-xs font-semibold text-zinc-400">{t("cleanup.unusedSection")}</p>
              )}
              <div className="divide-y divide-zinc-700/30">{plainFiles.map((file) => fileRow(file))}</div>

              {/* Clásica: los que «Normalizar» recupera (desmarcados, con el juego del que son) */}
              {classicRecoverable.length > 0 && (
                <>
                  <div className="px-5 pt-4 pb-1 border-t border-zinc-700/50">
                    <p className="text-xs font-semibold text-zinc-400">{t("cleanup.recoverableSection")}</p>
                    <p className="text-xs text-zinc-500">{t("cleanup.recoverableHelp")}</p>
                  </div>
                  <div className="divide-y divide-zinc-700/30">{classicRecoverable.map((file) => fileRow(file))}</div>
                </>
              )}

              {/* ES-DE: lo que queda de la estructura clásica en la carpeta de ROMs, sin ROM (marcados) */}
              {orphanLeftovers.length > 0 && (
                <>
                  <div className="px-5 pt-4 pb-1 border-t border-zinc-700/50">
                    <p className="text-xs font-semibold text-zinc-400">{t("cleanup.leftoverSection")}</p>
                    <p className="text-xs text-zinc-500">{t("cleanup.leftoverHelp")}</p>
                  </div>
                  <div className="divide-y divide-zinc-700/30">{orphanLeftovers.map((file) => fileRow(file))}</div>
                </>
              )}

              {/* De una ROM, pero ES-DE ya tiene ese medio: copias viejas (marcados) */}
              {copyLeftovers.length > 0 && (
                <>
                  <div className="px-5 pt-4 pb-1 border-t border-zinc-700/50">
                    <p className="text-xs font-semibold text-zinc-400">{t("cleanup.leftoverCopySection")}</p>
                    <p className="text-xs text-zinc-500">{t("cleanup.leftoverCopyHelp")}</p>
                  </div>
                  <div className="divide-y divide-zinc-700/30">{copyLeftovers.map((file) => fileRow(file))}</div>
                </>
              )}

              {/* …y los que «Normalizar media» recuperaría (desmarcados) */}
              {ownedLeftovers.length > 0 && (
                <>
                  <div className="px-5 pt-4 pb-1 border-t border-zinc-700/50">
                    <p className="text-xs font-semibold text-zinc-400">{t("cleanup.leftoverOwnedSection")}</p>
                    <p className="text-xs text-zinc-500">{t("cleanup.leftoverOwnedHelp")}</p>
                  </div>
                  <div className="divide-y divide-zinc-700/30">{ownedLeftovers.map((file) => fileRow(file))}</div>
                </>
              )}

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

              {/* Empty, not a real image/video, or cut short */}
              {damaged.length > 0 && (
                <>
                  <div className="px-5 pt-4 pb-1 border-t border-zinc-700/50">
                    <p className="text-xs font-semibold text-zinc-400">{t("cleanup.damagedSection")}</p>
                    <p className="text-xs text-zinc-500">{t("cleanup.damagedHelp")}</p>
                  </div>
                  <div className="divide-y divide-zinc-700/30">
                    {damaged.map((d) => (
                      <label
                        key={d.path}
                        className="flex items-center gap-3 px-5 py-2 cursor-pointer transition-colors hover:bg-zinc-700/30"
                      >
                        <input
                          type="checkbox"
                          checked={selected.has(d.path)}
                          onChange={() => toggleFile(d.path)}
                          className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
                        />
                        <div className="min-w-0 flex-1">
                          <p className="text-sm text-zinc-300 truncate">
                            {d.game_name} <span className="text-zinc-500">· {kindLabel(d.field)}</span>
                          </p>
                          <p className="text-xs text-amber-400/80 truncate" title={d.path}>
                            {damageLabel(d)}
                          </p>
                        </div>
                        <span className="text-xs text-zinc-500 shrink-0">{formatSize(d.size)}</span>
                      </label>
                    ))}
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
                : t(action === "set_aside" ? "cleanup.setAside" : "cleanup.clean", { count: selected.size })}
            </button>
          )}
        </div>
    </ModalShell>
  );
}
