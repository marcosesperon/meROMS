import { useEffect } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";

/**
 * «Jugar» no ha podido: qué falta (el emulador o su core, con dónde se ha buscado, o que ES-DE
 * no conoce el sistema) y un botón para elegir el emulador en la vista del sistema.
 */
export function LaunchProblemModal() {
  const { t } = useTranslation();
  const problem = useAppStore((s) => s.launchProblem);
  const setProblem = useAppStore((s) => s.setLaunchProblem);
  const setSystemViewOpen = useAppStore((s) => s.setSystemViewOpen);
  const systemFolder = useAppStore((s) => s.selectedSystem?.name ?? "");
  const close = () => setProblem(null);

  // Esc (o B en el mando) la cierra
  useEffect(() => {
    if (!problem) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        setProblem(null);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [problem, setProblem]);

  if (!problem) return null;
  const { game, plan } = problem;
  const missing = plan.missing;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onMouseDown={close}>
      <div
        className="bg-zinc-800 border border-zinc-600 rounded-lg w-[520px] max-w-[calc(100vw-2rem)] flex flex-col mx-4"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("play.problemTitle", { name: game.name || game.path })}</h3>
        </div>
        <div className="px-5 py-4 space-y-3 text-sm text-zinc-300">
          {plan.no_config ? (
            <p>{t("play.noConfig", { system: systemFolder })}</p>
          ) : missing ? (
            <>
              <p>
                {t(missing.kind === "core" ? "play.missingCore" : "play.missingEmulator", { name: missing.name })}
                {plan.label && <span className="text-zinc-500"> ({plan.label})</span>}
              </p>
              {missing.searched.length > 0 && (
                <div>
                  <p className="text-xs text-zinc-400 mb-1">{t("play.searched")}</p>
                  <ul className="max-h-40 overflow-y-auto rounded bg-zinc-900 border border-zinc-700 px-3 py-2 font-mono text-[11px] text-zinc-400 space-y-0.5">
                    {missing.searched.map((path) => (
                      <li key={path} className="break-all">
                        {path}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              <p className="text-xs text-zinc-500">{t("play.missingHelp")}</p>
            </>
          ) : null}
        </div>
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-end gap-2">
          <button onClick={close} className="px-4 py-2 text-sm text-zinc-300 rounded border border-zinc-600 hover:bg-zinc-700">
            {t("play.close")}
          </button>
          <button
            onClick={() => {
              close();
              setSystemViewOpen(true);
            }}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500"
          >
            {t("play.configure")}
          </button>
        </div>
      </div>
    </div>
  );
}
