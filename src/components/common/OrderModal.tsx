import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { backupLabeled, cancelScrape, cancelTask, undoGroupBegin, undoGroupEnd } from "../../commands";
import { newTaskId, subscribeTask, type TaskProgressInfo } from "../../hooks/useTask";
import { useAppStore, type ToolRequest } from "../../store/useAppStore";
import type { ScrapeProgress } from "../../types";
import { collectionRulesFrom } from "../../utils/collectionRules";
import { bumpMediaVersion } from "../../utils/images";
import { ORDER_STEPS, STEP_APPLIERS, STEP_PLANNERS, type OrderContext, type OrderStep, type StepPlan } from "../../utils/orderSteps";
import { getActiveProfile } from "../../utils/profiles";
import { renameOptionsFrom } from "../../utils/renameSources";
import { runUndo } from "../../utils/undo";
import { showOrderSummary, type StepSummary } from "./OrderSummaryDialog";
import { TaskProgress } from "./TaskProgress";

interface OrderModalProps {
  open: boolean;
  systemPath: string;
  /** Pasos marcados al abrir («Arreglar» de una línea del estado); sin ellos, todos */
  steps?: string[];
  /** Hay alguna fuente de scrapeo para este sistema */
  canScrape: boolean;
  /** Lo que la lista enseña ahora (búsqueda o filtros) y lo marcado en la selección múltiple */
  visiblePaths: string[];
  markedPaths: string[];
  totalGames: number;
  /** Ámbito con el que se abre (desde el panel de selección múltiple: los marcados) */
  initialScope?: Scope;
  onClose: () => void;
}

type Scope = "all" | "visible" | "marked";

/** Herramienta que enseña el detalle de cada paso («Ver todo») */
const STEP_TOOL: Record<OrderStep, ToolRequest> = {
  roms: { id: "newRoms" },
  versions: { id: "versions" },
  media: { id: "media" },
  scrape: { id: "scrapeAll" },
  names: { id: "rename" },
};

/**
 * «Ponerlo todo en orden»: los pasos de `utils/orderSteps.ts` con lo que va a hacer cada uno
 * (números y ejemplos), una casilla para quitarlo y «Ver todo» para revisarlo en su herramienta.
 * Antes de empezar se guarda una copia de la gamelist marcada («antes de Ponerlo todo en orden»)
 * y todo lo que mueve ficheros queda en un solo Deshacer. Al terminar, un único resumen.
 */
