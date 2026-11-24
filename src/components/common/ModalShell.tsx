import type { ReactNode } from "react";

/**
 * Marco de una ventana de herramienta. Suelta: fondo oscuro y caja centrada. Dentro de otra
 * ventana (`embedded`, p. ej. una pestaña de «Medios»): solo el contenido, que ocupa el sitio.
 */
export function ModalShell({ embedded, className, children }: { embedded?: boolean; className: string; children: ReactNode }) {
  if (embedded) return <div className="flex-1 min-h-0 flex flex-col">{children}</div>;
  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className={`bg-zinc-800 border border-zinc-600 rounded-lg flex flex-col mx-4 ${className}`}>{children}</div>
    </div>
  );
}
