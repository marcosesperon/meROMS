import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { importRoms, planImport, type ImportGame, type ImportSource } from "../../commands";
import { useTask } from "../../hooks/useTask";
import { useAppStore } from "../../store/useAppStore";
import type { MigrationMode } from "../../types";
import { collectionRulesFrom } from "../../utils/collectionRules";
import { showMessage } from "../../utils/dialogs";
import { formatBytes } from "../../utils/format";
import { reportWithUndo } from "../../utils/undo";
import { TaskProgress } from "./TaskProgress";

interface ImportModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
}

type MediaChoice = "all" | "required" | "none";

/** Color del aviso de cada estado */
const STATUS_STYLE: Record<ImportGame["status"], string> = {
  new: "text-emerald-400 bg-emerald-400/10",
  same_rom: "text-zinc-400 bg-zinc-600/30",
  other_version: "text-amber-400 bg-amber-400/10",
  taken: "text-red-400 bg-red-400/10",
};
/** Misma ROM que aporta algo: se completa tu juego */
const COMPLETE_STYLE = "text-teal-300 bg-teal-400/10";

/** Medios que contarían con la elección de medios (todos / obligatorios / ninguno) */
function mediaFor(g: ImportGame, choice: MediaChoice, required: string[]): string[] {
  if (choice === "none") return [];
  return choice === "all" ? g.media : g.media.filter((f) => required.includes(f));
}

/** Lo que aporta a tu juego uno que ya tienes: datos vacíos + medios que le faltan */
function adds(g: ImportGame, choice: MediaChoice, required: string[]): number {
  return g.fills + mediaFor(g, choice, required).length;
}

/** Se puede marcar: lo nuevo, lo nombre ocupado no, y lo que ya tienes solo si aporta algo */
function selectable(g: ImportGame, choice: MediaChoice, required: string[]): boolean {
  if (g.status === "taken") return false;
  if (g.status === "same_rom") return !!g.mine && adds(g, choice, required) > 0;
  return true;
}

/**
 * «Importar ROMs…»: trae al sistema abierto las ROMs de otra carpeta, con su estructura (clásica,
 * ES-DE o sueltas). Primero se elige la carpeta y sale la vista previa juego a juego (nuevo,
 * la misma ROM, otra versión, nombre ocupado); después, copiar o mover y qué medios. Al
 * terminar, resumen con Deshacer y, si se marca, «Ponerlo todo en orden» con los importados
 * (solo se abre, no se aplica nada).
 */
