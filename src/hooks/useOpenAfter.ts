import { useEffect, useState } from "react";
import { useAppStore } from "../store/useAppStore";

/** Lo que hay que quedarse quieto en una caja para que se abra y salga su vídeo */
export const BOX_OPEN_DELAY = 2000;

/**
 * Caja que se abre para enseñar el vídeo: devuelve `true` cuando `wanted` lleva `BOX_OPEN_DELAY`
 * seguido (con el mismo `key`: en el carrusel, la caja del centro). En cuanto `wanted` deja de ser
 * verdad o cambia `key`, vuelve a `false` al momento. No se abre con el editor, la ficha completa
 * u otra ventana encima (todas las ventanas de la app son capas «fixed inset-0»).
 */
export function useOpenAfter(wanted: boolean, key?: unknown): boolean {
  const [open, setOpen] = useState(false);
  const covered = useAppStore((s) => s.editorOpen || s.fullViewOpen);
  const active = wanted && !covered;

  useEffect(() => {
    setOpen(false);
    if (!active) return;
    const timer = window.setTimeout(() => {
      if (!document.querySelector(".fixed.inset-0")) setOpen(true);
    }, BOX_OPEN_DELAY);
    return () => window.clearTimeout(timer);
  }, [active, key]);

  return open && active;
}
