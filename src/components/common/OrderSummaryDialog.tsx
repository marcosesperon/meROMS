import { useEffect, useRef, useState } from "react";
import { create } from "zustand";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import type { OrderStep } from "../../utils/orderSteps";
import { withoutZeros, type OutcomeItem } from "../../utils/outcome";
import { CheckIcon, CloseIcon, InfoIcon, MinusIcon, WarningIcon } from "./Icons";

/*
 * Resumen de «Ponerlo todo en orden» al terminar: una cabecera con cómo ha ido, un bloque por
 * paso con sus números destacados, los fallos plegados y la nota de la copia de seguridad.
 * Como el aviso de resultado (ResultDialog): «Aceptar» es el botón principal y ni Intro, ni
 * Esc, ni la X, ni un clic fuera deshacen nada.
 */

/** Lo que hizo un paso */
export interface StepSummary {
  step: OrderStep;
  items: OutcomeItem[];
  /** El paso entero falló (con el motivo) */
  error?: string;
}

export interface OrderSummary {
  steps: StepSummary[];
  /** Se paró a medias */
  stopped: boolean;
  /** Lo que no se pudo hacer (de cualquier paso) */
  failed: string[];
  /** Hay algo que deshacer (si no, sin botón «Deshacer») */
  canUndo: boolean;
}

interface SummaryState {
  summary: OrderSummary | null;
  resolve: ((undo: boolean) => void) | null;
  open: (summary: OrderSummary) => Promise<boolean>;
  close: (undo: boolean) => void;
}

const useSummaryDialog = create<SummaryState>((set, get) => ({
  summary: null,
  resolve: null,
  open: (summary) =>
    new Promise<boolean>((resolve) => {
      get().resolve?.(false);
      set({ summary, resolve });
    }),
  close: (undo) => {
    const { resolve } = get();
    set({ summary: null, resolve: null });
    resolve?.(undo);
  },
}));

/** Enseña el resumen y espera: true = el usuario ha pulsado «Deshacer» */
export const showOrderSummary = (summary: OrderSummary) => useSummaryDialog.getState().open(summary);

/** Cómo ha ido en general: color, icono y texto de la cabecera */
function overall(summary: OrderSummary) {
  const didSomething = summary.steps.some((s) => withoutZeros(s.items).length > 0);
  if (summary.stopped) return { tone: "amber", key: "order.sumStopped" as const };
  if (summary.failed.length || summary.steps.some((s) => s.error)) return { tone: "amber", key: "order.sumWithErrors" as const };
  if (!didSomething) return { tone: "zinc", key: "order.sumNothing" as const };
  return { tone: "green", key: "order.sumAllDone" as const };
}

const TONE = {
  green: { badge: "bg-emerald-500/15 text-emerald-400 ring-emerald-500/30", text: "text-emerald-400" },
  amber: { badge: "bg-amber-500/15 text-amber-400 ring-amber-500/30", text: "text-amber-400" },
  red: { badge: "bg-red-500/15 text-red-400 ring-red-500/30", text: "text-red-400" },
  zinc: { badge: "bg-zinc-700/60 text-zinc-400 ring-zinc-600/50", text: "text-zinc-500" },
};

