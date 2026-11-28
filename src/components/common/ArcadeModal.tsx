import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import {
  applyEmulators,
  arcadeCheck,
  arcadeDelete,
  arcadeDownload,
  arcadeStatus,
  mameVersions,
  type ArcadeCoreId,
  type ArcadeGame,
  type ArcadeReport,
  type ArcadeStatus,
} from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { reportWithUndo } from "../../utils/undo";
import { CORE_NAMES, STATE_COLOR, STATE_MARK, describeCheck } from "../../utils/arcadeSets";
import { useTask } from "../../hooks/useTask";
import { useBlockClose } from "../../hooks/useBlockClose";
import { TaskProgress } from "./TaskProgress";
import { CloseButton } from "./CloseButton";
import { ProcessingLabel } from "./ProcessingLabel";

interface ArcadeModalProps {
  open: boolean;
  systemPath: string;
  /** Solo estos juegos (los marcados o uno); sin nada, el sistema entero */
  onlyPaths?: string[];
  onClose: () => void;
  /** Después de cambiar emuladores (recargar los juegos) */
  onDone: () => void;
}

/**
 * Herramientas → «Emulador por juego» (arcade): con qué núcleo funciona cada set (comparando los
 * CRC de sus chips con el DAT de cada núcleo) y, a los que no les vale su emulador, el que se
 * propone. Arriba, los DAT de cada núcleo (descargar, actualizar, versión de MAME) y su orden.
 */
