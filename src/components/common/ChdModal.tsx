import { useEffect, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { cancelChd, chdStatus, convertToChd, listChdCandidates, type ChdCandidate } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { formatBytes } from "../../utils/format";

interface ChdModalProps {
  open: boolean;
  systemPath: string;
  systemName: string;
  onClose: () => void;
  /** Después de convertir (recargar los juegos) */
  onDone: () => void;
}

type Originals = "trash" | "keep" | "subfolder";
type Progress = { game: number; total: number; name: string; disc: number; discs: number; step: "convert" | "verify"; percent: number };

const ORIGINALS: Originals[] = ["trash", "keep", "subfolder"];

/**
 * Herramientas → «Convertir a CHD…»: los juegos de disco del sistema que se pueden pasar a CHD
 * con chdman (instalado aparte), qué hacer con los originales y la conversión con su progreso.
 */
export function ChdModal({ open, systemPath, systemName, onClose, onDone }: ChdModalProps) {
  const { t } = useTranslation();
  const [chdman, setChdman] = useState<{ path: string | null; version: string | null } | null>(null);
  // undefined = cargando; null = el sistema no usa CHD
  const [games, setGames] = useState<ChdCandidate[] | null | undefined>(undefined);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [originals, setOriginals] = useState<Originals>("trash");
  const [folder, setFolder] = useState(() => t("chd.defaultFolder"));
  const [progress, setProgress] = useState<Progress | null>(null);

  async function loadStatus() {
    setChdman(await chdStatus().catch(() => ({ path: null, version: null })));
  }

  async function loadGames() {
    setGames(undefined);
    try {
      const r = await listChdCandidates(systemPath);
      setGames(r.supported ? r.games : null);
      // Al principio, marcados todos los que se pueden convertir
      setChecked(new Set(r.games.filter((g) => !g.blocked).map((g) => g.path)));
    } catch (e) {
      await showMessage(String(e), "error");
      setGames([]);
    }
  }

  useEffect(() => {
    if (!open) return;
    loadStatus();
    loadGames();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, systemPath]);

  if (!open) return null;

  /** Elegir chdman a mano: se guarda en Ajustes */
  async function pickChdman() {
    const file = await openDialog({ directory: false, multiple: false });
    if (!file) return;
    // Se guarda ya (no con la espera de patchSettings) para que la búsqueda lo encuentre
    const { settings, updateSettings } = useAppStore.getState();
    await updateSettings({ ...settings, chdman_path: String(file) });
    await loadStatus();
  }

  const toggle = (path: string) =>
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });

  const selected = (games ?? []).filter((g) => checked.has(g.path));
  const selectedSize = selected.reduce((sum, g) => sum + g.size, 0);
  const busy = !!progress;

  async function convert() {
    if (selected.length === 0) return;
    if (!(await askConfirm(t("chd.confirm", { count: selected.length, size: formatBytes(selectedSize) })))) return;
    setProgress({ game: 0, total: selected.length, name: "", disc: 0, discs: 0, step: "convert", percent: 0 });
    const unlisten = await listen<Progress>("chd-progress", (e) => setProgress(e.payload));
    try {
      const r = await convertToChd(systemPath, selected.map((g) => g.path), originals, originals === "subfolder" ? folder : undefined);
      const saved = Math.max(0, r.bytes_before - r.bytes_after);
      const lines = [
        r.stopped
          ? t("chd.stopped", { count: r.converted })
          : t("chd.done", { count: r.converted, before: formatBytes(r.bytes_before), after: formatBytes(r.bytes_after), saved: formatBytes(saved) }),
      ];
      if (r.skipped.length) lines.push("", t("chd.skipped", { count: r.skipped.length }), ...r.skipped.slice(0, 10));
      if (r.failed.length) lines.push("", t("chd.failed", { count: r.failed.length }), ...r.failed.slice(0, 10));
      if (r.originals_failed.length) lines.push("", t("chd.originalsFailed", { count: r.originals_failed.length }), ...r.originals_failed.slice(0, 10));
      await showMessage(lines.join("\n"), r.failed.length || r.originals_failed.length ? "error" : "info");
      onDone();
      await loadGames();
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      unlisten();
      setProgress(null);
    }
  }

  // Avance total: cada disco es la mitad convertir y la mitad comprobar
  const percent = (() => {
    if (!progress?.total || !progress.discs) return 0;
    const disc = (progress.step === "verify" ? 0.5 : 0) + progress.percent / 200;
    const game = (Math.max(progress.disc - 1, 0) + disc) / progress.discs;
    return Math.round(((Math.max(progress.game - 1, 0) + game) / progress.total) * 100);
  })();

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[680px] max-w-[calc(100vw-2rem)] h-[85vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("chd.title")}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("chd.help")}</p>
          {/* Dónde está chdman o cómo instalarlo */}
          {chdman && (chdman.path ? (
            <p className="text-[11px] text-emerald-400 mt-2 font-mono break-all">
              {t("chd.found", { version: chdman.version ?? "" })} · {chdman.path}
            </p>
          ) : (
            <div className="mt-3 rounded border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-200 space-y-1">
              <p>{t("chd.missing")}</p>
              <p className="text-zinc-300">{t("chd.installWindows")}</p>
              <p className="text-zinc-300 font-mono">{t("chd.installMac")}</p>
              <p className="text-zinc-300">{t("chd.installLinux")}</p>
              <div className="flex gap-2 pt-1">
                <button onClick={pickChdman} className="px-3 py-1 text-xs bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600">
                  {t("chd.pick")}
                </button>
                <button onClick={loadStatus} className="px-3 py-1 text-xs bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600">
                  {t("chd.recheck")}
                </button>
              </div>
            </div>
          ))}
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-3 space-y-3">
          {progress ? (
            <div className="py-6">
              <div className="w-full bg-zinc-700 rounded-full h-2">
                <div className="bg-indigo-500 h-2 rounded-full transition-all" style={{ width: `${percent}%` }} />
              </div>
              <p className="text-xs text-zinc-400 mt-2">{t("chd.progress", { game: progress.game, total: progress.total, disc: progress.disc, discs: progress.discs })}</p>
              <p className="text-xs text-zinc-300 truncate">
                {progress.name} — {t(`chd.step_${progress.step}` as TranslationKeys)} {progress.percent.toFixed(0)}%
              </p>
            </div>
          ) : games === undefined ? (
            <p className="py-12 text-center text-zinc-500">{t("chd.loading")}</p>
          ) : games === null ? (
            <p className="py-12 text-center text-sm text-zinc-400">{t("chd.unsupported", { system: systemName })}</p>
          ) : games.length === 0 ? (
            <p className="py-12 text-center text-sm text-zinc-500">{t("chd.none")}</p>
          ) : (
            <>
              <div className="flex items-center justify-between">
                <h4 className="text-sm font-medium text-zinc-300">
                  {t("chd.games", { count: games.length })} <span className="text-zinc-500 font-normal">· {formatBytes(selectedSize)}</span>
                </h4>
                <div className="flex gap-2 text-xs">
                  <button onClick={() => setChecked(new Set(games.filter((g) => !g.blocked).map((g) => g.path)))} className="text-indigo-300 hover:text-indigo-200">
                    {t("cleanup.selectAll")}
                  </button>
                  <button onClick={() => setChecked(new Set())} className="text-indigo-300 hover:text-indigo-200">
                    {t("bulk.selectNone")}
                  </button>
                </div>
              </div>
              <div className="space-y-1">
                {games.map((g) => (
                  <label key={g.path} className={`flex items-center gap-2 text-xs ${g.blocked ? "opacity-50" : "cursor-pointer"}`}>
                    <input
                      type="checkbox"
                      checked={checked.has(g.path)}
                      disabled={!!g.blocked}
                      onChange={() => toggle(g.path)}
                      className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                    />
                    <span className="text-zinc-200 truncate flex-1" title={g.discs.join("\n")}>{g.name}</span>
                    {g.discs.length > 1 && <span className="text-zinc-500 shrink-0">{t("chd.discs", { count: g.discs.length })}</span>}
                    {g.blocked && <span className="text-amber-400 shrink-0">{t("chd.blockedExists")}</span>}
                    <span className="text-zinc-500 shrink-0 w-16 text-right">{formatBytes(g.size)}</span>
                  </label>
                ))}
              </div>
            </>
          )}
        </div>

        {/* Qué hacer con los originales */}
        {!!games?.length && !progress && (
          <div className="border-t border-zinc-700 px-5 py-3 space-y-2">
            <h4 className="text-sm font-medium text-zinc-300">{t("chd.originals")}</h4>
            <div className="flex flex-wrap gap-x-5 gap-y-1 text-sm text-zinc-300">
              {ORIGINALS.map((o) => (
                <label key={o} className="flex items-center gap-2 cursor-pointer">
                  <input type="radio" checked={originals === o} onChange={() => setOriginals(o)} className="text-indigo-600" />
                  {t(`chd.originals_${o}` as TranslationKeys)}
                </label>
              ))}
            </div>
            {originals === "subfolder" && (
              <div>
                <input
                  value={folder}
                  onChange={(e) => setFolder(e.target.value)}
                  className="w-full max-w-xs bg-zinc-900 border border-zinc-600 rounded px-3 py-1.5 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
                />
                <p className="text-xs text-zinc-500 mt-1">{t("chd.subfolderHelp")}</p>
              </div>
            )}
            {originals === "keep" && <p className="text-xs text-zinc-500">{t("chd.originalsKeepHelp")}</p>}
          </div>
        )}

        <div className="border-t border-zinc-700 px-5 py-3 flex items-center gap-2">
          <button onClick={onClose} disabled={busy} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-50">
            {t("rename.close")}
          </button>
          <div className="flex-1" />
          {progress ? (
            <button onClick={() => cancelChd()} className="px-4 py-2 text-sm bg-red-600/20 text-red-400 rounded hover:bg-red-600/30">
              {t("scrapeProgress.stop")}
            </button>
          ) : (
            <button
              onClick={convert}
              disabled={!chdman?.path || selected.length === 0 || (originals === "subfolder" && !folder.trim())}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50"
            >
              {t("chd.convert", { count: selected.length })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
