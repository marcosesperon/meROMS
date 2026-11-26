import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "../../i18n";
import { renamePreview, renameRoms } from "../../commands";
import type { RenameEntry, RenameOptions, RenameRequest } from "../../types";
import { activeSources, hasExtension, nameSourceText, planRename, previewRequest, renameOptionsFrom, renameRequestOf } from "../../utils/renameSources";
import { RenameOptionsPanel } from "./RenameOptionsPanel";
import { useAppStore } from "../../store/useAppStore";
import { showMessage } from "../../utils/dialogs";
import { reportWithUndo } from "../../utils/undo";
import { useTask } from "../../hooks/useTask";
import { useDebounce } from "../../hooks/useDebounce";
import { TaskProgress } from "./TaskProgress";

interface RenameModalProps {
  open: boolean;
  systemPath: string;
  /** Solo estos juegos (uno desde el editor, los marcados en la selección múltiple); sin nada, todos */
  gamePaths?: string[];
  onClose: () => void;
  /** Tras aplicar: ruta antigua → nueva de los juegos cuyo fichero cambió */
  onDone?: (moved: Record<string, string>) => void;
}

/** "./sub/Juego (USA).sfc" + "Juego" → "./sub/Juego.sfc" (solo cambia el nombre, no la carpeta) */
function pathWithName(entry: RenameEntry, newName: string): string {
  const slash = entry.current_path.lastIndexOf("/");
  const fileName = entry.current_path.slice(slash + 1);
  const ext = fileName.slice(entry.current_name.length);
  return entry.current_path.slice(0, slash + 1) + newName + ext;
}

/**
 * Renombrar: una sola ventana para el nombre de fichero y el título de la gamelist. A la
 * izquierda las opciones (fuentes en cadena, reglas, etiquetas, artículo); a la derecha lo que
 * cambia en cada juego, editable a mano. Sirve para todo el sistema, los marcados o un juego.
 */
