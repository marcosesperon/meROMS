import { useEffect, type ReactNode } from "react";
import { create } from "zustand";

/*
 * Pregunta con varias respuestas (los avisos del sistema solo tienen dos botones). Es una ventana
 * de meROMS: Esc, la X o un clic fuera dan la primera opción, que debe ser la que no cambia nada.
 */

export interface ChoiceOption {
  id: string;
  label: string;
  /** Botón rojo (borra algo) */
  danger?: boolean;
  /** Botón principal (violeta) */
  primary?: boolean;
}

interface ChoiceRequest {
  title: string;
  body: ReactNode;
  options: ChoiceOption[];
}

interface ChoiceState {
  request: ChoiceRequest | null;
  resolve: ((id: string) => void) | null;
  open: (request: ChoiceRequest) => Promise<string>;
  close: (id: string) => void;
}

const useChoice = create<ChoiceState>((set, get) => ({
  request: null,
  resolve: null,
  open: (request) =>
    new Promise<string>((resolve) => {
      // Si hubiera otra abierta (no debería), se da su primera opción
      const previous = get().request;
      if (previous) get().resolve?.(previous.options[0].id);
      set({ request, resolve });
    }),
  close: (id) => {
    const { resolve } = get();
    set({ request: null, resolve: null });
    resolve?.(id);
  },
}));

/** Enseña la pregunta y espera la respuesta (el id de la opción) */
export const askChoice = (request: ChoiceRequest) => useChoice.getState().open(request);

export function ChoiceDialog() {
  const request = useChoice((s) => s.request);
  const close = useChoice((s) => s.close);
  const safe = request?.options[0].id ?? "";

  // Esc = la primera opción (la que no cambia nada); se captura antes que la ventana de debajo
  useEffect(() => {
    if (!request) return;
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      close(safe);
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [request, close, safe]);

  if (!request) return null;

  return (
    <div className="fixed inset-0 z-[90] bg-black/60 flex items-center justify-center" onMouseDown={() => close(safe)}>
      <div
        role="alertdialog"
        aria-modal="true"
        className="w-[620px] max-w-[calc(100vw-2rem)] bg-zinc-800 border border-zinc-600 rounded-lg shadow-2xl shadow-black/60 flex flex-col"
        onMouseDown={(e) => e.stopPropagation()}
        data-shot="choice-dialog"
      >
        <div className="flex items-center justify-between px-5 pt-4">
          <h3 className="text-sm font-semibold text-zinc-100">{request.title}</h3>
          <button onClick={() => close(safe)} className="text-zinc-500 hover:text-zinc-200 text-lg leading-none" aria-label="×">
            ×
          </button>
        </div>
        <div className="px-5 py-4 text-sm text-zinc-300 max-h-[60vh] overflow-y-auto">{request.body}</div>
        <div className="flex flex-wrap justify-end gap-2 px-5 pb-4">
          {request.options.map((o) => (
            <button
              key={o.id}
              onClick={() => close(o.id)}
              className={`px-4 py-2 text-sm rounded ${
                o.danger
                  ? "bg-red-600 text-white hover:bg-red-500"
                  : o.primary
                    ? "bg-indigo-600 text-white hover:bg-indigo-500"
                    : "border border-zinc-600 text-zinc-300 hover:bg-zinc-700"
              }`}
            >
              {o.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
