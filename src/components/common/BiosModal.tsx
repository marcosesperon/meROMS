import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { biosDownload, checkBios, type SystemBios } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { showMessage } from "../../utils/dialogs";

interface BiosModalProps {
  open: boolean;
  onClose: () => void;
}

const STATUS_STYLE: Record<string, string> = {
  ok: "text-emerald-400",
  missing: "text-zinc-500",
  mismatch: "text-amber-400",
};

/**
 * Herramientas → «BIOS…»: para cada sistema del perfil, las BIOS que pide la lista de libretro
 * y si están en la carpeta de BIOS del perfil (OK, falta, no coincide).
 */
export function BiosModal({ open, onClose }: BiosModalProps) {
  const { t } = useTranslation();
  const systems = useAppStore((s) => s.systems);
  const [report, setReport] = useState<{ bios_dir: string | null; systems: SystemBios[] } | null | undefined>(undefined);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  async function load() {
    setReport(undefined);
    try {
      const r = await checkBios(systems.map((s) => s.name));
      // Primero los que tienen algo que falta o no coincide
      r?.systems.sort((a, b) => Number(a.ok === a.total) - Number(b.ok === b.total) || a.system.localeCompare(b.system));
      setReport(r);
    } catch (e) {
      await showMessage(String(e), "error");
      setReport(null);
    }
  }

  useEffect(() => {
    if (open) load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (!open) return null;

  async function download() {
    setBusy(true);
    try {
      await biosDownload();
      await load();
    } catch (e) {
      await showMessage(t("bios.downloadFailed", { error: String(e) }), "error");
    } finally {
      setBusy(false);
    }
  }

  const toggle = (system: string) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(system)) next.delete(system);
      else next.add(system);
      return next;
    });

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[680px] max-w-[calc(100vw-2rem)] h-[85vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("bios.title")}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("bios.help")}</p>
          {report?.bios_dir && <p className="text-[11px] text-zinc-400 font-mono mt-2 break-all">{report.bios_dir}</p>}
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-3 space-y-2">
          {report === undefined ? (
            <p className="py-12 text-center text-zinc-500">{t("bios.checking")}</p>
          ) : report === null ? (
            <div className="py-12 text-center space-y-3">
              <p className="text-sm text-zinc-400">{t("bios.listNone")}</p>
              <button onClick={download} disabled={busy} className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
                {busy ? "..." : t("bios.download")}
              </button>
            </div>
          ) : report.systems.length === 0 ? (
            <p className="py-12 text-center text-sm text-zinc-500">{t("bios.noneNeeded")}</p>
          ) : (
            report.systems.map((s) => {
              const complete = s.ok === s.total;
              return (
                <div key={s.system} className="rounded border border-zinc-700 bg-zinc-900/40">
                  <button onClick={() => toggle(s.system)} className="w-full px-3 py-2 flex items-center gap-3 text-left">
                    <span className="text-xs text-zinc-500">{expanded.has(s.system) ? "▾" : "▸"}</span>
                    <span className="text-sm text-zinc-200 flex-1 truncate">
                      {s.system} <span className="text-zinc-500 text-xs">· {s.section}</span>
                    </span>
                    <span className={`text-xs ${complete ? "text-emerald-400" : s.ok > 0 ? "text-amber-300" : "text-zinc-400"}`}>
                      {t("bios.count", { ok: s.ok, total: s.total })}
                    </span>
                  </button>
                  {expanded.has(s.system) && (
                    <div className="px-3 pb-2 space-y-1">
                      {s.files.map((f) => (
                        <div key={f.name} className="flex items-start gap-2 text-xs">
                          <span className={`w-24 shrink-0 ${STATUS_STYLE[f.status]}`}>{t(`bios.status_${f.status}` as never)}</span>
                          <span className="min-w-0">
                            <span className="block text-zinc-300 font-mono break-all">{f.name}</span>
                            {f.status === "mismatch" && f.expected_md5 && (
                              <span className="block text-[11px] text-zinc-500 font-mono">MD5 {f.expected_md5}</span>
                            )}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button onClick={onClose} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
            {t("rename.close")}
          </button>
          {report && (
            <button onClick={load} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600">
              {t("bios.recheck")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
