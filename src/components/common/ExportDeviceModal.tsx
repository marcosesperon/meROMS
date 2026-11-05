import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import {
  applyPull,
  cancelExport,
  planExport,
  pullChanges,
  runExport,
  type DeviceChange,
  type ExportOptions,
  type ExportPreview,
} from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { getActiveProfile } from "../../utils/profiles";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { formatBytes } from "../../utils/format";

interface ExportDeviceModalProps {
  open: boolean;
  onClose: () => void;
}

// Tipos de media que se pueden copiar (carpetas de ES-DE) y su texto
const MEDIA: { folder: string; label: TranslationKeys; classicOnly?: boolean }[] = [
  { folder: "covers", label: "filter.media.image" },
  { folder: "screenshots", label: "filter.media.thumbnail" },
  { folder: "videos", label: "filter.media.video" },
  { folder: "marquees", label: "filter.media.marquee" },
  { folder: "titlescreens", label: "filter.media.titleshot" },
  { folder: "backcovers", label: "filter.media.boxback" },
  { folder: "3dboxes", label: "filter.media.boxart" },
  { folder: "physicalmedia", label: "filter.media.cartridge" },
  { folder: "fanart", label: "filter.media.fanart" },
  { folder: "miximages", label: "filter.media.mix" },
  { folder: "manuals", label: "filter.media.manual" },
  { folder: "wheels", label: "filter.media.wheel", classicOnly: true },
];

const FIELD_LABEL: Record<DeviceChange["field"], TranslationKeys> = {
  favorite: "form.favorite",
  hidden: "form.hidden",
  playcount: "exportDevice.playcount",
  lastplayed: "exportDevice.lastplayed",
  gametime: "exportDevice.gametime",
};

type Progress = { current: number; total: number; bytes: number; total_bytes: number; file: string };

const size = formatBytes;

/**
 * «Exportar a dispositivo»: copia sistemas, juegos y media elegidos a una carpeta (SD, USB…)
 * con la estructura del perfil, solo lo nuevo o cambiado; y trae de vuelta lo que cambió allí.
 */
