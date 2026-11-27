import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { deleteSystem, planDeleteSystem, type DeleteSystemPlan } from "../../commands";
import { useTask } from "../../hooks/useTask";
import { useAppStore } from "../../store/useAppStore";
import { showMessage } from "../../utils/dialogs";
import { formatBytes } from "../../utils/format";
import { TaskProgress } from "./TaskProgress";
import { TrashIcon, WarningIcon } from "./Icons";
import { useBlockClose } from "../../hooks/useBlockClose";
import { CloseButton } from "./CloseButton";

interface DeleteSystemModalProps {
  systemPath: string;
  onClose: () => void;
}

/**
 * «Eliminar sistema…»: enseña lo que se iría (juegos, medios, tamaño, carpetas), deja elegir
 * papelera o borrado definitivo y tres extras (carpetas de emuladores, partidas, lo de fuera), y
 * pide escribir el nombre de la carpeta del sistema para activar el botón rojo. Al terminar,
 * vuelve a la pantalla de inicio y recarga la barra de sistemas.
 */
export function DeleteSystemModal({ systemPath, onClose }: DeleteSystemModalProps) {
  const { t } = useTranslation();
  const [plan, setPlan] = useState<DeleteSystemPlan | null>(null);
  const [permanent, setPermanent] = useState(false);
  const [emulators, setEmulators] = useState(false);
  const [saves, setSaves] = useState(false);
  const [external, setExternal] = useState(false);
  const [typed, setTyped] = useState("");
  const task = useTask();
  // Mientras borra no se puede salir de la ventana (Cancelar sigue en la barra)
  useBlockClose(task.running);

  useEffect(() => {
    planDeleteSystem(systemPath)
      .then(setPlan)
      .catch(async (e) => {
        await showMessage(String(e), "error");
        onClose();
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [systemPath]);

  // Esc cierra, salvo mientras borra
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape" && !task.running) {
        e.stopPropagation();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose, task.running]);

  const confirmed = !!plan && typed.trim() === plan.name;

  async function run() {
    if (!plan || !confirmed || task.running) return;
    try {
      const result = await task.run((id) => deleteSystem(systemPath, { permanent, emulators, saves, external }, id));
      if (!result) return;
      const { goHome, loadSystems } = useAppStore.getState();
      goHome();
      await loadSystems();
      onClose();
      const lines = [t(result.stopped ? "deleteSystem.stopped" : "deleteSystem.done", { name: plan.name })];
      if (result.failed.length) lines.push("", t("deleteSystem.failed", { count: result.failed.length }), ...result.failed.slice(0, 20));
      await showMessage(lines.join("\n"), result.failed.length ? "error" : "info");
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  const extraBytes = (emulators ? (plan?.emulator_dirs.reduce((n, d) => n + d.bytes, 0) ?? 0) : 0) + (saves ? (plan?.saves_bytes ?? 0) : 0);

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onMouseDown={() => !task.running && onClose()}>
      <div
        className="bg-zinc-800 border border-red-900/60 rounded-lg w-[560px] max-w-[calc(100vw-2rem)] max-h-[88vh] flex flex-col"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-zinc-700 flex items-start gap-3">
          <span className="w-9 h-9 rounded-full bg-red-500/15 text-red-400 ring-1 ring-red-500/30 flex items-center justify-center shrink-0">
            <WarningIcon size={18} />
          </span>
          <div className="min-w-0">
            <h3 className="text-base font-semibold text-zinc-100">{t("deleteSystem.title", { name: plan?.name ?? "" })}</h3>
            <p className="text-sm text-zinc-400 mt-1">{t("deleteSystem.description")}</p>
          </div>
        </div>

        {task.running ? (
          <TaskProgress progress={task.progress} onCancel={task.cancel} cancelling={task.cancelling} />
        ) : !plan ? (
          <div className="py-12 text-center text-sm text-zinc-500">{t("deleteSystem.measuring")}</div>
        ) : (
          <div className="px-5 py-4 overflow-y-auto space-y-4 text-sm">
            {/* Lo que se va siempre */}
            <div className="rounded-md bg-zinc-900/60 px-3 py-2.5">
              <p className="text-zinc-200">
                {t("deleteSystem.always", { games: plan.games, media: plan.media_files, size: formatBytes(plan.bytes) })}
              </p>
              <ul className="mt-1.5 text-xs text-zinc-500 font-mono space-y-0.5 break-all">
                {[plan.rom_dir, plan.media_dir, plan.gamelist_dir].filter(Boolean).map((p) => (
                  <li key={p!}>{p}</li>
                ))}
              </ul>
            </div>

            {/* Papelera o definitivo */}
            <div className="flex flex-col gap-1.5">
              {[false, true].map((perm) => (
                <label key={String(perm)} className="flex items-center gap-2 cursor-pointer">
                  <input type="radio" checked={permanent === perm} onChange={() => setPermanent(perm)} />
                  <span className={perm ? "text-red-300" : "text-zinc-200"}>{t(perm ? "deleteSystem.permanent" : "deleteSystem.trash")}</span>
                </label>
              ))}
            </div>

            {/* Extras */}
            <div className="space-y-2">
              <Check
                checked={emulators}
                onChange={setEmulators}
                disabled={plan.emulator_dirs.length === 0}
                label={t("deleteSystem.emulators", { count: plan.emulator_dirs.length })}
                detail={plan.emulator_dirs.map((d) => `${d.name}/ (${formatBytes(d.bytes)})`).join(" · ")}
              />
              <Check
                checked={saves}
                onChange={setSaves}
                disabled={!plan.saves_dir}
                label={t("deleteSystem.saves", { count: plan.saves_files })}
                detail={plan.saves_dir ? `${plan.saves_dir} (${formatBytes(plan.saves_bytes)})` : t("deleteSystem.noSaves")}
              />
              <Check
                checked={external}
                onChange={setExternal}
                label={t("deleteSystem.external")}
                detail={t(plan.collection_lines ? "deleteSystem.externalDetailCollections" : "deleteSystem.externalDetail", { count: plan.collection_lines })}
              />
            </div>

            {/* Escribir el nombre */}
            <div>
              <p className="text-zinc-300">{t("deleteSystem.typeName", { name: plan.name })}</p>
              <input
                autoFocus
                value={typed}
                onChange={(e) => setTyped(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && run()}
                placeholder={plan.name}
                className="mt-1.5 w-full px-3 py-2 rounded bg-zinc-900 border border-zinc-600 text-zinc-100 font-mono focus:outline-none focus:border-red-500"
              />
            </div>
          </div>
        )}

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between items-center gap-3">
          <CloseButton onClick={onClose}>{t("rename.close")}</CloseButton>
          {plan && !task.running && (
            <button
              onClick={run}
              disabled={!confirmed}
              className="px-4 py-2 text-sm bg-red-600 text-white rounded hover:bg-red-500 disabled:opacity-40 disabled:cursor-not-allowed inline-flex items-center gap-2"
            >
              <TrashIcon size={14} />
              {t(permanent ? "deleteSystem.runPermanent" : "deleteSystem.runTrash", { size: formatBytes(plan.bytes + extraBytes) })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function Check({ checked, onChange, label, detail, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; detail: string; disabled?: boolean }) {
  return (
    <label className={`flex items-start gap-2 ${disabled ? "opacity-50" : "cursor-pointer"}`}>
      <input type="checkbox" className="mt-1" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      <span className="min-w-0">
        <span className="text-zinc-200">{label}</span>
        {detail && <span className="block text-xs text-zinc-500 break-words">{detail}</span>}
      </span>
    </label>
  );
}
