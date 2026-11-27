import { useEffect } from "react";
import { useAppStore } from "../store/useAppStore";

/**
 * Mientras `busy` sea verdad, la ventana de la acción no se puede cerrar: cuenta un proceso en el
 * store y `common/CloseButton` (y las pestañas de «Medios») se desactivan hasta que acabe.
 * Va en el store y no en una prop porque una pestaña incrustada no conoce el Cerrar de la
 * ventana que la contiene.
 */
export function useBlockClose(busy: boolean) {
  const setBusyAction = useAppStore((s) => s.setBusyAction);
  useEffect(() => {
    if (!busy) return;
    setBusyAction(true);
    return () => setBusyAction(false);
  }, [busy, setBusyAction]);
}

/** ¿Hay alguna acción en marcha? (para desactivar Cerrar, la X o cambiar de pestaña) */
export function useActionBusy(): boolean {
  return useAppStore((s) => s.busyActions > 0);
}
