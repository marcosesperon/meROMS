import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import { transferSystem } from "../../commands";
import type { MigrationMode, SystemInfo } from "../../types";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { getActiveProfile, isProfileComplete } from "../../utils/profiles";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "./TaskProgress";

interface TransferProfileModalProps {
  open: boolean;
  system: SystemInfo | null;
  onClose: () => void;
}

/** Une una carpeta y un nombre con la barra que ya use la carpeta (Windows o no) */
const joinPath = (dir: string, name: string) => `${dir.replace(/[\\/]+$/, "")}${dir.includes("\\") ? "\\" : "/"}${name}`;

/**
 * «Llevar a otro perfil…»: el sistema entero (ROMs, medios, gamelist y colecciones) pasa del
 * perfil activo a otro perfil guardado, moviendo o copiando. Nunca se sobrescribe nada: los
 * juegos que el destino ya tiene se saltan.
 */
export function TransferProfileModal({ open, system, onClose }: TransferProfileModalProps) {
  const { t } = useTranslation();
  const settings = useAppStore((s) => s.settings);
  const active = getActiveProfile(settings);
  // Los otros perfiles completos con otra carpeta de ROMs (con la misma, es «Migrar estructura»)
  const targets = useMemo(
    () => (settings.profiles ?? []).filter((p) => p.id !== active?.id && isProfileComplete(p) && p.rom_root_path !== active?.rom_root_path),
    [settings.profiles, active]
  );
  const [targetId, setTargetId] = useState("");
  const [mode, setMode] = useState<MigrationMode>("move");
  const task = useTask();

  useEffect(() => {
    if (open) {
      setTargetId(targets[0]?.id ?? "");
      setMode("move");
    }
  }, [open, targets]);

  if (!open || !system) return null;
  const target = targets.find((p) => p.id === targetId);
  const destination = target ? joinPath(target.rom_root_path, system.name) : "";

  async function start() {
    if (!system || !target) return;
    const kind = t(target.kind === "esde" ? "profiles.kindEsde" : "profiles.kindClassic");
    if (mode === "move" && !(await askConfirm(t("transfer.confirmMove", { count: system.game_count, system: system.full_name, profile: target.name, kind })))) {
      return;
    }
    try {
      const result = await task.run((id) => transferSystem(system.path, target.id, mode, id));
      if (!result) return;
      const lines = [t("transfer.done", { count: result.transferred, path: result.destination })];
      if (task.wasCancelled()) lines.unshift(t("task.stopped", { done: result.transferred, total: system.game_count }), "");
      if (result.media_unsupported > 0) lines.push(t("transfer.unsupported", { count: result.media_unsupported }));
      if (result.files_left > 0) lines.push(t("transfer.filesLeft", { count: result.files_left }));
      if (result.skipped.length > 0) lines.push("", t("transfer.skipped", { count: result.skipped.length }), ...result.skipped.slice(0, 30));
      await showMessage(lines.join("\n"));
      // Al mover, el sistema de este perfil ha cambiado: recargar sistemas y juegos
      const { loadSystems, loadGames, selectedSystem } = useAppStore.getState();
      await loadSystems();
      if (selectedSystem) loadGames(selectedSystem.path);
      onClose();
    } catch (e) {
      await showMessage(t("transfer.failed", { error: String(e) }), "error");
    }
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[520px] max-w-[calc(100vw-2rem)] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("transfer.title", { system: system.full_name })}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("transfer.help")}</p>
        </div>

        {task.running ? (
          <TaskProgress progress={task.progress} onCancel={task.cancel} cancelling={task.cancelling} />
        ) : targets.length === 0 ? (
          <p className="px-5 py-8 text-sm text-zinc-400 text-center">{t("transfer.noTargets")}</p>
        ) : (
          <div className="px-5 py-4 space-y-4">
            <label className="block">
              <span className="text-xs text-zinc-400">{t("transfer.profile")}</span>
              <select
                value={targetId}
                onChange={(e) => setTargetId(e.target.value)}
                className="mt-1 w-full bg-zinc-900 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
              >
                {targets.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} ({t(p.kind === "esde" ? "profiles.kindEsde" : "profiles.kindClassic")})
                  </option>
                ))}
              </select>
            </label>
            <div className="flex gap-4 text-sm text-zinc-300">
              {(["move", "copy"] as MigrationMode[]).map((m) => (
                <label key={m} className="flex items-center gap-2 cursor-pointer">
                  <input type="radio" checked={mode === m} onChange={() => setMode(m)} className="text-indigo-600" />
                  {t(m === "move" ? "transfer.move" : "transfer.copy")}
                </label>
              ))}
            </div>
            <p className="text-sm text-zinc-300">
              {t("transfer.summary", { count: system.game_count })}
              <span className="block font-mono text-xs text-zinc-500 break-all mt-1">{destination}</span>
            </p>
            <p className="text-xs text-zinc-500">{t(mode === "move" ? "transfer.moveHelp" : "transfer.copyHelp")}</p>
          </div>
        )}

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-end gap-3">
          <button onClick={onClose} disabled={task.running} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-50">
            {t("rename.close")}
          </button>
          {targets.length > 0 && (
            <button
              onClick={start}
              disabled={task.running || !target || system.game_count === 0}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50"
            >
              {t(mode === "move" ? "transfer.startMove" : "transfer.startCopy")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