export function OrderModal({ open, systemPath, steps, canScrape, visiblePaths, markedPaths, totalGames, initialScope, onClose }: OrderModalProps) {
  const { t } = useTranslation();
  const openTool = useAppStore((s) => s.openTool);
  const [checked, setChecked] = useState<Set<OrderStep>>(new Set(ORDER_STEPS));
  const [plans, setPlans] = useState<Partial<Record<OrderStep, StepPlan | "error">>>({});
  const [scope, setScope] = useState<Scope>("all");
  // Scrapear también los que el scraper ya no encontró (casilla del paso de scrapeo)
  const [retryMisses, setRetryMisses] = useState(false);
  const [running, setRunning] = useState<OrderStep | null>(null);
  const [progress, setProgress] = useState<TaskProgressInfo | null>(null);
  const stopRef = useRef(false);
  const taskRef = useRef<string | null>(null);
  const [stopping, setStopping] = useState(false);

  // Con la lista filtrada o con juegos marcados se ofrece hacerlo solo con esos
  const canVisible = visiblePaths.length > 0 && visiblePaths.length < totalGames;
  const canMarked = markedPaths.length > 0;

  function context(): OrderContext {
    const { settings, scraperStatus } = useAppStore.getState();
    return {
      systemPath,
      rules: collectionRulesFrom(settings.collection_rules),
      renameOptions: renameOptionsFrom(settings.rename_options),
      scraperStatus,
      canScrape,
      isEsde: getActiveProfile(settings)?.kind === "esde",
      scope: scope === "visible" ? new Set(visiblePaths) : scope === "marked" ? new Set(markedPaths) : null,
      retryMisses,
    };
  }

  // Al abrir: los pasos pedidos (o todos) y el ámbito por defecto (lo marcado, si hay)
  useEffect(() => {
    if (!open) return;
    setChecked(new Set(steps?.length ? (steps.filter((s) => ORDER_STEPS.includes(s as OrderStep)) as OrderStep[]) : ORDER_STEPS));
    setScope(initialScope === "marked" && markedPaths.length > 0 ? "marked" : "all");
    setRetryMisses(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  // Vista previa de cada paso: primero los marcados (a la vez) y después los demás, para que
  // lo que se va a aplicar no espere a lo que no. Se rehace al cambiar el ámbito.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setPlans({});
    const ctx = context();
    const plan = (step: OrderStep) =>
      STEP_PLANNERS[step](ctx)
        .then((p) => !cancelled && setPlans((prev) => ({ ...prev, [step]: p })))
        .catch((e) => {
          console.error(`Order plan ${step} failed:`, e);
          if (!cancelled) setPlans((prev) => ({ ...prev, [step]: "error" }));
        });
    const wanted = steps?.length ? ORDER_STEPS.filter((x) => steps.includes(x)) : ORDER_STEPS;
    const rest = ORDER_STEPS.filter((x) => !wanted.includes(x));
    Promise.all(wanted.map(plan)).then(() => !cancelled && rest.forEach(plan));
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, scope, systemPath, retryMisses]);

  // Progreso del scrapeo (no usa las tareas: tiene su propio aviso)
  useEffect(() => {
    if (running !== "scrape") return;
    const unlisten = listen<ScrapeProgress>("scrape-progress", (e) =>
      setProgress({ phase: "", current: e.payload.current, total: e.payload.total, item: e.payload.game_name })
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, [running]);

  if (!open) return null;

  const planOf = (step: OrderStep) => plans[step];
  // Se puede aplicar mientras su vista previa no diga lo contrario (nada que hacer, bloqueado o
  // error): al aplicar, cada paso vuelve a mirarlo todo, así que no hace falta esperarla
  const ready = (step: OrderStep) => {
    const p = planOf(step);
    return !p || (p !== "error" && p.count > 0 && !p.blocked);
  };
  const toRun = ORDER_STEPS.filter((s) => checked.has(s) && ready(s));

  function toggle(step: OrderStep) {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(step)) next.delete(step);
      else next.add(step);
      return next;
    });
  }

  async function stop() {
    stopRef.current = true;
    setStopping(true);
    if (running === "scrape") await cancelScrape().catch(() => {});
    if (taskRef.current) await cancelTask(taskRef.current).catch(() => {});
  }

  async function apply() {
    const { loadGames, refreshUndo, loadSystems } = useAppStore.getState();
    stopRef.current = false;
    setStopping(false);
    const summary: StepSummary[] = [];
    const failed: string[] = [];
    // La suscripción al progreso de la operación en curso (una a la vez)
    const listening: { stop: (() => void) | null } = { stop: null };
    const runner = {
      // Cada operación con su id de tarea: así se ve su progreso y se puede parar
      task: () => {
        listening.stop?.();
        const id = newTaskId();
        taskRef.current = id;
        listening.stop = subscribeTask(id, setProgress);
        return id;
      },
      stopped: () => stopRef.current,
    };
    try {
      await backupLabeled(systemPath, "order");
      await undoGroupBegin();
      for (const step of toRun) {
        if (stopRef.current) break;
        setRunning(step);
        setProgress(null);
        try {
          // Cada paso mira la gamelist como la dejó el anterior
          const out = await STEP_APPLIERS[step](context(), runner);
          summary.push({ step, items: out.items });
          failed.push(...out.failed);
        } catch (e) {
          summary.push({ step, items: [], error: String(e) });
        }
        await loadGames(systemPath, { skipStatus: true });
      }
    } catch (e) {
      failed.push(String(e));
    } finally {
      listening.stop?.();
      taskRef.current = null;
      await undoGroupEnd(systemPath).catch(() => {});
      setRunning(null);
      setProgress(null);
      bumpMediaVersion();
      await loadGames(systemPath, { mediaChanged: true }).catch(() => {});
      loadSystems();
      await refreshUndo();
    }
    onClose();
    // El resumen con un bloque por paso (OrderSummaryDialog); «Deshacer», como en los demás avisos
    const undo = await showOrderSummary({ steps: summary, stopped: stopRef.current, failed, canUndo: !!useAppStore.getState().undoInfo });
    if (undo) await runUndo();
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[720px] max-w-[calc(100vw-2rem)] max-h-[88vh] flex flex-col mx-4" data-shot="order-modal">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("order.title")}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("order.help")}</p>
          {(canVisible || canMarked) && !running && (
            <div className="flex flex-wrap gap-x-5 gap-y-1 mt-3 text-sm text-zinc-300">
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="radio" checked={scope === "all"} onChange={() => setScope("all")} className="text-indigo-600" />
                {t("order.scopeAll", { count: totalGames })}
              </label>
              {canVisible && (
                <label className="flex items-center gap-2 cursor-pointer">
                  <input type="radio" checked={scope === "visible"} onChange={() => setScope("visible")} className="text-indigo-600" />
                  {t("order.scopeVisible", { count: visiblePaths.length })}
                </label>
              )}
              {canMarked && (
                <label className="flex items-center gap-2 cursor-pointer">
                  <input type="radio" checked={scope === "marked"} onChange={() => setScope("marked")} className="text-indigo-600" />
                  {t("order.scopeMarked", { count: markedPaths.length })}
                </label>
              )}
            </div>
          )}
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-3 space-y-2">
          {ORDER_STEPS.map((step, i) => {
            const plan = planOf(step);
            const isRunning = running === step;
            const done = !!running && toRun.indexOf(step) < toRun.indexOf(running) && toRun.includes(step);
            const nothing = !!plan && plan !== "error" && plan.count === 0;
            return (
              <div
                key={step}
                className={`rounded border px-3 py-2.5 ${isRunning ? "border-indigo-500 bg-indigo-600/10" : "border-zinc-700 bg-zinc-900/40"} ${nothing ? "opacity-60" : ""}`}
              >
                <div className="flex items-start gap-2">
                  <input
                    type="checkbox"
                    checked={checked.has(step) && ready(step)}
                    disabled={!ready(step) || !!running}
                    onChange={() => toggle(step)}
                    className="mt-0.5 rounded border-zinc-600 bg-zinc-700 text-indigo-600 disabled:opacity-30"
                  />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium text-zinc-200">
                      {i + 1}. {t(`order.step_${step}` as TranslationKeys)}
                      {done && <span className="ml-2 text-xs text-emerald-400">✓</span>}
                    </p>
                    <p className="text-[11px] text-zinc-500">{t(`order.why_${step}` as TranslationKeys)}</p>
                    {!plan ? (
                      <p className="text-xs text-zinc-500 mt-1">{t("order.planning")}</p>
                    ) : plan === "error" ? (
                      <p className="text-xs text-red-400 mt-1">{t("order.planFailed")}</p>
                    ) : plan.count === 0 ? (
                      <p className="text-xs text-emerald-400 mt-1">{t("order.nothingToDo")}</p>
                    ) : (
                      <>
                        {plan.lines.map((line) => (
                          <p key={line} className="text-xs text-zinc-300 mt-1">
                            {line}
                          </p>
                        ))}
                        {plan.examples.length > 0 && (
                          <p className="text-[11px] text-zinc-500 font-mono mt-1 truncate" title={plan.examples.join("\n")}>
                            {plan.examples.join(" · ")}
                            {plan.count > plan.examples.length ? " …" : ""}
                          </p>
                        )}
                        {plan.blocked && <p className="text-xs text-amber-400 mt-1">{plan.blocked}</p>}
                      </>
                    )}
                    {/* Scrapeo: los que el scraper ya no encontró se saltan, salvo que se pida */}
                    {step === "scrape" && plan && plan !== "error" && (retryMisses || !!plan.skipped) && (
                      <label className="mt-1.5 flex items-center gap-2 text-xs text-zinc-400 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={retryMisses}
                          disabled={!!running}
                          onChange={(e) => setRetryMisses(e.target.checked)}
                          className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                        />
                        {retryMisses ? t("scrapeMiss.retry") : t("scrapeMiss.retryCount", { count: plan.skipped ?? 0 })}
                      </label>
                    )}
                    {isRunning && (
                      <div className="mt-2">
                        <TaskProgress progress={progress} compact onCancel={stop} cancelling={stopping} />
                      </div>
                    )}
                  </div>
                  {!running && plan && plan !== "error" && plan.count > 0 && (
                    <button
                      onClick={() => {
                        onClose();
                        openTool(STEP_TOOL[step]);
                      }}
                      className="shrink-0 text-xs text-indigo-400 hover:text-indigo-300"
                    >
                      {t("order.viewAll")}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex items-center justify-between gap-3">
          <button onClick={onClose} disabled={!!running} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-50">
            {t("rename.close")}
          </button>
          <span className="text-[11px] text-zinc-500 flex-1 text-right">{t("order.backupHint")}</span>
          <button
            onClick={apply}
            disabled={!!running || toRun.length === 0}
            data-shot="order-apply"
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {running ? "…" : t("order.apply", { count: toRun.length })}
          </button>
        </div>
      </div>
    </div>
  );
}
