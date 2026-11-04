import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../../i18n";

interface ColumnsMenuProps {
  /** Every optional column (not thumbnail/name), with its label */
  all: { id: string; label: string }[];
  /** Visible ones, in order */
  visible: string[];
  onChange: (visible: string[]) => void;
  onReset: () => void;
}

/**
 * Menú «Columnas» de la tabla: mostrar u ocultar cada columna y cambiar su orden.
 * Se pinta con posición fija respecto a la ventana para que no lo recorte el scroll de la tabla.
 */
export function ColumnsMenu({ all, visible, onChange, onReset }: ColumnsMenuProps) {
  const { t } = useTranslation();
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!pos) return;
    const close = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setPos(null);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setPos(null);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [pos]);

  // Visibles primero (en su orden) y luego las ocultas
  const ordered = [...visible.map((id) => all.find((c) => c.id === id)!).filter(Boolean), ...all.filter((c) => !visible.includes(c.id))];

  const move = (id: string, d: number) => {
    const i = visible.indexOf(id);
    const next = [...visible];
    [next[i], next[i + d]] = [next[i + d], next[i]];
    onChange(next);
  };
  const toggle = (id: string) => onChange(visible.includes(id) ? visible.filter((v) => v !== id) : [...visible, id]);
  const btn = "w-5 h-5 rounded text-[11px] text-zinc-400 hover:text-zinc-100 hover:bg-zinc-700 disabled:opacity-20";

  return (
    <div ref={rootRef}>
      <button
        onClick={(e) => {
          e.stopPropagation();
          const r = e.currentTarget.getBoundingClientRect();
          setPos(pos ? null : { left: r.left, top: r.bottom + 4 });
        }}
        title={t("columns.title")}
        className="w-7 h-7 rounded text-zinc-400 hover:text-zinc-100 hover:bg-zinc-700 text-sm"
      >
        ⋮
      </button>
      {pos && (
        <div
          className="fixed z-[70] w-60 max-h-[70vh] overflow-y-auto py-2 bg-zinc-800 border border-zinc-600 rounded-lg shadow-xl normal-case font-normal"
          style={{ left: pos.left, top: pos.top }}
        >
          <p className="px-3 pb-1 text-[11px] uppercase tracking-wider text-zinc-500 font-semibold">{t("columns.title")}</p>
          {ordered.map((c) => {
            const shown = visible.includes(c.id);
            const i = visible.indexOf(c.id);
            return (
              <div key={c.id} className="flex items-center gap-2 px-3 py-1 hover:bg-zinc-700/50">
                <input
                  type="checkbox"
                  checked={shown}
                  onChange={() => toggle(c.id)}
                  className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                />
                <span className={`flex-1 text-xs ${shown ? "text-zinc-200" : "text-zinc-500"}`}>{c.label}</span>
                {shown && (
                  <>
                    <button className={btn} disabled={i === 0} onClick={() => move(c.id, -1)}>↑</button>
                    <button className={btn} disabled={i === visible.length - 1} onClick={() => move(c.id, 1)}>↓</button>
                  </>
                )}
              </div>
            );
          })}
          <div className="px-3 pt-2 mt-1 border-t border-zinc-700">
            <button onClick={onReset} className="text-xs text-indigo-400 hover:text-indigo-300">
              {t("columns.reset")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