export function ExportDeviceModal({ open, onClose }: ExportDeviceModalProps) {
  const { t } = useTranslation();
  const systems = useAppStore((s) => s.systems);
  const settings = useAppStore((s) => s.settings);
  const isClassic = getActiveProfile(settings)?.kind !== "esde";
  const targets = settings.export_targets ?? [];

  const withGames = useMemo(() => systems.filter((s) => s.game_count > 0), [systems]);
  const [options, setOptions] = useState<ExportOptions>(() => ({
    dest: "",
    systems: [],
    games: "all",
    media: MEDIA.filter((m) => m.folder !== "videos" && m.folder !== "manuals").map((m) => m.folder),
    remove_extra: false,
  }));
  const [preview, setPreview] = useState<ExportPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [changes, setChanges] = useState<DeviceChange[] | null>(null);
  const [unchecked, setUnchecked] = useState<Set<number>>(new Set());

  // Al abrir: el último destino usado, o todos los sistemas con juegos
  useEffect(() => {
    if (!open) return;
    const last = targets[0];
    setOptions((o) => (last ? { ...last } : { ...o, systems: withGames.map((s) => s.path) }));
    setPreview(null);
    setChanges(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  // Cualquier cambio de opciones invalida la vista previa
  useEffect(() => setPreview(null), [options]);

  if (!open) return null;

  const set = (patch: Partial<ExportOptions>) => setOptions((o) => ({ ...o, ...patch }));
  const toggle = (list: string[], value: string) => (list.includes(value) ? list.filter((v) => v !== value) : [...list, value]);
  const ready = !!options.dest && options.systems.length > 0;

  /** El destino y su selección quedan los primeros de la lista (se recuerdan hasta 5) */
  function remember() {
    const rest = targets.filter((tg) => tg.dest !== options.dest);
    useAppStore.getState().patchSettings({ export_targets: [{ ...options }, ...rest].slice(0, 5) });
  }

  async function pickDest() {
    const dir = await openDialog({ directory: true, multiple: false });
    if (dir) set({ dest: String(dir) });
  }

  async function doPreview() {
    setBusy(true);
    try {
      setPreview(await planExport(options));
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  async function doExport() {
    if (!preview) return;
    if (options.remove_extra && preview.remove_files > 0 && !(await askConfirm(t("exportDevice.removeConfirm", { count: preview.remove_files })))) return;
    setBusy(true);
    setProgress({ current: 0, total: preview.copy_files, bytes: 0, total_bytes: preview.copy_bytes, file: "" });
    const unlisten = await listen<Progress>("export-progress", (e) => setProgress(e.payload));
    try {
      remember();
      const r = await runExport(options);
      const lines = [t(r.stopped ? "exportDevice.stopped" : "exportDevice.done", { copied: r.copied, removed: r.removed })];
      if (r.failed.length) lines.push("", t("exportDevice.failed", { count: r.failed.length }), ...r.failed.slice(0, 20));
      await showMessage(lines.join("\n"), r.failed.length ? "error" : "info");
      setPreview(null);
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      unlisten();
      setProgress(null);
      setBusy(false);
    }
  }

  async function doPull() {
    setBusy(true);
    try {
      setChanges(await pullChanges(options.dest, options.systems));
      setUnchecked(new Set());
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  async function doApplyPull() {
    if (!changes) return;
    setBusy(true);
    try {
      const n = await applyPull(changes.filter((_, i) => !unchecked.has(i)));
      const { selectedSystem, loadGames } = useAppStore.getState();
      if (selectedSystem) await loadGames(selectedSystem.path);
      await showMessage(t("exportDevice.pullDone", { count: n }));
      setChanges(null);
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  const fmt = (c: DeviceChange, v: string | null) =>
    c.field === "favorite" || c.field === "hidden" ? t(v === "true" ? "exportDevice.yes" : "exportDevice.no") : v ?? "—";
  const percent = progress?.total_bytes ? Math.round((progress.bytes / progress.total_bytes) * 100) : 0;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[760px] max-w-[calc(100vw-2rem)] h-[88vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("exportDevice.title")}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t(isClassic ? "exportDevice.helpClassic" : "exportDevice.helpEsde")}</p>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-5">
          {/* Destino */}
          <section className="space-y-2">
            <h4 className="text-sm font-medium text-zinc-300">{t("exportDevice.dest")}</h4>
            <div className="flex gap-2">
              <input
                value={options.dest}
                onChange={(e) => set({ dest: e.target.value })}
                placeholder={t("exportDevice.destPlaceholder")}
                className="flex-1 min-w-0 bg-zinc-900 border border-zinc-600 rounded px-3 py-1.5 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
              />
              <button onClick={pickDest} disabled={busy} className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600">
                {t("exportDevice.choose")}
              </button>
            </div>
            {targets.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {targets.map((tg) => (
                  <button
                    key={tg.dest}
                    onClick={() => setOptions({ ...tg })}
                    disabled={busy}
                    title={tg.dest}
                    className={`px-2 py-0.5 text-[11px] rounded max-w-[260px] truncate ${tg.dest === options.dest ? "bg-indigo-600 text-white" : "bg-zinc-700 text-zinc-300 hover:bg-zinc-600"}`}
                  >
                    {tg.dest}
                  </button>
                ))}
              </div>
            )}
          </section>

          {/* Sistemas */}
          <section className="space-y-2">
            <div className="flex items-center justify-between">
              <h4 className="text-sm font-medium text-zinc-300">{t("exportDevice.systems", { count: options.systems.length })}</h4>
              <div className="flex gap-2 text-xs">
                <button onClick={() => set({ systems: withGames.map((s) => s.path) })} className="text-indigo-300 hover:text-indigo-200">
                  {t("cleanup.selectAll")}
                </button>
                <button onClick={() => set({ systems: [] })} className="text-indigo-300 hover:text-indigo-200">
                  {t("bulk.selectNone")}
                </button>
              </div>
            </div>
            <div className="grid grid-cols-2 sm:grid-cols-3 gap-x-4 gap-y-1 max-h-40 overflow-y-auto rounded border border-zinc-700 p-2">
              {withGames.map((s) => (
                <label key={s.path} className="flex items-center gap-2 text-xs text-zinc-300 cursor-pointer min-w-0">
                  <input type="checkbox" checked={options.systems.includes(s.path)} onChange={() => set({ systems: toggle(options.systems, s.path) })} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                  <span className="truncate">{s.name}</span>
                  <span className="text-zinc-500">{s.game_count}</span>
                </label>
              ))}
            </div>
          </section>

          {/* Juegos y media */}
          <section className="space-y-2">
            <h4 className="text-sm font-medium text-zinc-300">{t("exportDevice.games")}</h4>
            <div className="flex flex-wrap gap-x-5 gap-y-1 text-sm text-zinc-300">
              {(["all", "favorites", "visible"] as const).map((g) => (
                <label key={g} className="flex items-center gap-2 cursor-pointer">
                  <input type="radio" checked={options.games === g} onChange={() => set({ games: g })} className="text-indigo-600" />
                  {t(`exportDevice.games_${g}` as TranslationKeys)}
                </label>
              ))}
            </div>
          </section>
          <section className="space-y-2">
            <h4 className="text-sm font-medium text-zinc-300">{t("exportDevice.media")}</h4>
            <div className="flex flex-wrap gap-x-4 gap-y-1">
              {MEDIA.filter((m) => isClassic || !m.classicOnly).map((m) => (
                <label key={m.folder} className="flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
                  <input type="checkbox" checked={options.media.includes(m.folder)} onChange={() => set({ media: toggle(options.media, m.folder) })} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                  {t(m.label)}
                </label>
              ))}
            </div>
          </section>
          <label className="flex items-center gap-2 text-sm text-zinc-300 cursor-pointer">
            <input type="checkbox" checked={options.remove_extra} onChange={(e) => set({ remove_extra: e.target.checked })} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
            {t("exportDevice.removeExtra")}
          </label>

          {/* Vista previa / progreso */}
          {progress ? (
            <div>
              <div className="w-full bg-zinc-700 rounded-full h-2">
                <div className="bg-indigo-500 h-2 rounded-full transition-all" style={{ width: `${percent}%` }} />
              </div>
              <p className="text-xs text-zinc-400 mt-1 truncate">
                {progress.current} / {progress.total} · {size(progress.bytes)} / {size(progress.total_bytes)} — {progress.file}
              </p>
            </div>
          ) : preview ? (
            <div className="rounded border border-zinc-700 bg-zinc-900/50 p-3 text-sm text-zinc-300 space-y-1">
              <p>{t("exportDevice.previewCopy", { files: preview.copy_files, size: size(preview.copy_bytes), games: preview.games })}</p>
              <p className="text-zinc-500">{t("exportDevice.previewUpToDate", { count: preview.up_to_date })}</p>
              {options.remove_extra && (
                <p className={preview.remove_files ? "text-amber-300" : "text-zinc-500"}>{t("exportDevice.previewRemove", { count: preview.remove_files })}</p>
              )}
              {options.remove_extra && preview.remove_sample.length > 0 && (
                <div className="max-h-24 overflow-y-auto text-[11px] text-zinc-500 font-mono">
                  {preview.remove_sample.map((f) => (
                    <p key={f} className="truncate">{f}</p>
                  ))}
                </div>
              )}
            </div>
          ) : null}

          {/* Cambios traídos del destino */}
          {changes && (
            <section className="space-y-2">
              <h4 className="text-sm font-medium text-zinc-300">{t("exportDevice.changes", { count: changes.length })}</h4>
              {changes.length === 0 ? (
                <p className="text-xs text-zinc-500">{t("exportDevice.noChanges")}</p>
              ) : (
                <div className="max-h-48 overflow-y-auto rounded border border-zinc-700 p-2 space-y-1">
                  {changes.map((c, i) => (
                    <label key={i} className="flex items-center gap-2 text-xs cursor-pointer">
                      <input
                        type="checkbox"
                        checked={!unchecked.has(i)}
                        onChange={() => setUnchecked((prev) => {
                          const next = new Set(prev);
                          if (next.has(i)) next.delete(i);
                          else next.add(i);
                          return next;
                        })}
                        className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                      />
                      <span className="text-zinc-500 shrink-0">{c.system}</span>
                      <span className="text-zinc-200 truncate flex-1">{c.name}</span>
                      <span className="text-zinc-400 shrink-0">
                        {t(FIELD_LABEL[c.field])}: {fmt(c, c.from)} → {fmt(c, c.to)}
                      </span>
                    </label>
                  ))}
                </div>
              )}
            </section>
          )}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex items-center gap-2">
          <button onClick={onClose} disabled={busy} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-50">
            {t("rename.close")}
          </button>
          <div className="flex-1" />
          {progress ? (
            <button onClick={() => cancelExport()} className="px-4 py-2 text-sm bg-red-600/20 text-red-400 rounded hover:bg-red-600/30">
              {t("scrapeProgress.stop")}
            </button>
          ) : changes && changes.length > 0 ? (
            <button onClick={doApplyPull} disabled={busy || unchecked.size === changes.length} className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
              {t("exportDevice.applyPull", { count: changes.length - unchecked.size })}
            </button>
          ) : (
            <>
              <button onClick={doPull} disabled={!ready || busy} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600 disabled:opacity-50">
                {t("exportDevice.pull")}
              </button>
              {preview ? (
                <button onClick={doExport} disabled={busy || (preview.copy_files === 0 && preview.remove_files === 0 && preview.games === 0)} className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
                  {t("exportDevice.export")}
                </button>
              ) : (
                <button onClick={doPreview} disabled={!ready || busy} className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
                  {busy ? "..." : t("exportDevice.preview")}
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
