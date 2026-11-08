import { useEffect, useRef } from "react";
import { create } from "zustand";
import { useTranslation } from "../../i18n";

/*
 * Aviso del resultado de una operación que se puede deshacer (renombrar, mover, normalizar…).
 * Es una ventana de meROMS y no el diálogo del sistema: en Windows el diálogo nativo dejaba
 * «Deshacer» como botón por defecto y bastaba pulsar Intro para deshacer lo recién hecho.
 * Aquí «Aceptar» es el botón principal y Intro, Esc, la X o un clic fuera nunca deshacen.
 */

interface ResultDialogState {
  message: string | null;
  resolve: ((undo: boolean) => void) | null;
  open: (message: string) => Promise<boolean>;
  close: (undo: boolean) => void;
}

const useResultDialog = create<ResultDialogState>((set, get) => ({
  message: null,
  resolve: null,
  open: (message) =>
    new Promise<boolean>((resolve) => {
      // Si hubiera uno abierto (no debería), se da por aceptado
      get().resolve?.(false);
      set({ message, resolve });
    }),
  close: (undo) => {
    const { resolve } = get();
    set({ message: null, resolve: null });
    resolve?.(undo);
  },
}));

/** Enseña el aviso y espera: true = el usuario ha pulsado «Deshacer» */
export const askUndo = (message: string) => useResultDialog.getState().open(message);

export function ResultDialog() {
  const { t } = useTranslation();
  const message = useResultDialog((s) => s.message);
  const close = useResultDialog((s) => s.close);
  const okRef = useRef<HTMLButtonElement>(null);

  // Esc = Aceptar. Se captura antes que nadie para que no cierre también la ventana de debajo
  useEffect(() => {
    if (message === null) return;
    okRef.current?.focus();
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      close(false);
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [message, close]);

  if (message === null) return null;

  return (
    <div className="fixed inset-0 z-[90] bg-black/60 flex items-center justify-center" onMouseDown={() => close(false)}>
      <div
        role="alertdialog"
        aria-modal="true"
        className="w-[440px] max-w-[calc(100vw-2rem)] bg-zinc-800 border border-zinc-600 rounded-lg shadow-2xl shadow-black/60 flex flex-col"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 pt-4">
          <h3 className="text-sm font-semibold text-zinc-100">meROMS</h3>
          <button onClick={() => close(false)} className="text-zinc-500 hover:text-zinc-200 text-lg leading-none" aria-label={t("dialog.ok")}>
            ×
          </button>
        </div>
        <p className="px-5 py-4 text-sm text-zinc-300 whitespace-pre-wrap max-h-[50vh] overflow-y-auto">{message}</p>
        <div className="flex justify-end gap-3 px-5 pb-4">
          <button
            onClick={() => close(true)}
            data-shot="result-undo"
            className="px-4 py-2 text-sm rounded border border-zinc-600 text-zinc-300 hover:bg-zinc-700"
          >
            {t("undo.undo")}
          </button>
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