export function RenameModal({ open, systemPath, gamePaths, onClose, onDone }: RenameModalProps) {
  const { t } = useTranslation();
  const loadGames = useAppStore((s) => s.loadGames);
  const scraperStatus = useAppStore((s) => s.scraperStatus);
  const savedOptions = useAppStore((s) => s.settings.rename_options);
  const patchSettings = useAppStore((s) => s.patchSettings);
  const [options, setOptionsState] = useState<RenameOptions>(() => renameOptionsFrom(savedOptions));
  const [entries, setEntries] = useState<RenameEntry[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  // Lo escrito a mano (por ruta): se pierde al cambiar las opciones, que rehacen la propuesta
  const [files, setFiles] = useState<Record<string, string>>({});
  const [titles, setTitles] = useState<Record<string, string>>({});
  const previewTask = useTask();
  const renameTask = useTask();
  const busy = previewTask.running || renameTask.running;
  // Cada cambio de opciones rehace la vista previa (sin preguntar al scraper: eso es «Buscar»)
  const debouncedOptions = useDebounce(options, 250);
  // Cada vista previa lleva un número: si llega una vieja después de otra más nueva, no cuenta
  const lastAsk = useRef(0);
  const single = gamePaths?.length === 1;

  function setOptions(next: RenameOptions) {
    setOptionsState(next);
    patchSettings({ rename_options: next });
  }

  // Al abrir: las opciones guardadas
  useEffect(() => {
    if (open) setOptionsState(renameOptionsFrom(useAppStore.getState().settings.rename_options));
    else {
      setEntries([]);
      setSelected(new Set());
    }
  }, [open]);

  // Lo que cambia la propuesta rehace la vista previa: también el artículo y los interruptores,
  // porque el backend repite fichero y título hasta que no cambian (y uno depende del otro)
  const request = useMemo(
    () => previewRequest(debouncedOptions, scraperStatus),
    [debouncedOptions, scraperStatus]
  );
  const requestKey = JSON.stringify(request);

  useEffect(() => {
    if (!open) return;
    preview(false);
    // Al cerrar o cambiar de opciones a medias, la vista previa anterior se para
    return () => previewTask.cancel();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, systemPath, gamePaths?.join("\n"), requestKey]);

  async function preview(askScraper: boolean) {
    const ask = ++lastAsk.current;
    try {
      const result = await previewTask.run((task) => renamePreview(systemPath, request, gamePaths, askScraper, task));
      // Cancelado (o ya hay otra más nueva): se queda lo que había
      if (!result || ask !== lastAsk.current) return;
      setEntries(result);
      setFiles({});
      setTitles({});
    } catch (e) {
      console.error("Rename preview failed:", e);
      await showMessage(t("rename.failed", { error: String(e) }), "error");
    }
  }

  // Lo que se haría con cada juego, según las opciones, los interruptores y lo escrito a mano
  const rows = useMemo(
    () => entries.map((entry) => planRename(entry, options, files[entry.current_path], titles[entry.current_path])),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [entries, files, titles, options.renameFiles, options.changeTitles, options.article]
  );
  // Se enseñan los que cambian en algo (aunque sea con un conflicto); con un solo juego, siempre
  const shown = rows.filter((r) => single || r.fileChanges || r.titleChanges);

  // Al llegar una lista nueva se marcan todos los que cambian
  useEffect(() => {
    setSelected(new Set(rows.filter((r) => r.changes).map((r) => r.entry.current_path)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entries, options.renameFiles, options.changeTitles, options.article]);

  const applicable = rows.filter((r) => r.changes && selected.has(r.entry.current_path));
  const fileCount = applicable.filter((r) => r.fileChanges && !r.blocked).length;
  const titleCount = applicable.filter((r) => r.titleChanges).length;
  const conflictCount = rows.filter((r) => r.fileChanges && r.blocked).length;
  const scraperOn = activeSources(options, scraperStatus).includes("scraper");
  const scraperPending = scraperOn ? entries.filter((e) => e.pending || e.unanswered).length : 0;
  const unanswered = scraperOn ? entries.filter((e) => e.unanswered).length : 0;

  function toggle(path: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  function toggleAll() {
    const all = shown.filter((r) => r.changes).map((r) => r.entry.current_path);
    setSelected(all.every((p) => selected.has(p)) ? new Set() : new Set(all));
  }

  async function apply() {
    const renames: RenameRequest[] = applicable.map(renameRequestOf);
    if (renames.length === 0) return;
    try {
      const result = await renameTask.run((task) => renameRoms(systemPath, renames, task));
      if (!result) return;
      await loadGames(systemPath);
      const moved: Record<string, string> = {};
      for (const r of applicable) {
        if (r.fileChanges && !r.blocked && !result.skipped.some((s) => r.entry.current_path.endsWith(s))) {
          moved[r.entry.current_path] = pathWithName(r.entry, r.file);
        }
      }
      onDone?.(moved);
      let message = [t("rename.done", { count: result.renamed }), t("rename.doneTitles", { count: result.titled })].join("\n");
      if (renameTask.wasCancelled()) message = t("task.stopped", { done: result.renamed, total: renames.length }) + "\n\n" + message;
      if (result.skipped.length > 0) {
        message += "\n\n" + t("rename.skipped", { count: result.skipped.length }) + "\n" + result.skipped.join("\n");
      }
      await reportWithUndo(message);
      onClose();
    } catch (e) {
      await showMessage(t("rename.failed", { error: String(e) }), "error");
    }
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-[60]" data-editor-layer>
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[1100px] max-w-[96vw] h-[86vh] flex flex-col mx-4">
        {/* Cabecera */}
        <div className="px-5 py-3 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("rename.title")}</h3>
          <p className="text-xs text-zinc-500 mt-0.5">
            {gamePaths ? t("rename.scopeGames", { count: gamePaths.length }) : t("rename.scopeSystem")}
          </p>
        </div>

        <div className="flex-1 flex min-h-0">
          {/* Opciones */}
          <aside className="w-80 shrink-0 border-r border-zinc-700 p-4 overflow-y-auto">
            <RenameOptionsPanel
              options={options}
              onChange={setOptions}
              scraperPending={scraperPending}
              onSearch={() => preview(true)}
              disabled={renameTask.running}
            />
          </aside>

          {/* Propuestas */}
          <div className="flex-1 flex flex-col min-w-0">
            <div className="px-4 py-2 border-b border-zinc-700/60 flex flex-wrap items-center gap-x-4 gap-y-1 text-sm">
              <label className="flex items-center gap-2 cursor-pointer text-zinc-300">
                <input
                  type="checkbox"
                  checked={options.renameFiles}
                  disabled={busy}
                  onChange={() => setOptions({ ...options, renameFiles: !options.renameFiles })}
                  className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                />
                {t("rename.renameFiles")}
              </label>
              <label className="flex items-center gap-2 cursor-pointer text-zinc-300">
                <input
                  type="checkbox"
                  checked={options.changeTitles}
                  disabled={busy}
                  onChange={() => setOptions({ ...options, changeTitles: !options.changeTitles })}
                  className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                />
                {t("rename.changeTitles")}
              </label>
              <span className="flex-1" />
              {!busy && (
                <span className="text-xs text-zinc-400">
                  {t("rename.summary", { files: fileCount, titles: titleCount })}
                  {conflictCount > 0 && <span className="text-amber-400"> · {t("rename.conflicts", { count: conflictCount })}</span>}
                </span>
              )}
            </div>
            {/* Avisos del scraper: ROMs sin preguntar todavía o que no contestaron */}
            {!busy && scraperPending > 0 && (
              <p className="px-4 py-1.5 text-xs text-amber-400 border-b border-zinc-700/60">
                {unanswered > 0 ? t("rename.scraperUnanswered", { count: unanswered }) + " " : ""}
                {t("rename.scraperPending", { count: scraperPending })}
              </p>
            )}

            <div className="flex-1 overflow-y-auto">
              {renameTask.running ? (
                <TaskProgress progress={renameTask.progress} onCancel={renameTask.cancel} cancelling={renameTask.cancelling} />
              ) : previewTask.running ? (
                <TaskProgress progress={previewTask.progress} label={t("rename.scanning")} onCancel={previewTask.cancel} cancelling={previewTask.cancelling} />
              ) : shown.length === 0 ? (
                <div className="flex items-center justify-center py-12 text-zinc-500 text-sm px-8 text-center">{t("rename.nothing")}</div>
              ) : (
                <>
                  <div className="grid grid-cols-[auto_1fr_1fr_1fr] gap-3 px-4 py-2 text-xs text-zinc-500 font-medium border-b border-zinc-700/50 sticky top-0 bg-zinc-800 z-10">
                    <input
                      type="checkbox"
                      checked={shown.some((r) => r.changes) && shown.filter((r) => r.changes).every((r) => selected.has(r.entry.current_path))}
                      onChange={toggleAll}
                      title={t("rename.selectAll")}
                      className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                    />
                    <span>{t("rename.currentName")}</span>
                    <span>{t("rename.newName")}</span>
                    <span>{t("rename.newTitle")}</span>
                  </div>
                  <div className="divide-y divide-zinc-700/30">
                    {shown.map((r) => {
                      const { entry } = r;
                      const path = entry.current_path;
                      return (
                        <div key={path} className={`grid grid-cols-[auto_1fr_1fr_1fr] gap-3 px-4 py-2 items-start ${r.changes ? "" : "opacity-50"}`}>
                          <input
                            type="checkbox"
                            checked={r.changes && selected.has(path)}
                            disabled={!r.changes}
                            onChange={() => toggle(path)}
                            className="mt-1.5 rounded border-zinc-600 bg-zinc-700 text-indigo-600 disabled:opacity-30"
                          />
                          {/* Ahora: fichero y título */}
                          <div className="min-w-0">
                            <p className="text-sm text-zinc-300 truncate" title={entry.current_name}>{entry.current_name}</p>
                            <p className="text-xs text-zinc-500 truncate" title={entry.game_title}>{entry.game_title || "—"}</p>
                          </div>
                          {/* Fichero nuevo, de qué fuente sale y si choca */}
                          <div className="min-w-0">
                            <input
                              type="text"
                              value={r.file}
                              disabled={!options.renameFiles || !hasExtension(entry)}
                              onChange={(e) => setFiles((prev) => ({ ...prev, [path]: e.target.value }))}
                              className={`w-full bg-zinc-900 border rounded px-2 py-1 text-sm focus:outline-none focus:border-indigo-500 disabled:opacity-50 ${
                                r.fileChanges && r.blocked
                                  ? "border-amber-500/70 text-amber-200"
                                  : r.fileChanges
                                    ? "border-indigo-500/50 text-indigo-300"
                                    : "border-zinc-600 text-zinc-400"
                              }`}
                            />
                            <div className="flex flex-wrap gap-1 mt-1">
                              <span className="text-[10px] px-1.5 py-0.5 rounded bg-zinc-700 text-zinc-300">
                                {entry.source === "current" ? t("rename.sourceCurrent") : t(nameSourceText(entry.source).label)}
                              </span>
                              {!!entry.tracks && (
                                <span className="text-[10px] px-1.5 py-0.5 rounded bg-zinc-700 text-zinc-300" title={t("rename.tracksHelp")}>
                                  {t("rename.tracks", { count: entry.tracks })}
                                </span>
                              )}
                              {r.fileChanges && r.invalid && (
                                <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-600/20 text-amber-300" title={t("rename.invalidHelp")}>
                                  {t("rename.invalid")}
                                </span>
                              )}
                              {r.fileChanges && !r.invalid && r.blocked && (
                                <span
                                  className="text-[10px] px-1.5 py-0.5 rounded bg-amber-600/20 text-amber-300"
                                  title={t(entry.conflict === "duplicate" ? "rename.conflictDuplicateHelp" : "rename.conflictExistsHelp")}
                                >
                                  {t(entry.conflict === "duplicate" ? "rename.conflictDuplicate" : "rename.conflictExists")}
                                </span>
                              )}
                            </div>
                          </div>
                          {/* Título nuevo */}
                          <input
                            type="text"
                            value={r.title}
                            disabled={!options.changeTitles}
                            onChange={(e) => setTitles((prev) => ({ ...prev, [path]: e.target.value }))}
                            className={`w-full bg-zinc-900 border rounded px-2 py-1 text-sm focus:outline-none focus:border-indigo-500 disabled:opacity-50 ${
                              r.titleChanges ? "border-indigo-500/50 text-indigo-300" : "border-zinc-600 text-zinc-400"
                            }`}
                          />
                        </div>
                      );
                    })}
                  </div>
                </>
              )}
            </div>
          </div>
        </div>

        {/* Pie */}
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button onClick={onClose} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors">
            {t("rename.close")}
          </button>
          <button
            onClick={apply}
            disabled={busy || applicable.length === 0}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {renameTask.running ? "..." : t("rename.apply", { count: applicable.length })}
          </button>
        </div>
      </div>
    </div>
  );
}
