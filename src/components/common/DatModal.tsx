import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import { clearSystemDat, datComputeCrcs, datReport, saveGamelist, setSystemDat, trashFiles, type DatMissing, type DatReport } from "../../commands";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "./TaskProgress";
import { CloseButton } from "./CloseButton";
import { ProcessingLabel } from "./ProcessingLabel";
import { useBlockClose } from "../../hooks/useBlockClose";

interface DatModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
  /** The DAT changed or the games/files changed (refresh marks, list, counters) */
  onChanged: () => void;
}

type Tab = "missing" | "unknown" | "names";

/**
 * Herramienta «DAT…»: asociar un DAT al sistema y ver qué juegos faltan, qué ficheros no
 * conoce el DAT y qué nombres se pueden poner desde el DAT.
 */
export function DatModal({ open, systemPath, onClose, onChanged }: DatModalProps) {
  const { t } = useTranslation();
  const [report, setReport] = useState<DatReport | null>(null);
  // El informe: con progreso y Cancelar
  const reportTask = useTask();
  const loading = reportTask.running;
  // Qué acción está en marcha (elegir DAT, papelera, nombres): su botón dice «Procesando…»
  const [doing, setDoing] = useState<"pick" | "trash" | "names" | null>(null);
  const busy = doing !== null;
  // Mandar a la papelera los ficheros que el DAT no conoce: con progreso y Cancelar
  const trashTask = useTask();
  const [tab, setTab] = useState<Tab>("missing");
  // Faltan: cada versión por separado (la lista de antes) o un juego por fila con sus versiones
  const [eachVersion, setEachVersion] = useState(false);
  // Betas, prototipos, demos, hacks…: fuera salvo que se pidan
  const [withUnwanted, setWithUnwanted] = useState(false);
  // Juegos desplegados (por el nombre de su versión preferida)
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [unknownSel, setUnknownSel] = useState<Set<string>>(new Set());
  const [nameSel, setNameSel] = useState<Set<string>>(new Set());
  // «Comprobar por contenido» en marcha: fichero actual / total
  const [crcProgress, setCrcProgress] = useState<{ current: number; total: number; file: string } | null>(null);
  // Mientras se aplica algo (o se calculan las huellas) la ventana no se puede cerrar
  useBlockClose(busy || crcProgress !== null);

  async function load() {
    try {
      // Envuelto: el informe puede ser null (sistema sin DAT), y null en `run` es «cancelado»
      const res = await reportTask.run(async (task) => ({ found: await datReport(systemPath, task) }));
      // Cancelado: no hay nada que enseñar
      if (!res) return onClose();
      const r = res.found;
      setReport(r);
      setUnknownSel(new Set());
      setNameSel(new Set(r?.names.map((n) => n.game_path) ?? []));
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  useEffect(() => {
    // Siempre empieza en «Faltan» (también al abrirla desde la barra de Colección del resumen)
    if (open) {
      setTab("missing");
      setExpanded(new Set());
      load();
    } else setReport(null);
    // Al cerrar la ventana a medias, el informe se para
    return () => reportTask.cancel();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, systemPath]);

  const wanted = (m: DatMissing) => withUnwanted || !m.unwanted;
  const missing = useMemo(() => (report?.missing ?? []).filter(wanted), [report, withUnwanted]); // eslint-disable-line react-hooks/exhaustive-deps
  // Un juego por fila; sin los no deseados, tampoco se enseñan sus versiones no deseadas
  const groups = useMemo(
    () =>
      (report?.missing_groups ?? [])
        .filter((g) => withUnwanted || !g.unwanted)
        .map((g) => ({ ...g, versions: g.versions.filter(wanted) })),
    [report, withUnwanted] // eslint-disable-line react-hooks/exhaustive-deps
  );

  if (!open) return null;

  async function pickDat() {
    const file = await openDialog({ multiple: false, filters: [{ name: "DAT", extensions: ["dat", "xml"] }] });
    if (!file) return;
    setDoing("pick");
    try {
      await setSystemDat(systemPath, String(file));
      onChanged();
      await load();
    } catch (e) {
      await showMessage(t("dat.loadFailed", { error: String(e) }), "error");
    } finally {
      setDoing(null);
    }
  }

  /** Calcula las huellas que falten (los sueltos, la primera vez, se leen enteros) y recarga el informe */
  async function checkByContent() {
    setCrcProgress({ current: 0, total: 0, file: "" });
    const unlisten = await listen<{ current: number; total: number; file: string }>("dat-crc-progress", (e) => setCrcProgress(e.payload));
    try {
      await datComputeCrcs(systemPath);
      onChanged();
      await load();
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      unlisten();
      setCrcProgress(null);
    }
  }

  async function removeDat() {
    if (!(await askConfirm(t("dat.removeConfirm")))) return;
    await clearSystemDat(systemPath);
    onChanged();
    setReport(null);
  }

  async function trashUnknown() {
    const paths = [...unknownSel];
    if (!(await askConfirm(t("newRoms.trashConfirm", { count: paths.length })))) return;
    setDoing("trash");
    try {
      const result = await trashTask.run((task) => trashFiles(systemPath, paths, task));
      if (!result) return;
      if (trashTask.wasCancelled()) await showMessage(t("task.stopped", { done: result.trashed, total: paths.length }));
      if (result.failed.length > 0) {
        await showMessage(t("newRoms.trashFailed", { count: result.failed.length }) + "\n" + result.failed.join("\n"), "error");
      }
      onChanged();
      await load();
    } finally {
      setDoing(null);
    }
  }

  async function applyNames() {
    if (!report) return;
    const names = new Map(report.names.filter((n) => nameSel.has(n.game_path)).map((n) => [n.game_path, n.proposed]));
    setDoing("names");
    try {
      const { games, folders, provider, setGames } = useAppStore.getState();
      const updated = games.map((g) => (names.has(g.path) ? { ...g, name: names.get(g.path) } : g));
      await saveGamelist(systemPath, { games: updated, folders: folders || [], provider });
      setGames(updated);
      onChanged();
      await load();
    } catch (e) {
      await showMessage(t("detail.saveFailed", { error: String(e) }), "error");
    } finally {
      setDoing(null);
    }
  }

  function copyMissing() {
    // Lo que se ve: cada versión, o la preferida de cada juego
    const rows = eachVersion ? missing : groups.map((g) => g.versions[0]);
    const text = rows.map((m) => `${m.name}\t${m.description}${m.year ? `\t${m.year}` : ""}`).join("\n");
    navigator.clipboard.writeText(text).catch(() => {});
  }

  const toggle = (set: Set<string>, setter: (s: Set<string>) => void, key: string) => {
    const next = new Set(set);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setter(next);
  };

  const tabs: { id: Tab; label: string; count: number }[] = report
    ? [
        { id: "missing", label: t("dat.tabMissing"), count: eachVersion ? missing.length : groups.length },
        { id: "unknown", label: t("dat.tabUnknown"), count: report.unknown_files.length },
        { id: "names", label: t("dat.tabNames"), count: report.names.length },
      ]
    : [];

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[760px] h-[80vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("dat.title")}</h3>
          <div className="flex items-center justify-between gap-3 mt-2">
            <p className="text-sm text-zinc-400 min-w-0 truncate">
              {report
                ? t("dat.current", { name: report.summary.name, games: report.summary.games, clones: report.summary.clones })
                : t("dat.none")}
            </p>
            <div className="flex gap-2 shrink-0">
              <button onClick={pickDat} disabled={busy} className="px-3 py-1.5 text-xs bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
                {doing === "pick" ? <ProcessingLabel /> : t("dat.pick")}
              </button>
              {report && (
                <button onClick={removeDat} className="px-3 py-1.5 text-xs bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
                  {t("dat.remove")}
                </button>
              )}
            </div>
          </div>
          {!report && <p className="text-xs text-zinc-500 mt-2">{t("dat.help")}</p>}
          {/* Reconocer por contenido (CRC): con barra mientras calcula */}
          {report && (
            <div className="flex items-center gap-3 mt-2 text-xs">
              {!report.summary.has_crcs ? (
                <p className="text-amber-300">{t("dat.noCrcs")}</p>
              ) : crcProgress ? (
                <div className="flex-1 min-w-0">
                  <TaskProgress
                    progress={{ phase: "hashing", current: crcProgress.current, total: crcProgress.total, item: crcProgress.file }}
                    compact
                  />
                </div>
              ) : (
                <>
                  <p className="flex-1 text-zinc-400">{t("dat.contentMatched", { count: report.content_matched })}</p>
                  <button
                    onClick={checkByContent}
                    disabled={busy}
                    className="px-3 py-1.5 bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600 disabled:opacity-50"
                    title={t("dat.checkContentHelp")}
                  >
                    {t("dat.checkContent")}
                  </button>
                </>
              )}
            </div>
          )}
          {report && (
            <div className="flex gap-1 mt-3">
              {tabs.map((x) => (
                <button
                  key={x.id}
                  onClick={() => setTab(x.id)}
                  className={`px-3 py-1.5 text-xs rounded ${tab === x.id ? "bg-zinc-700 text-zinc-100" : "text-zinc-400 hover:text-zinc-200"}`}
                >
                  {x.label} <span className="text-zinc-500">{x.count}</span>
                </button>
              ))}
            </div>
          )}
        </div>

        <div className="flex-1 overflow-y-auto">
          {trashTask.running ? (
            <TaskProgress progress={trashTask.progress} onCancel={trashTask.cancel} cancelling={trashTask.cancelling} />
          ) : loading ? (
            <TaskProgress progress={reportTask.progress} label={t("newRoms.scanning")} onCancel={reportTask.cancel} cancelling={reportTask.cancelling} />
          ) : !report ? null : tab === "missing" ? (
            <>
              <div className="px-5 py-2 flex items-center gap-4 border-b border-zinc-700/50 text-xs">
                <label className="flex items-center gap-1.5 text-zinc-400 cursor-pointer">
                  <input type="checkbox" checked={eachVersion} onChange={(e) => setEachVersion(e.target.checked)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                  {t("dat.eachVersion")}
                </label>
                <label className="flex items-center gap-1.5 text-zinc-400 cursor-pointer">
                  <input type="checkbox" checked={withUnwanted} onChange={(e) => setWithUnwanted(e.target.checked)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                  {t("dat.includeUnwanted")}
                </label>
                <button onClick={copyMissing} className="ml-auto text-indigo-400 hover:text-indigo-300">{t("dat.copyList")}</button>
              </div>
              {(eachVersion ? missing.length : groups.length) === 0 && (
                <p className="py-12 text-center text-sm text-emerald-400">{t("dat.noMissing")}</p>
              )}
              <div className="divide-y divide-zinc-700/30">
                {eachVersion
                  ? missing.map((m) => <MissingRow key={m.name} game={m} />)
                  : groups.map((g) => {
                      const [first, ...others] = g.versions;
                      const isOpen = expanded.has(first.name);
                      return (
                        <div key={first.name}>
                          <MissingRow
                            game={first}
                            more={others.length > 0 ? { count: others.length, open: isOpen, toggle: () => toggle(expanded, setExpanded, first.name) } : undefined}
                          />
                          {isOpen && others.map((m) => <MissingRow key={m.name} game={m} nested />)}
                        </div>
                      );
                    })}
              </div>
            </>
          ) : tab === "unknown" ? (
            <div className="divide-y divide-zinc-700/30">
              {report.unknown_files.length === 0 && <p className="py-12 text-center text-sm text-zinc-500">{t("dat.noUnknown")}</p>}
              {report.unknown_files.map((f) => (
                <label key={f} className="flex items-center gap-3 px-5 py-1.5 cursor-pointer hover:bg-zinc-700/30">
                  <input type="checkbox" checked={unknownSel.has(f)} onChange={() => toggle(unknownSel, setUnknownSel, f)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                  <span className="text-sm text-zinc-300 font-mono truncate">{f}</span>
                </label>
              ))}
            </div>
          ) : (
            <div className="divide-y divide-zinc-700/30">
              {report.names.length === 0 && <p className="py-12 text-center text-sm text-zinc-500">{t("dat.noNames")}</p>}
              {report.names.map((n) => (
                <label key={n.game_path} className="flex items-center gap-3 px-5 py-1.5 cursor-pointer hover:bg-zinc-700/30">
                  <input type="checkbox" checked={nameSel.has(n.game_path)} onChange={() => toggle(nameSel, setNameSel, n.game_path)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                  <div className="min-w-0">
                    <p className="text-sm text-zinc-200 truncate">{n.proposed}</p>
                    <p className="text-xs text-zinc-500 truncate">
                      <span className="font-mono">{n.game_path}</span>
                      {n.current && ` · ${n.current}`}
                    </p>
                  </div>
                </label>
              ))}
            </div>
          )}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <CloseButton onClick={onClose}>{t("rename.close")}</CloseButton>
          {report && tab === "unknown" && report.unknown_files.length > 0 && (
            <button onClick={trashUnknown} disabled={unknownSel.size === 0 || busy} className="px-4 py-2 text-sm bg-red-600 text-white rounded hover:bg-red-500 disabled:opacity-50">
              {doing === "trash" ? <ProcessingLabel /> : t("newRoms.trash", { count: unknownSel.size })}
            </button>
          )}
          {report && tab === "names" && report.names.length > 0 && (
            <button onClick={applyNames} disabled={nameSel.size === 0 || busy} className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
              {doing === "names" ? <ProcessingLabel /> : t("dat.applyNames", { count: nameSel.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

/** Una fila de «Faltan»: nombre, y debajo el del fichero, año y fabricante; «+N» despliega las otras versiones */
function MissingRow({ game, more, nested }: { game: DatMissing; more?: { count: number; open: boolean; toggle: () => void }; nested?: boolean }) {
  const { t } = useTranslation();
  return (
    <div className={`flex items-center gap-3 py-1.5 ${nested ? "pl-10 pr-5 bg-zinc-900/30" : "px-5"}`}>
      <div className="min-w-0 flex-1">
        <p className={`truncate ${nested ? "text-xs text-zinc-300" : "text-sm text-zinc-200"}`}>{game.description || game.name}</p>
        <p className="text-xs text-zinc-500 truncate">
          <span className="font-mono">{game.name}</span>
          {game.cloneof && ` · ${t("dat.cloneOf", { parent: game.cloneof })}`}
          {game.year && ` · ${game.year}`}
          {game.manufacturer && ` · ${game.manufacturer}`}
        </p>
      </div>
      {more && (
        <button onClick={more.toggle} className="shrink-0 text-xs text-indigo-400 hover:text-indigo-300">
          {t("dat.moreVersions", { count: more.count })} {more.open ? "▴" : "▾"}
        </button>
      )}
    </div>
  );
}