export function ArcadeModal({ open, systemPath, onlyPaths, onClose, onDone }: ArcadeModalProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ArcadeStatus | null>(null);
  const [report, setReport] = useState<ArcadeReport | null>(null);
  const [onlyProblems, setOnlyProblems] = useState(true);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [applying, setApplying] = useState(false);
  // Descarga de un DAT (una a la vez) y comprobación de los juegos: con progreso y Cancelar
  const [downloading, setDownloading] = useState<ArcadeCoreId | null>(null);
  const downloadTask = useTask();
  const checkTask = useTask();
  useBlockClose(applying || downloading !== null);

  async function loadStatus() {
    setStatus(await arcadeStatus().catch(() => null));
  }

  async function check() {
    try {
      const r = await checkTask.run((task) => arcadeCheck(systemPath, onlyPaths, task));
      if (!r) return onClose();
      setReport(r);
      // Marcados, los que tienen propuesta
      setChecked(new Set(r.games.filter((g) => g.proposal).map((g) => g.path)));
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  useEffect(() => {
    if (!open) {
      setReport(null);
      return;
    }
    loadStatus();
    check();
    return () => checkTask.cancel();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, systemPath, onlyPaths?.join("\n")]);

  const games = useMemo(() => {
    const all = report?.games ?? [];
    return onlyProblems ? all.filter(hasProblem) : all;
  }, [report, onlyProblems]);

  if (!open) return null;

  async function download(id: ArcadeCoreId) {
    setDownloading(id);
    try {
      const r = await downloadTask.run((task) => arcadeDownload(id, task));
      if (!r) return;
      await loadStatus();
      await check();
    } catch (e) {
      await showMessage(t("arcade.downloadFailed", { error: String(e) }), "error");
    } finally {
      setDownloading(null);
    }
  }

  async function remove(id: ArcadeCoreId) {
    if (!(await askConfirm(t("arcade.deleteConfirm", { name: CORE_NAMES[id] })))) return;
    await arcadeDelete(id);
    await loadStatus();
    await check();
  }

  /** Sube o baja un núcleo en el orden de preferencia (se guarda en Ajustes) */
  async function move(id: ArcadeCoreId, by: -1 | 1) {
    if (!status) return;
    const order = [...status.order];
    const i = order.indexOf(id);
    const j = i + by;
    if (i < 0 || j < 0 || j >= order.length) return;
    [order[i], order[j]] = [order[j], order[i]];
    const { settings, updateSettings } = useAppStore.getState();
    await updateSettings({ ...settings, arcade_core_order: order });
    await loadStatus();
    await check();
  }

  async function setMameVersion(version: string) {
    const { settings, updateSettings } = useAppStore.getState();
    await updateSettings({ ...settings, arcade_mame_version: version || undefined });
    await loadStatus();
  }

  async function apply() {
    const changes = (report?.games ?? []).filter((g) => checked.has(g.path) && g.proposal).map((g) => ({ path: g.path, label: g.proposal }));
    if (changes.length === 0) return;
    setApplying(true);
    try {
      const count = await applyEmulators(systemPath, changes);
      onDone();
      await reportWithUndo(t("arcade.applied", { count }));
      await check();
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      setApplying(false);
    }
  }

  const proposals = (report?.games ?? []).filter((g) => g.proposal);
  const selected = proposals.filter((g) => checked.has(g.path)).length;
  const toggle = (path: string) =>
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[900px] max-w-[calc(100vw-2rem)] h-[88vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("arcade.title")}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("arcade.help")}</p>
          {status && (
            <CoresBlock
              status={status}
              downloading={downloading}
              downloadProgress={downloadTask.progress}
              onCancelDownload={downloadTask.cancel}
              onDownload={download}
              onDelete={remove}
              onMove={move}
              onMameVersion={setMameVersion}
            />
          )}
        </div>

        <div className="flex-1 overflow-y-auto">
          {checkTask.running || !report ? (
            <TaskProgress progress={checkTask.progress} label={t("arcade.checking")} onCancel={checkTask.cancel} cancelling={checkTask.cancelling} />
          ) : report.cores.length === 0 ? (
            <p className="py-12 px-8 text-center text-sm text-zinc-400">{t("arcade.noDats")}</p>
          ) : (
            <>
              <div className="px-5 py-2 flex items-center gap-4 border-b border-zinc-700/50 text-xs sticky top-0 bg-zinc-800 z-10">
                <label className="flex items-center gap-1.5 text-zinc-400 cursor-pointer">
                  <input type="checkbox" checked={onlyProblems} onChange={(e) => setOnlyProblems(e.target.checked)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                  {t("arcade.onlyProblems")}
                </label>
                <span className="text-zinc-500">{t("arcade.summary", { games: report.games.length, proposals: proposals.length })}</span>
                {report.custom && <span className="text-amber-300">{t("arcade.customCommand")}</span>}
              </div>
              {games.length === 0 ? (
                <p className="py-12 text-center text-sm text-emerald-400">{t("arcade.allFine")}</p>
              ) : (
                <table className="w-full text-xs">
                  <thead className="text-zinc-500">
                    <tr className="border-b border-zinc-700/50">
                      <th className="w-8" />
                      <th className="text-left font-medium py-1.5">{t("arcade.colGame")}</th>
                      {report.cores.map((id) => (
                        <th key={id} className="font-medium px-1.5 whitespace-nowrap">{CORE_NAMES[id]}</th>
                      ))}
                      <th className="text-left font-medium px-2">{t("arcade.colCurrent")}</th>
                      <th className="text-left font-medium px-2">{t("arcade.colProposal")}</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-zinc-700/30">
                    {games.map((g) => (
                      <GameRow key={g.path} game={g} cores={report.cores} checked={checked.has(g.path)} onToggle={() => toggle(g.path)} />
                    ))}
                  </tbody>
                </table>
              )}
            </>
          )}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex items-center gap-2">
          <CloseButton onClick={onClose}>{t("rename.close")}</CloseButton>
          <div className="flex-1" />
          {proposals.length > 0 && (
            <div className="flex gap-2 text-xs mr-2">
              <button onClick={() => setChecked(new Set(proposals.map((g) => g.path)))} className="text-indigo-300 hover:text-indigo-200">
                {t("cleanup.selectAll")}
              </button>
              <button onClick={() => setChecked(new Set())} className="text-indigo-300 hover:text-indigo-200">
                {t("bulk.selectNone")}
              </button>
            </div>
          )}
          <button
            onClick={apply}
            disabled={applying || selected === 0}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50"
          >
            {applying ? <ProcessingLabel /> : t("arcade.apply", { count: selected })}
          </button>
        </div>
      </div>
    </div>
  );
}

/** Tiene algo que ver: su emulador actual no lo tiene completo, o hay propuesta */
function hasProblem(g: ArcadeGame): boolean {
  if (g.proposal) return true;
  const current = g.current_core ? g.checks[g.current_core] : undefined;
  return !!current && current.state !== "ok" && current.state !== "not_a_game";
}

function GameRow({ game, cores, checked, onToggle }: { game: ArcadeGame; cores: ArcadeCoreId[]; checked: boolean; onToggle: () => void }) {
  const { t } = useTranslation();
  const file = game.path.replace(/^\.\//, "");
  return (
    <tr className="hover:bg-zinc-700/20">
      <td className="text-center">
        {game.proposal && <input type="checkbox" checked={checked} onChange={onToggle} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />}
      </td>
      <td className="py-1.5 pr-2 max-w-0 w-full">
        <p className="text-sm text-zinc-200 truncate">{game.name || file}</p>
        <p className="text-zinc-500 font-mono truncate">{file}</p>
      </td>
      {cores.map((id) => {
        const c = game.checks[id];
        const isCurrent = game.current_core === id;
        return (
          <td key={id} className="text-center px-1.5" title={c ? describeCheck(c, t) : ""}>
            {c && (
              <span className={`inline-block min-w-6 px-1 rounded font-semibold ${STATE_COLOR[c.state]} ${isCurrent ? "ring-1 ring-zinc-500" : ""}`}>
                {STATE_MARK[c.state]}
                {c.driver && c.driver !== "good" && c.state === "ok" && <span className="text-[9px] text-amber-300 align-super">*</span>}
              </span>
            )}
          </td>
        );
      })}
      <td className="px-2 text-zinc-400 whitespace-nowrap">
        {game.current ?? "—"}
        {game.own_emulator && <span className="text-zinc-500"> · {t("arcade.ownEmulator")}</span>}
      </td>
      <td className="px-2 whitespace-nowrap text-indigo-300">{game.proposal ?? ""}</td>
    </tr>
  );
}

/** Los DAT de cada núcleo, en el orden de preferencia: versión, descargar/actualizar, quitar, subir/bajar */
function CoresBlock({
  status,
  downloading,
  downloadProgress,
  onCancelDownload,
  onDownload,
  onDelete,
  onMove,
  onMameVersion,
}: {
  status: ArcadeStatus;
  downloading: ArcadeCoreId | null;
  downloadProgress: Parameters<typeof TaskProgress>[0]["progress"];
  onCancelDownload: () => void;
  onDownload: (id: ArcadeCoreId) => void;
  onDelete: (id: ArcadeCoreId) => void;
  onMove: (id: ArcadeCoreId, by: -1 | 1) => void;
  onMameVersion: (version: string) => void;
}) {
  const { t } = useTranslation();
  const [versions, setVersions] = useState<string[] | null>(null);
  const byId = new Map(status.cores.map((c) => [c.id, c.dat]));
  // Las versiones de MAME se piden la primera vez que se abre el selector
  const loadVersions = () => {
    if (versions === null) mameVersions().then(setVersions).catch(() => setVersions([]));
  };

  return (
    <div className="mt-3 rounded border border-zinc-700 divide-y divide-zinc-700/60">
      {status.order.map((id, i) => {
        const dat = byId.get(id) ?? null;
        return (
          <div key={id} className="px-3 py-1.5 flex items-center gap-3 text-xs">
            <div className="flex flex-col text-[9px] leading-none text-zinc-500">
              <button disabled={i === 0 || !!downloading} onClick={() => onMove(id, -1)} className="hover:text-zinc-200 disabled:opacity-30" title={t("arcade.moveUp")}>▲</button>
              <button disabled={i === status.order.length - 1 || !!downloading} onClick={() => onMove(id, 1)} className="hover:text-zinc-200 disabled:opacity-30" title={t("arcade.moveDown")}>▼</button>
            </div>
            <span className="w-28 shrink-0 text-zinc-200 font-medium">{CORE_NAMES[id]}</span>
            <div className="flex-1 min-w-0">
              {downloading === id ? (
                <TaskProgress progress={downloadProgress} compact onCancel={onCancelDownload} />
              ) : dat ? (
                <span className="text-zinc-400">
                  {t("arcade.datInfo", { version: dat.version, games: dat.games, date: new Date(dat.updated).toLocaleDateString() })}
                  <span className="text-zinc-500"> · {t(`arcade.source_${dat.source}` as TranslationKeys)}</span>
                </span>
              ) : (
                <span className="text-zinc-500">{t("arcade.noDat")}</span>
              )}
            </div>
            {/* MAME actual: el MAME instalado o una versión publicada */}
            {id === "mame" && downloading !== id && (
              <select
                value={status.mame_version_setting ?? ""}
                onFocus={loadVersions}
                onMouseDown={loadVersions}
                onChange={(e) => onMameVersion(e.target.value)}
                className="bg-zinc-900 border border-zinc-600 rounded px-1.5 py-0.5 text-xs text-zinc-300 max-w-44"
                title={t("arcade.mameVersionHelp")}
              >
                <option value="">
                  {status.mame_path ? t("arcade.mameInstalled", { version: status.mame_version ?? "?" }) : t("arcade.mameLatest")}
                </option>
                {(versions ?? (status.mame_version_setting ? [status.mame_version_setting] : [])).map((v) => (
                  <option key={v} value={v}>
                    MAME {v}
                  </option>
                ))}
              </select>
            )}
            {downloading !== id && (
              <>
                <button
                  onClick={() => onDownload(id)}
                  disabled={!!downloading}
                  className="px-2.5 py-1 bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600 disabled:opacity-50"
                >
                  {dat ? t("arcade.update") : t("arcade.download")}
                </button>
                {dat && (
                  <button onClick={() => onDelete(id)} disabled={!!downloading} className="text-zinc-500 hover:text-red-400 disabled:opacity-50" title={t("arcade.delete")}>
                    ✕
                  </button>
                )}
              </>
            )}
          </div>
        );
      })}
    </div>
  );
}
