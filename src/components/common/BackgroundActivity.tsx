import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";

/** Lo que tiene que durar el trabajo antes de enseñar el aviso (lo rápido no parpadea) */
const SHOW_AFTER_MS = 300;

/**
 * Aviso discreto de trabajo en segundo plano del sistema abierto (formato de las tarjetas, ROMs
 * nuevas y desaparecidas, medios rotos, estados, logros, DAT): una línea animada arriba de la
 * lista y «Analizando…» en la esquina. No tapa nada ni recoge clics, y la lista se puede usar
 * mientras. Va dentro de un contenedor `relative`.
 */
export function BackgroundActivity() {
  const { t } = useTranslation();
  const busy = useAppStore((s) => s.backgroundJobs > 0);
  const [shown, setShown] = useState(false);
  useEffect(() => {
    if (!busy) {
      setShown(false);
      return;
    }
    const timer = setTimeout(() => setShown(true), SHOW_AFTER_MS);
    return () => clearTimeout(timer);
  }, [busy]);
  if (!shown) return null;

  return (
    <div data-background-activity className="absolute inset-x-0 top-0 z-20 pointer-events-none">
      <div className="h-0.5 overflow-hidden bg-indigo-500/15">
        <div className="h-full w-1/3 bg-indigo-400/80 animate-[task-slide_1.2s_ease-in-out_infinite]" />
      </div>
      <div className="absolute right-3 top-2 flex items-center gap-1.5 rounded-full bg-zinc-900/85 border border-white/10 px-2.5 py-1 text-[11px] text-zinc-300 shadow-lg shadow-black/40 backdrop-blur">
        <span className="w-2.5 h-2.5 rounded-full border-2 border-indigo-400/80 border-t-transparent animate-spin" />
        {t("gameList.analysing")}
      </div>
    </div>
  );
}
