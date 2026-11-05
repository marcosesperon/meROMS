import { useEffect, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import {
  applyCompare,
  cancelCompare,
  compareFolders,
  type CompareAction,
  type CompareGame,
  type CompareSide,
  type SystemCompare,
} from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { getActiveProfile } from "../../utils/profiles";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { formatBytes } from "../../utils/format";

interface CompareModalProps {
  open: boolean;
  onClose: () => void;
}

type Progress = { current: number; total: number; bytes: number; total_bytes: number; file: string };
type Direction = CompareAction["to"];
type ListKind = "only_here" | "only_there" | "different";

/** Clave de un juego en la selección: sistema + ruta */
const keyOf = (system: string, path: string) => `${system}\u0000${path}`;

/** Hacia dónde va un juego de la lista «Solo aquí» / «Solo allí» al marcarlo */
const FIXED_DIRECTION: Partial<Record<ListKind, Direction>> = { only_here: "there", only_there: "here" };

const LIST_TITLE: Record<ListKind, TranslationKeys> = {
  only_here: "compare.onlyHere",
  only_there: "compare.onlyThere",
  different: "compare.different",
};

const sideText = (side: CompareSide | null) =>
  side ? `${formatBytes(side.size)} · ${new Date(side.modified * 1000).toLocaleDateString()}` : "—";

/**
 * Herramientas → «Comparar con otra carpeta…»: la biblioteca frente a otra carpeta con la misma
 * estructura, sistema a sistema (solo aquí, solo allí, distintos), y copia lo elegido en el
 * sentido que toque, con su media y su entrada de gamelist.
 */
export function CompareModal({ open, onClose }: CompareModalProps) {
  const { t } = useTranslation();
  const settings = useAppStore((s) => s.settings);
  const isClassic = getActiveProfile(settings)?.kind !== "esde";

  const [dest, setDest] = useState("");
  const [checkCrc, setCheckCrc] = useState(false);
  const [report, setReport] = useState<SystemCompare[] | null>(null);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  // Juegos elegidos y hacia dónde van
  const [chosen, setChosen] = useState<Map<string, Direction>>(new Map());
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);

  // Al abrir: la última carpeta comparada
  useEffect(() => {
    if (!open) return;
    setDest(settings.compare_dest ?? "");
    setReport(null);
    setChosen(new Map());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (!open) return null;

  async function pickDest() {
    const dir = await openDialog({ directory: true, multiple: false });
    if (dir) {
      setDest(String(dir));
      setReport(null);
    }
  }

  /** Corre algo largo con la barra de progreso de `compare-progress` */
  async function withProgress<T>(job: () => Promise<T>): Promise<T | undefined> {
    setBusy(true);
    setProgress({ current: 0, total: 0, bytes: 0, total_bytes: 0, file: "" });
    const unlisten = await listen<Progress>("compare-progress", (e) => setProgress(e.payload));
    try {
      return await job();
    } catch (e) {
      await showMessage(String(e), "error");
      return undefined;
    } finally {
      unlisten();
      setProgress(null);
      setBusy(false);
    }
  }

  async function doCompare() {
    useAppStore.getState().patchSettings({ compare_dest: dest });
    const r = await withProgress(() => compareFolders(dest, checkCrc));
    if (!r) return;
    setReport(r.systems);
    setChosen(new Map());
    // Abiertos de entrada los sistemas que tienen diferencias
    setExpanded(new Set(r.systems.filter((s) => s.only_here.length + s.only_there.length + s.different.length > 0).map((s) => s.system)));
  }

  /** Los juegos elegidos, con lo que ocupa cada sentido y cuántos sustituyen a otro */
  function summary() {
    const actions: CompareAction[] = [];
    const total = { here: 0, there: 0, sizeHere: 0, sizeThere: 0, replaced: 0 };
    for (const s of report ?? []) {
      for (const g of [...s.only_here, ...s.only_there, ...s.different]) {
        const to = chosen.get(keyOf(s.system, g.path));
        if (!to) continue;
        actions.push({ system: s.system, game_path: g.path, to });
        const from = to === "there" ? g.here : g.there;
        total[to] += 1;
        if (to === "there") total.sizeThere += from?.size ?? 0;
        else total.sizeHere += from?.size ?? 0;
        if (g.here && g.there) total.replaced += 1;
      }
    }
    return { actions, ...total };
  }

  async function doApply() {
    const s = summary();
    const ok = await askConfirm(
      t("compare.confirm", {
        toThere: s.there,
        sizeThere: formatBytes(s.sizeThere),
        toHere: s.here,
        sizeHere: formatBytes(s.sizeHere),
        replaced: s.replaced,
      }),
    );
    if (!ok) return;
    const r = await withProgress(() => applyCompare(dest, s.actions));
    if (!r) return;
    const lines = [t(r.stopped ? "compare.stopped" : "compare.done", { there: r.to_there, here: r.to_here, replaced: r.replaced })];
    if (r.failed.length) lines.push("", t("compare.failed", { count: r.failed.length }), ...r.failed.slice(0, 20));
    await showMessage(lines.join("\n"), r.failed.length ? "error" : "info");

    // Puede haber sistemas o juegos nuevos en la biblioteca
    const { loadSystems, selectedSystem, loadGames } = useAppStore.getState();
    await loadSystems();
    if (selectedSystem) await loadGames(selectedSystem.path);
    await doCompare();
  }

  const setChoice = (system: string, path: string, to: Direction | null) =>
    setChosen((prev) => {
      const next = new Map(prev);
      if (to) next.set(keyOf(system, path), to);
      else next.delete(keyOf(system, path));
      return next;
    });

  /** «Marcar todos» / «Ninguno» de una lista de un sentido fijo */
  const setAll = (system: string, games: CompareGame[], to: Direction | null) =>
    setChosen((prev) => {
      const next = new Map(prev);
      for (const g of games) {
        if (to) next.set(keyOf(system, g.path), to);
        else next.delete(keyOf(system, g.path));
      }
      return next;
    });

  const toggleSystem = (system: string) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(system)) next.delete(system);
      else next.add(system);
      return next;
    });

  const count = chosen.size;
  const anyDifference = report?.some((s) => s.only_here.length + s.only_there.length + s.different.length > 0);
  const percent = progress?.total_bytes
    ? Math.round((progress.bytes / progress.total_bytes) * 100)
    : progress?.total
      ? Math.round((progress.current / progress.total) * 100)
      : 0;

  function renderList(s: SystemCompare, kind: ListKind) {
    const games = s[kind];
    if (games.length === 0) return null;
    const fixed = FIXED_DIRECTION[kind];
    return (
      <div className="space-y-1">
        <div className="flex items-center justify-between">
          <h5 className="text-xs font-medium text-zinc-400">
            {t(LIST_TITLE[kind], { count: games.length })}
            {fixed && <span className="text-zinc-500 font-normal"> · {t(fixed === "there" ? "compare.copyThere" : "compare.bringHere")}</span>}
          </h5>
          {fixed && (
            <div className="flex gap-2 text-[11px]">
              <button onClick={() => setAll(s.system, games, fixed)} disabled={busy} className="text-indigo-300 hover:text-indigo-200">
                {t("cleanup.selectAll")}
              </button>
              <button onClick={() => setAll(s.system, games, null)} disabled={busy} className="text-indigo-300 hover:text-indigo-200">
                {t("bulk.selectNone")}
              </button>
            </div>
          )}
        </div>
        {games.map((g) => {
          const choice = chosen.get(keyOf(s.system, g.path));
          if (fixed) {
            const side = fixed === "there" ? g.here : g.there;
            return (
              <label key={g.path} className="flex items-center gap-2 text-xs cursor-pointer">
                <input
                  type="checkbox"
                  checked={!!choice}
                  disabled={busy}
                  onChange={(e) => setChoice(s.system, g.path, e.target.checked ? fixed : null)}
                  className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                />
                <span className="text-zinc-200 truncate flex-1" title={g.path}>{g.name}</span>
                <span className="text-zinc-500 shrink-0">{sideText(side)}</span>
              </label>
            );
          }
          // Distintos: se elige cuál se queda (por defecto, ninguno)
          const options: { to: Direction | null; label: TranslationKeys }[] = [
            { to: null, label: "compare.none" },
            { to: "there", label: "compare.useHere" },
            { to: "here", label: "compare.useThere" },
          ];
          return (
            <div key={g.path} className="rounded bg-zinc-900/40 px-2 py-1.5 space-y-1">
              <p className="text-xs text-zinc-200 truncate" title={g.path}>{g.name}</p>
              <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px]">
                <span className="text-zinc-400">{t("compare.here")}: {sideText(g.here)}</span>
                <span className="text-zinc-400">{t("compare.there")}: {sideText(g.there)}</span>
                <span className="flex-1" />
                {options.map((o) => (
                  <button
                    key={o.label}
                    onClick={() => setChoice(s.system, g.path, o.to)}
                    disabled={busy}
                    className={`px-2 py-0.5 rounded ${(choice ?? null) === o.to ? "bg-indigo-600 text-white" : "bg-zinc-700 text-zinc-300 hover:bg-zinc-600"}`}
                  >
                    {t(o.label)}
                  </button>
                ))}
              </div>
            </div>
          );
        })}
      </div>
    );
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[760px] max-w-[calc(100vw-2rem)] h-[88vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700 space-y-3">
          <div>
            <h3 className="text-base font-semibold text-zinc-100">{t("compare.title")}</h3>
            <p className="text-xs text-zinc-500 mt-1">{t(isClassic ? "compare.helpClassic" : "compare.helpEsde")}</p>
          </div>
          <div className="flex gap-2">
            <input
              value={dest}
              onChange={(e) => {
                setDest(e.target.value);
                setReport(null);
              }}
              placeholder={t("compare.folder")}
              disabled={busy}
              className="flex-1 min-w-0 bg-zinc-900 border border-zinc-600 rounded px-3 py-1.5 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
            />
            <button onClick={pickDest} disabled={busy} className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600">
              {t("exportDevice.choose")}
            </button>
          </div>
          <label className="flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
            <input type="checkbox" checked={checkCrc} disabled={busy} onChange={(e) => setCheckCrc(e.target.checked)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
            {t("compare.checkCrc")}
          </label>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-3 space-y-2">
          {progress ? (
            <div className="py-6">
              <div className="w-full bg-zinc-700 rounded-full h-2">
                <div className="bg-indigo-500 h-2 rounded-full transition-all" style={{ width: `${percent}%` }} />
              </div>
              <p className="text-xs text-zinc-400 mt-1 truncate">
                {progress.total_bytes
                  ? `${progress.current} / ${progress.total} · ${formatBytes(progress.bytes)} / ${formatBytes(progress.total_bytes)} — ${progress.file}`
                  : `${t("compare.comparing")} ${progress.total ? `${progress.current} / ${progress.total} — ${progress.file}` : ""}`}
              </p>
            </div>
          ) : report === null ? null : report.length === 0 ? (
            <p className="py-12 text-center text-sm text-zinc-500">{t("compare.noGames")}</p>
          ) : (
            <>
              {!anyDifference && <p className="py-4 text-center text-sm text-emerald-400">{t("compare.nothing")}</p>}
              {report.map((s) => {
                const diffs = s.only_here.length + s.only_there.length + s.different.length;
                return (
                  <div key={s.system} className="rounded border border-zinc-700 bg-zinc-900/40">
                    <button onClick={() => toggleSystem(s.system)} disabled={diffs === 0} className="w-full px-3 py-2 flex items-center gap-3 text-left">
                      <span className="text-xs text-zinc-500 w-3">{diffs === 0 ? "" : expanded.has(s.system) ? "▾" : "▸"}</span>
                      <span className="text-sm text-zinc-200 flex-1 truncate">{s.system}</span>
                      <span className="text-[11px] text-zinc-400 shrink-0 space-x-3">
                        {s.only_here.length > 0 && <span>{t("compare.onlyHere", { count: s.only_here.length })}</span>}
                        {s.only_there.length > 0 && <span>{t("compare.onlyThere", { count: s.only_there.length })}</span>}
                        {s.different.length > 0 && <span className="text-amber-300">{t("compare.different", { count: s.different.length })}</span>}
                        <span className="text-zinc-500">{t("compare.same", { count: s.same })}</span>
                      </span>
                    </button>
                    {expanded.has(s.system) && diffs > 0 && (
                      <div className="px-3 pb-3 space-y-3">
                        {renderList(s, "only_here")}
                        {renderList(s, "only_there")}
                        {renderList(s, "different")}
                      </div>
                    )}
                  </div>
                );
              })}
            </>
          )}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex items-center gap-2">
          <button onClick={onClose} disabled={busy} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-50">
            {t("rename.close")}
          </button>
          <div className="flex-1" />
          {progress ? (
            <button onClick={() => cancelCompare()} className="px-4 py-2 text-sm bg-red-600/20 text-red-400 rounded hover:bg-red-600/30">
              {t("scrapeProgress.stop")}
            </button>
          ) : (
            <>
              <button
                onClick={doCompare}
                disabled={!dest.trim() || busy}
                className={`px-4 py-2 text-sm rounded disabled:opacity-50 ${report ? "bg-zinc-700 text-zinc-200 hover:bg-zinc-600" : "bg-indigo-600 text-white hover:bg-indigo-500"}`}
              >
                {t(report ? "compare.again" : "compare.run")}
              </button>
              {report && (
                <button onClick={doApply} disabled={busy || count === 0} className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
                  {t("compare.apply", { count })}
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
