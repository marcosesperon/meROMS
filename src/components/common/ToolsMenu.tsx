import { useEffect, useRef, useState } from "react";

export interface ToolsMenuItem {
  label: string;
  onClick: () => void;
  title?: string;
  /** Pinta una línea de separación encima de esta opción */
  separatorBefore?: boolean;
}

interface ToolsMenuProps {
  label: string;
  items: ToolsMenuItem[];
}

/**
 * Botón con desplegable para las herramientas secundarias de la barra del sistema.
 * La barra está abajo del todo, así que la lista se abre hacia arriba.
 * Se cierra al elegir una opción, al hacer clic fuera o con Esc.
 */
export function ToolsMenu({ label, items }: ToolsMenuProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <div ref={rootRef} className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        data-shot="tools-menu"
        className={`px-3 py-1.5 text-sm rounded whitespace-nowrap transition-colors ${
          open ? "bg-zinc-600 text-zinc-100" : "bg-zinc-700 text-zinc-300 hover:bg-zinc-600"
        }`}
      >
        {label} <span className="text-xs">▾</span>
      </button>
      {open && (
        <div className="absolute right-0 bottom-full mb-2 min-w-[200px] py-1 bg-zinc-800 border border-zinc-600 rounded-lg shadow-xl z-40">
          {items.map((item) => (
            <div key={item.label}>
              {item.separatorBefore && <div className="my-1 border-t border-zinc-700" />}
              <button
                onClick={() => {
                  setOpen(false);
                  item.onClick();
                }}
                title={item.title}
                className="w-full text-left px-4 py-2 text-sm text-zinc-300 hover:bg-zinc-700 whitespace-nowrap"
              >
                {item.label}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