export function ImportModal({ open, systemPath, onClose }: ImportModalProps) {
  const { t } = useTranslation();
  const settings = useAppStore((s) => s.settings);
  const [folder, setFolder] = useState<string | null>(null);
  const [esdeHome, setEsdeHome] = useState<string | null>(null);
  const [source, setSource] = useState<ImportSource | null>(null);
  const [games, setGames] = useState<ImportGame[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [mode, setMode] = useState<MigrationMode>("copy");
  const [media, setMedia] = useState<MediaChoice>("required");
  const [thenOrder, setThenOrder] = useState(false);
  // Ficheros con extensión que no es de este sistema: ocultos hasta marcar la casilla
  const [showUnknown, setShowUnknown] = useState(false);
  const required = useMemo(() => collectionRulesFrom(settings.collection_rules).required_media, [settings.collection_rules]);
  const scan = useTask();
  const apply = useTask();

  // Al cerrar, todo como al principio
  useEffect(() => {
    if (open) return;
    setFolder(null);
    setEsdeHome(null);
    setSource(null);
    setGames([]);
    setSelected(new Set());
    setMode("copy");
    setMedia("required");
    setThenOrder(false);
    setShowUnknown(false);
  }, [open]);

  // La vista previa, cada vez que cambia la carpeta (o la de ES-DE elegida a mano)
  useEffect(() => {
    if (!open || !folder) return;
    scan
      .run((task) => planImport(systemPath, folder, esdeHome, task))
      .then((plan) => {
        if (!plan) return;
        setSource(plan.source);
        setGames(plan.games);
        setShowUnknown(false);
        // Marcados: todo lo que se puede marcar con extensión de este sistema (los que ya tienes,
        // si aportan algo con los medios elegidos)
        setSelected(new Set(plan.games.filter((g) => g.known_extension && selectable(g, media, required)).map((g) => g.path)));
      })
      .catch((e) => showMessage(String(e), "error"));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, folder, esdeHome, systemPath]);

  // La lista que se ve: sin los de extensión desconocida, salvo con la casilla
  const shown = useMemo(() => games.filter((g) => g.known_extension || showUnknown), [games, showUnknown]);
  const unknown = useMemo(() => games.filter((g) => !g.known_extension), [games]);
  const unknownExts = useMemo(() => {
    const exts = [...new Set(unknown.map((g) => (g.path.match(/\.[^./]+$/)?.[0] ?? "").toLowerCase()).filter(Boolean))];
    return exts.slice(0, 5).join(", ") + (exts.length > 5 ? "..." : "");
  }, [unknown]);

  const counts = useMemo(() => {
    const by = (s: ImportGame["status"]) => shown.filter((g) => g.status === s).length;
    return { new: by("new"), same_rom: by("same_rom"), other_version: by("other_version"), taken: by("taken") };
  }, [shown]);

  // Lo que de verdad se hará: los marcados que se ven y se pueden marcar con los medios elegidos
  const chosen = shown.filter((g) => selected.has(g.path) && selectable(g, media, required));
  const toImport = chosen.filter((g) => g.status !== "same_rom");
  const toComplete = chosen.filter((g) => g.status === "same_rom");

  if (!open) return null;

  async function pickFolder() {
    const picked = await openDialog({ directory: true, multiple: false });
    if (typeof picked === "string") {
      setEsdeHome(null);
      setFolder(picked);
    }
  }

  async function pickEsde() {
    const picked = await openDialog({ directory: true, multiple: false });
    if (typeof picked === "string") setEsdeHome(picked);
  }

  function toggleUnknown(on: boolean) {
    setShowUnknown(on);
    // Al enseñarlos salen marcados; al ocultarlos se desmarcan
    setSelected((prev) => {
      const next = new Set(prev);
      for (const g of unknown) {
        if (on && selectable(g, media, required)) next.add(g.path);
        else next.delete(g.path);
      }
      return next;
    });
  }

  function toggle(path: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  async function run() {
    if (!folder || chosen.length === 0) return;
    try {
      const paths = toImport.map((g) => g.path);
      const complete = toComplete.map((g) => ({ path: g.path, mine: g.mine! }));
      const result = await apply.run((task) => importRoms(systemPath, folder, esdeHome, paths, complete, { mode, media, required }, task));
      if (!result) return;
      const { loadGames, loadSystems, openTool } = useAppStore.getState();
      await loadGames(systemPath, { mediaChanged: true });
      loadSystems();
      onClose();
      const lines = [t(result.stopped ? "import.stopped" : "import.done", { count: result.imported.length, media: result.media })];
      if (result.completed.length) lines.push(t("import.completed", { count: result.completed.length }));
      if (result.skipped.length) lines.push("", t("import.skipped", { count: result.skipped.length }), ...result.skipped.slice(0, 20));
      await reportWithUndo(lines.join("\n"));
      // Solo se abre la ventana con los importados: no se aplica nada
      if (thenOrder && result.imported.length) openTool({ id: "order", gamePaths: result.imported });
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  const busy = scan.running || apply.running;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[760px] max-w-[calc(100vw-2rem)] max-h-[88vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("import.title")}</h3>
          <p className="text-sm text-zinc-400 mt-1">{t("import.description")}</p>
          <div className="flex items-center gap-3 mt-3">
            <button onClick={pickFolder} disabled={busy} className="px-3 py-1.5 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
              {t(folder ? "import.changeFolder" : "import.pickFolder")}
            </button>
            {folder && <span className="text-xs text-zinc-400 font-mono truncate">{folder}</span>}
          </div>
          {source && !scan.running && (
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1 mt-2 text-xs">
              <span className="text-zinc-300">{t(`import.kind_${source.kind}` as TranslationKeys)}</span>
              {source.esde_home && <span className="text-zinc-500 font-mono truncate">{source.esde_home}</span>}
              {source.kind !== "classic" && (
                <button onClick={pickEsde} className="text-indigo-400 hover:text-indigo-300">
                  {t("import.pickEsde")}
                </button>
              )}
            </div>
          )}
        </div>

        <div className="flex-1 overflow-y-auto min-h-[120px]">
          {apply.running ? (
            <TaskProgress progress={apply.progress} onCancel={apply.cancel} cancelling={apply.cancelling} />
          ) : scan.running ? (
            <TaskProgress progress={scan.progress} label={t("import.scanning")} onCancel={scan.cancel} cancelling={scan.cancelling} />
          ) : !folder ? (
            <div className="py-12 px-8 text-center text-sm text-zinc-500">{t("import.pickHint")}</div>
          ) : games.length === 0 || (shown.length === 0 && unknown.length === 0) ? (
            <div className="py-12 px-8 text-center text-sm text-zinc-500">{t("import.nothing")}</div>
          ) : (
            <>
              {unknown.length > 0 && (
                <label className="flex items-center gap-2 px-5 py-2 text-xs text-amber-300/90 bg-amber-400/5 border-b border-zinc-700/50 cursor-pointer">
                  <input type="checkbox" checked={showUnknown} onChange={(e) => toggleUnknown(e.target.checked)} />
                  {t("import.showUnknown", { count: unknown.length, exts: unknownExts })}
                </label>
              )}
              <div className="flex flex-wrap items-center gap-x-4 gap-y-1 px-5 py-2 text-xs text-zinc-500 border-b border-zinc-700/50">
                {(["new", "other_version", "same_rom", "taken"] as const).map((s) =>
                  counts[s] ? (
                    <span key={s} className={`px-2 py-0.5 rounded ${STATUS_STYLE[s]}`}>
                      {t(`import.count_${s}` as TranslationKeys, { count: counts[s] })}
                    </span>
                  ) : null
                )}
                <button onClick={() => setSelected(new Set(shown.filter((g) => selectable(g, media, required)).map((g) => g.path)))} className="text-indigo-400 hover:text-indigo-300">
                  {t("rename.selectAll")}
                </button>
                <button onClick={() => setSelected(new Set())} className="text-indigo-400 hover:text-indigo-300">
                  {t("rename.deselectAll")}
                </button>
              </div>
              <div className="divide-y divide-zinc-700/30">
                {shown.map((g) => {
                  const can = selectable(g, media, required);
                  const completes = g.status === "same_rom" && can;
                  const badge = completes
                    ? t("import.status_complete", { other: g.other ?? "", fields: g.fills, media: mediaFor(g, media, required).length })
                    : g.status === "same_rom"
                      ? t("import.status_nothing", { other: g.other ?? "" })
                      : t(`import.status_${g.status}` as TranslationKeys, { other: g.other ?? "" });
                  return (
                    <label key={g.path} className={`flex items-center gap-3 px-5 py-1.5 ${can ? "cursor-pointer hover:bg-zinc-700/30" : "opacity-60"}`}>
                      <input
                        type="checkbox"
                        checked={can && selected.has(g.path)}
                        disabled={!can}
                        onChange={() => toggle(g.path)}
                        className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
                      />
                      <div className="flex-1 min-w-0">
                        <p className="text-sm text-zinc-200 truncate">{g.name}</p>
                        <p className="text-xs text-zinc-500 font-mono truncate">
                          {g.path} · {formatBytes(g.bytes)}
                          {g.status !== "same_rom" && g.media.length > 0 && ` · ${t("import.mediaCount", { count: g.media.length })}`}
                        </p>
                      </div>
                      {!g.known_extension && <span className="text-xs px-2 py-0.5 rounded shrink-0 text-amber-300 bg-amber-400/10">{t("import.unknownExt")}</span>}
                      {g.status !== "new" && (
                        <span className={`text-xs px-2 py-0.5 rounded shrink-0 max-w-[45%] truncate ${completes ? COMPLETE_STYLE : STATUS_STYLE[g.status]}`} title={g.other ?? ""}>
                          {badge}
                        </span>
                      )}
                    </label>
                  );
                })}
              </div>
            </>
          )}
        </div>

        {games.length > 0 && !busy && (
          <div className="border-t border-zinc-700 px-5 py-3 grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm text-zinc-300">
            <div className="flex flex-col gap-1">
              <span className="text-xs text-zinc-500">{t("import.modeLabel")}</span>
              {(["copy", "move"] as const).map((m) => (
                <label key={m} className="flex items-center gap-2 cursor-pointer">
                  <input type="radio" checked={mode === m} onChange={() => setMode(m)} />
                  {t(m === "copy" ? "import.copy" : "import.move")}
                </label>
              ))}
            </div>
            <div className="flex flex-col gap-1">
              <span className="text-xs text-zinc-500">{t("import.mediaLabel")}</span>
              {(["all", "required", "none"] as const).map((m) => (
                <label key={m} className="flex items-center gap-2 cursor-pointer">
                  <input type="radio" checked={media === m} onChange={() => setMedia(m)} />
                  {t(`import.media_${m}` as TranslationKeys)}
                </label>
              ))}
            </div>
            <label className="sm:col-span-2 flex items-center gap-2 cursor-pointer">
              <input type="checkbox" checked={thenOrder} onChange={(e) => setThenOrder(e.target.checked)} />
              {t("import.thenOrder")}
            </label>
          </div>
        )}

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button onClick={onClose} disabled={apply.running} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-50">
            {t("rename.close")}
          </button>
          {games.length > 0 && (
            <button
              onClick={run}
              disabled={chosen.length === 0 || busy}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {toImport.length === 0
                ? t("import.runComplete", { count: toComplete.length })
                : toComplete.length === 0
                  ? t(mode === "copy" ? "import.runCopy" : "import.runMove", { count: toImport.length })
                  : t(mode === "copy" ? "import.runCopyComplete" : "import.runMoveComplete", { count: toImport.length, complete: toComplete.length })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
