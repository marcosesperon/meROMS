import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import { clearSystemDat, datReport, saveGamelist, setSystemDat, trashFiles, type DatReport } from "../../commands";
import { askConfirm, showMessage } from "../../utils/dialogs";

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
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<Tab>("missing");
  const [withClones, setWithClones] = useState(false);
  const [unknownSel, setUnknownSel] = useState<Set<string>>(new Set());
  const [nameSel, setNameSel] = useState<Set<string>>(new Set());

  async function load() {
    setLoading(true);
    try {
      const r = await datReport(systemPath);
      setReport(r);
      setUnknownSel(new Set());
      setNameSel(new Set(r?.names.map((n) => n.game_path) ?? []));
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (open) load();
    else setReport(null);
  }, [open, systemPath]);

  const missing = useMemo(
    () => (report?.missing ?? []).filter((m) => withClones || !m.cloneof),
    [report, withClones]
  );

  if (!open) return null;

  async function pickDat() {
    const file = await openDialog({ multiple: false, filters: [{ name: "DAT", extensions: ["dat", "xml"] }] });
    if (!file) return;
    setBusy(true);
    try {
      await setSystemDat(systemPath, String(file));
      onChanged();
      await load();
    } catch (e) {
      await showMessage(t("dat.loadFailed", { error: String(e) }), "error");
    } finally {
      setBusy(false);
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
    setBusy(true);
    try {
      const result = await trashFiles(systemPath, paths);
      if (result.failed.length > 0) {
        await showMessage(t("newRoms.trashFailed", { count: result.failed.length }) + "\n" + result.failed.join("\n"), "error");
      }
      onChanged();
      await load();
    } finally {
      setBusy(false);
    }
  }

  async function applyNames() {
    if (!report) return;
    const names = new Map(report.names.filter((n) => nameSel.has(n.game_path)).map((n) => [n.game_path, n.proposed]));
    setBusy(true);
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
      setBusy(false);
    }
  }

  function copyMissing() {
    const text = missing.map((m) => `${m.name}\t${m.description}${m.year ? `\t${m.year}` : ""}`).join("\n");
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
        { id: "missing", label: t("dat.tabMissing"), count: missing.length },
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
                {busy ? "..." : t("dat.pick")}
              </button>
              {report && (
                <button onClick={removeDat} className="px-3 py-1.5 text-xs bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
                  {t("dat.remove")}
                </button>
              )}
            </div>
          </div>
          {!report && <p className="text-xs text-zinc-500 mt-2">{t("dat.help")}</p>}
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
          {loading ? (
            <div className="flex items-center justify-center py-12 text-zinc-500">{t("newRoms.scanning")}</div>
          ) : !report ? null : tab === "missing" ? (
            <>
              <div className="px-5 py-2 flex items-center gap-4 border-b border-zinc-700/50 text-xs">
                <label className="flex items-center gap-1.5 text-zinc-400 cursor-pointer">
                  <input type="checkbox" checked={withClones} onChange={(e) => setWithClones(e.target.checked)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                  {t("dat.includeClones")}
                </label>
                <button onClick={copyMissing} className="text-indigo-400 hover:text-indigo-300">{t("dat.copyList")}</button>
              </div>
              <div className="divide-y divide-zinc-700/30">
                {missing.map((m) => (
                  <div key={m.name} className="px-5 py-1.5">
                    <p className="text-sm text-zinc-200 truncate">{m.description || m.name}</p>
                    <p className="text-xs text-zinc-500 truncate">
                      <span className="font-mono">{m.name}</span>
                      {m.cloneof && ` · ${t("dat.cloneOf", { parent: m.cloneof })}`}
                      {m.year && ` · ${m.year}`}
                      {m.manufacturer && ` · ${m.manufacturer}`}
                    </p>
                  </div>
                ))}
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
          <button onClick={onClose} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors">
            {t("rename.close")}
          </button>
          {report && tab === "unknown" && report.unknown_files.length > 0 && (
            <button onClick={trashUnknown} disabled={unknownSel.size === 0 || busy} className="px-4 py-2 text-sm bg-red-600 text-white rounded hover:bg-red-500 disabled:opacity-50">
              {t("newRoms.trash", { count: unknownSel.size })}
            </button>
          )}
          {report && tab === "names" && report.names.length > 0 && (
            <button onClick={applyNames} disabled={nameSel.size === 0 || busy} className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
              {t("dat.applyNames", { count: nameSel.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
