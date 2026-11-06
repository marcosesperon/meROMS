import { useEffect, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import type { Game } from "../../types";
import { backupSaves, importSave, listSaves, trashSave, type SaveFile } from "../../commands";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { formatBytes } from "../../utils/format";
import { resolveImageSrc } from "../../utils/images";
import { TrashIcon } from "../common/Icons";

/**
 * Pestaña «Datos guardados» de la ficha a pantalla completa: partidas y estados del juego en la
 * carpeta de saves del perfil, con copia de seguridad, borrar e importar.
 */
export function SavesTab({ game, systemPath }: { game: Game; systemPath: string }) {
  const { t } = useTranslation();
  const [data, setData] = useState<{ dir: string | null; files: SaveFile[] } | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function load() {
    setError(null);
    try {
      setData(await listSaves(systemPath, game.path));
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    setData(null);
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.path, systemPath]);

  // Ir a Ajustes (la carpeta se elige en el perfil): se cierra la ficha para verlos
  function openSettings() {
    const { closeFullView, setCurrentView } = useAppStore.getState();
    closeFullView();
    setCurrentView("settings");
  }

  async function backup() {
    const dest = await openDialog({ directory: true, multiple: false });
    if (!dest) return;
    try {
      const zip = await backupSaves(systemPath, game.path, String(dest));
      await showMessage(t("saves.backupDone", { file: zip }));
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  async function importOne() {
    const file = await openDialog({ directory: false, multiple: false });
    if (!file) return;
    try {
      await importSave(systemPath, game.path, String(file));
      await load();
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  async function remove(f: SaveFile) {
    if (!(await askConfirm(t("saves.confirmDelete", { name: f.name })))) return;
    try {
      await trashSave(f.path);
      await load();
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  if (error) return <p className="text-sm text-red-300">{error}</p>;
  if (!data) return <p className="text-sm text-zinc-500">{t("saves.loading")}</p>;
  if (!data.dir) {
    return (
      <div className="py-8 text-center">
        <p className="text-sm text-zinc-300">{t("saves.noFolder")}</p>
        <button onClick={openSettings} className="mt-4 px-4 py-2 text-sm bg-indigo-600 text-white rounded-md hover:bg-indigo-500">
          {t("welcome.openSettings")}
        </button>
      </div>
    );
  }

  const label = (f: SaveFile) =>
    f.kind === "save" ? t("saves.save") : f.slot === -1 ? t("saves.stateAuto") : t("saves.state", { n: f.slot ?? 0 });
  const btn = "px-3 py-1.5 text-xs bg-zinc-800 text-zinc-200 rounded hover:bg-zinc-700 disabled:opacity-40";

  return (
    <div className="h-full flex flex-col min-h-0">
      <div className="flex flex-wrap items-center gap-2 shrink-0">
        <button onClick={() => revealItemInDir(data.files[0]?.path ?? data.dir!).catch(() => {})} className={btn}>
          {t("saves.openFolder")}
        </button>
        <button onClick={backup} disabled={data.files.length === 0} className={btn}>
          {t("saves.backup")}
        </button>
        <button onClick={importOne} className={btn}>
          {t("saves.import")}
        </button>
        <span className="ml-auto text-[11px] text-zinc-500 font-mono truncate" title={data.dir}>{data.dir}</span>
      </div>

      {data.files.length === 0 ? (
        <p className="py-10 text-center text-sm text-zinc-500">{t("saves.none")}</p>
      ) : (
        <div className="mt-3 flex-1 min-h-0 overflow-y-auto space-y-2 pr-1">
          {data.files.map((f) => {
            const thumb = f.thumbnail ? resolveImageSrc(f.thumbnail, systemPath) : undefined;
            return (
              <div key={f.path} className="flex items-center gap-3 p-2 rounded-md bg-zinc-900/80">
                <div className="w-20 h-14 shrink-0 rounded bg-black/60 overflow-hidden flex items-center justify-center text-[10px] text-zinc-600">
                  {thumb ? <img src={thumb} alt="" className="w-full h-full object-cover" /> : f.kind === "save" ? "SAVE" : "STATE"}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="text-sm text-zinc-200">{label(f)}</p>
                  <p className="text-[11px] text-zinc-500 truncate" title={f.path}>
                    {f.name} · {new Date(f.modified * 1000).toLocaleString()} · {formatBytes(f.size)}
                  </p>
                </div>
                <button onClick={() => remove(f)} title={t("saves.delete")} className="p-2 rounded text-red-400 hover:bg-zinc-800">
                  <TrashIcon size={14} />
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
