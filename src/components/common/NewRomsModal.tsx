import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "../../i18n";
import { addNewRoms, listNewRoms, trashFiles } from "../../commands";
import type { NewRom } from "../../types";
import { askConfirm, showMessage } from "../../utils/dialogs";

interface NewRomsModalProps {
  open: boolean;
  systemPath: string;
  /** false = no hay scraper configurado: no se ofrece scrapear */
  canScrape: boolean;
  onClose: () => void;
  /** Se llama tras añadir, con las rutas añadidas y si hay que scrapearlas */
  onAdded: (paths: string[], scrape: boolean) => void;
  /** Files were sent to the trash (so the "New ROMs (N)" count can be refreshed) */
  onTrashed: () => void;
}

/**
 * «ROMs nuevas»: ficheros de la carpeta del sistema que no están en la gamelist.
 * Las de extensión conocida salen marcadas; las de otras extensiones, aparte y desmarcadas.
 * El nombre de cada una se puede cambiar antes de añadirla.
 */
export function NewRomsModal({ open, systemPath, canScrape, onClose, onAdded, onTrashed }: NewRomsModalProps) {
  const { t } = useTranslation();
  const [roms, setRoms] = useState<NewRom[]>([]);
  // ruta → nombre escrito (empieza con el propuesto)
  const [names, setNames] = useState<Record<string, string>>({});
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scrape, setScrape] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [adding, setAdding] = useState(false);
  const [trashing, setTrashing] = useState(false);

  function scan() {
    setScanning(true);
    listNewRoms(systemPath)
      .then((found) => {
        setRoms(found);
        setNames(Object.fromEntries(found.map((r) => [r.path, r.proposed_name])));
        setSelected(new Set(found.filter((r) => r.known_extension).map((r) => r.path)));
      })
      .catch((e) => showMessage(t("newRoms.failed", { error: String(e) }), "error"))
      .finally(() => setScanning(false));
  }

  useEffect(() => {
    if (!open) {
      setRoms([]);
      setNames({});
      setSelected(new Set());
      return;
    }
    setScrape(true);
    scan();
  }, [open, systemPath]);

  const known = useMemo(() => roms.filter((r) => r.known_extension), [roms]);
  const unknown = useMemo(() => roms.filter((r) => !r.known_extension), [roms]);

  if (!open) return null;

  function toggle(path: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  async function handleAdd() {
    const entries = roms
      .filter((r) => selected.has(r.path))
      .map((r) => ({ path: r.path, name: names[r.path] ?? r.proposed_name }));
    setAdding(true);
    try {
      await addNewRoms(systemPath, entries);
      onClose();
      onAdded(entries.map((e) => e.path), canScrape && scrape);
    } catch (e) {
      await showMessage(t("newRoms.failed", { error: String(e) }), "error");
    } finally {
      setAdding(false);
    }
  }

  // Lo que no es una ROM válida se limpia de la carpeta: a la papelera (se puede recuperar)
  async function handleTrash() {
    const paths = roms.filter((r) => selected.has(r.path)).map((r) => r.path);
    if (!(await askConfirm(t("newRoms.trashConfirm", { count: paths.length })))) return;
    setTrashing(true);
    try {
      const result = await trashFiles(systemPath, paths);
      if (result.failed.length > 0) {
        await showMessage(
          t("newRoms.trashDone", { count: result.trashed }) + "\n\n" + t("newRoms.trashFailed", { count: result.failed.length }) + "\n" + result.failed.join("\n"),
          "error"
        );
      }
      onTrashed();
      scan();
    } catch (e) {
      await showMessage(t("newRoms.failed", { error: String(e) }), "error");
    } finally {
      setTrashing(false);
    }
  }

  // Una fila: casilla, nombre editable y la ruta debajo
  const row = (rom: NewRom) => (
    <div key={rom.path} className="grid grid-cols-[auto_1fr] gap-3 px-5 py-2 items-center">
      <input
        type="checkbox"
        checked={selected.has(rom.path)}
        onChange={() => toggle(rom.path)}
        className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
      />
      <div className="min-w-0">
        <input
          type="text"
          value={names[rom.path] ?? ""}
          onChange={(e) => setNames((prev) => ({ ...prev, [rom.path]: e.target.value }))}
          className="w-full bg-zinc-900 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
        />
        <p className="text-xs text-zinc-500 font-mono truncate mt-0.5" title={rom.path}>
          {rom.path}
        </p>
      </div>
    </div>
  );

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[720px] max-h-[80vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("newRoms.title")}</h3>
          <p className="text-sm text-zinc-400 mt-1">{t("newRoms.description")}</p>
          {!scanning && roms.length > 0 && (
            <div className="flex items-center gap-4 mt-2 text-xs">
              <span className="text-zinc-500">{t("newRoms.found", { count: roms.length })}</span>
              <button onClick={() => setSelected(new Set(roms.map((r) => r.path)))} className="text-indigo-400 hover:text-indigo-300">
                {t("rename.selectAll")}
              </button>
              <button onClick={() => setSelected(new Set())} className="text-indigo-400 hover:text-indigo-300">
                {t("rename.deselectAll")}
              </button>
            </div>
          )}
        </div>

        <div className="flex-1 overflow-y-auto">
          {scanning ? (
            <div className="flex items-center justify-center py-12 text-zinc-500">{t("newRoms.scanning")}</div>
          ) : roms.length === 0 ? (
            <div className="py-12 px-8 text-center text-sm text-zinc-500">{t("newRoms.nothing")}</div>
          ) : (
            <>
              <div className="divide-y divide-zinc-700/30">{known.map(row)}</div>
              {unknown.length > 0 && (
                <>
                  <div className="px-5 pt-4 pb-1 border-t border-zinc-700">
                    <p className="text-xs font-semibold text-zinc-400">{t("newRoms.otherExtensions")}</p>
                    <p className="text-xs text-zinc-500">{t("newRoms.otherExtensionsHelp")}</p>
                  </div>
                  <div className="divide-y divide-zinc-700/30">{unknown.map(row)}</div>
                </>
              )}
            </>
          )}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex items-center gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("rename.close")}
          </button>
          <div className="flex-1" />
          {roms.length > 0 && canScrape && (
            <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
              <input
                type="checkbox"
                checked={scrape}
                onChange={(e) => setScrape(e.target.checked)}
                className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
              />
              {t("newRoms.scrapeAfter")}
            </label>
          )}
          {roms.length > 0 && (
            <button
              onClick={handleTrash}
              disabled={selected.size === 0 || adding || trashing}
              className="px-4 py-2 text-sm bg-red-600/20 text-red-400 rounded hover:bg-red-600/30 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              title={t("newRoms.trashHelp")}
            >
              {trashing ? "..." : t("newRoms.trash", { count: selected.size })}
            </button>
          )}
          {roms.length > 0 && (
            <button
              onClick={handleAdd}
              disabled={selected.size === 0 || adding || trashing}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {adding ? "..." : t("newRoms.add", { count: selected.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