export function OrderSummaryDialog() {
  const { t } = useTranslation();
  const summary = useSummaryDialog((s) => s.summary);
  const close = useSummaryDialog((s) => s.close);
  const okRef = useRef<HTMLButtonElement>(null);
  const [showFailed, setShowFailed] = useState(false);

  // Esc = Aceptar. Se captura antes que nadie para que no cierre también la ventana de debajo
  useEffect(() => {
    if (summary === null) return;
    setShowFailed(false);
    okRef.current?.focus();
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      close(false);
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [summary, close]);

  if (summary === null) return null;
  const head = overall(summary);
  const HeadIcon = head.tone === "green" ? CheckIcon : head.tone === "amber" ? WarningIcon : MinusIcon;

  return (
    <div className="fixed inset-0 z-[90] bg-black/60 flex items-center justify-center" onMouseDown={() => close(false)}>
      <div
        role="alertdialog"
        aria-modal="true"
        className="w-[480px] max-w-[calc(100vw-2rem)] max-h-[85vh] bg-zinc-800 border border-zinc-600 rounded-lg shadow-2xl shadow-black/60 flex flex-col"
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* Cabecera: cómo ha ido */}
        <div className="flex items-center gap-3 px-5 pt-4 pb-3 border-b border-zinc-700">
          <span className={`w-9 h-9 rounded-full ring-1 flex items-center justify-center shrink-0 ${TONE[head.tone as keyof typeof TONE].badge}`}>
            <HeadIcon size={18} />
          </span>
          <div className="flex-1 min-w-0">
            <h3 className="text-base font-semibold text-zinc-100">{t(head.key)}</h3>
            <p className="text-xs text-zinc-500">
              {t("order.title" as TranslationKeys)}
              {summary.stopped ? ` · ${t("order.sumStoppedNote")}` : ""}
            </p>
          </div>
          <button onClick={() => close(false)} className="self-start text-zinc-500 hover:text-zinc-200" aria-label={t("dialog.ok")}>
            <CloseIcon size={16} />
          </button>
        </div>

        <div className="px-5 py-3 overflow-y-auto flex flex-col gap-2.5">
          {summary.steps.map((s) => (
            <StepBlock key={s.step} summary={s} />
          ))}

          {/* Fallos: en rojo, plegados */}
          {summary.failed.length > 0 && (
            <div className="rounded-md bg-red-500/10 ring-1 ring-red-500/25 px-3 py-2">
              <button onClick={() => setShowFailed((v) => !v)} className="w-full flex items-center gap-2 text-sm text-red-400">
                <CloseIcon size={14} />
                <span className="flex-1 text-left">{t("order.sumFailed", { count: summary.failed.length })}</span>
                <span className="text-xs underline underline-offset-2">{t(showFailed ? "order.sumHide" : "order.sumShow")}</span>
              </button>
              {showFailed && (
                <ul className="mt-2 max-h-40 overflow-y-auto text-xs text-red-300/90 font-mono space-y-0.5">
                  {summary.failed.slice(0, 50).map((f, i) => (
                    <li key={i} className="break-all">
                      {f}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}

          {/* Nota de la copia de seguridad y la papelera */}
          <p className="flex gap-2 text-[11px] leading-snug text-zinc-500 pt-1">
            <InfoIcon size={13} className="shrink-0 mt-px" />
            <span>{t("order.backupNote")}</span>
          </p>
        </div>

        <div className="flex justify-end gap-3 px-5 py-3 border-t border-zinc-700">
          {summary.canUndo && (
            <button onClick={() => close(true)} className="px-4 py-2 text-sm rounded border border-zinc-600 text-zinc-300 hover:bg-zinc-700">
              {t("undo.undo")}
            </button>
          )}
          <button
            ref={okRef}
            onClick={() => close(false)}
            className="px-4 py-2 text-sm rounded bg-indigo-600 text-white hover:bg-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-400"
          >
            {t("dialog.ok")}
          </button>
        </div>
      </div>
    </div>
  );
}

/** Un paso: ✓ verde si hizo algo, ✖ rojo si falló, «–» gris si no había nada que hacer */
function StepBlock({ summary }: { summary: StepSummary }) {
  const { t } = useTranslation();
  const items = withoutZeros(summary.items);
  const tone = summary.error ? "red" : items.length ? "green" : "zinc";
  const Icon = tone === "green" ? CheckIcon : tone === "red" ? CloseIcon : MinusIcon;
  return (
    <div className="flex gap-3">
      <span className={`w-6 h-6 mt-0.5 rounded-full ring-1 flex items-center justify-center shrink-0 ${TONE[tone].badge}`}>
        <Icon size={13} />
      </span>
      <div className="flex-1 min-w-0">
        <p className={`text-sm font-medium ${tone === "zinc" ? "text-zinc-400" : "text-zinc-100"}`}>
          {t(`order.step_${summary.step}` as TranslationKeys)}
          {tone === "zinc" && <span className="font-normal text-zinc-500"> · {t("order.sumStepNothing")}</span>}
        </p>
        {summary.error && <p className="text-xs text-red-400 break-words">{summary.error}</p>}
        {items.length > 0 && (
          <div className="mt-1 flex flex-wrap gap-x-4 gap-y-1">
            {items.map((item, i) => (
              <ItemText key={i} item={item} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

/** Marca para encontrar dónde va el número dentro del texto traducido */
const NUMBER_MARK = "\u0001";

/** «30 ficheros movidos» con el 30 destacado, donde lo ponga la traducción */
function ItemText({ item }: { item: OutcomeItem }) {
  const { t } = useTranslation();
  // Sin el punto final de algunos textos («10 versiones ocultas.»): aquí van en fila
  const text = t(item.key, { ...item.params, count: NUMBER_MARK }).replace(/\.\s*$/, "");
  const [before, after] = item.count === undefined ? [text, null] : text.split(NUMBER_MARK);
  return (
    <span className="text-xs text-zinc-400">
      {before}
      {after !== null && (
        <>
          <span className="text-lg font-semibold tabular-nums text-zinc-100 align-[-1px]">{item.count}</span>
          {after}
        </>
      )}
    </span>
  );
}
