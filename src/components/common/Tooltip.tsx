import { useState, type ReactNode } from "react";

interface TooltipProps {
  text: string;
  children: ReactNode;
}

/**
 * Tooltip propio que sale al momento al pasar el ratón. El `title` del navegador tarda en
 * salir (y en el WebView de macOS a veces ni aparece), y un tooltip normal quedaría recortado
 * dentro de la barra lateral (que no deja que nada sobresalga): este se pinta con posición
 * fija respecto a la ventana, encima del elemento.
 */
export function Tooltip({ text, children }: TooltipProps) {
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  return (
    <span
      className="inline-flex"
      onMouseEnter={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        setPos({ left: r.left, top: r.top });
      }}
      onMouseLeave={() => setPos(null)}
    >
      {children}
      {pos && (
        <span
          role="tooltip"
          className="fixed z-[70] -translate-y-full -mt-2 max-w-[220px] px-2.5 py-1.5 rounded-md bg-zinc-800 border border-zinc-700 text-xs text-zinc-200 shadow-xl pointer-events-none"
          style={{ left: pos.left, top: pos.top }}
        >
          {text}
        </span>
      )}
    </span>
  );
}
