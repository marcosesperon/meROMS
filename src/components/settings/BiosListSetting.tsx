import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { biosDelete, biosDownload, biosStatus, type BiosListStatus } from "../../commands";
import { askConfirm, showMessage } from "../../utils/dialogs";

/**
 * Lista de BIOS (System.dat de libretro): descargarla, actualizarla o borrarla. No va dentro
 * de la app por su licencia (CC-BY-SA); sin ella, Herramientas → BIOS no puede comprobar nada.
 */
export function BiosListSetting() {
  const { t, locale } = useTranslation();
  const [status, setStatus] = useState<BiosListStatus | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    biosStatus().then(setStatus).catch(() => setStatus(null));
  }, []);

  async function download() {
    setBusy(true);
    try {
      setStatus(await biosDownload());
    } catch (e) {
      await showMessage(t("bios.downloadFailed", { error: String(e) }), "error");
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!(await askConfirm(t("bios.deleteConfirm")))) return;
    await biosDelete();
    setStatus(await biosStatus());
  }

  return (
    <div className="space-y-2">
      <p className="text-xs text-zinc-500">{t("bios.listHelp")}</p>
      <div className="flex items-center gap-3 text-xs">
        <p className="flex-1 text-zinc-400">
          {status?.downloaded
            ? t("bios.listCurrent", {
                files: status.files,
                date: status.downloaded_at ? new Date(status.downloaded_at * 1000).toLocaleDateString(locale) : "?",
              })
            : t("bios.listNone")}
        </p>
        <button onClick={download} disabled={busy} className="px-3 py-1.5 bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
          {busy ? "..." : status?.downloaded ? t("launchbox.update") : t("bios.download")}
        </button>
        {status?.downloaded && (
          <button onClick={remove} disabled={busy} className="px-3 py-1.5 bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
            {t("launchbox.delete")}
          </button>
        )}
      </div>
    </div>
  );
}
