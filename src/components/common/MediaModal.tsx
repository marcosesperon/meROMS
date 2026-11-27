import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import type { ScrapePolicy } from "../../types";
import { MediaReviewModal } from "./MediaReviewModal";
import { NormalizeMediaModal } from "./NormalizeMediaModal";
import { UnusedMediaModal } from "./UnusedMediaModal";
import { useActionBusy } from "../../hooks/useBlockClose";

export type MediaTab = "damaged" | "unused" | "swapped" | "legacy";
// En el orden en que conviene revisarlo: primero colocar lo que ya hay, después quitar lo que sobra
const TABS: MediaTab[] = ["legacy", "swapped", "damaged", "unused"];

interface MediaModalProps {
  open: boolean;
  systemPath: string;
  /** Pestaña con la que se abre (la línea del estado que se ha pulsado) */
  tab?: string;
  onClose: () => void;
  /** Tras cambiar medios (recargar los juegos sin la caché de imágenes) */
  onDone: () => void;
  /** «Intercambiar tipos de medio» (todo el sistema), desde la pestaña de cambiados */
  onOpenSwap: () => void;
  /** Volver a descargar los dañados que se han quitado */
  onRescrape?: (gamePaths: string[], policy: ScrapePolicy) => void;
}

/**
 * «Medios» (antes, cuatro entradas sueltas en Herramientas): una ventana con pestañas, en el
 * orden en que conviene arreglarlo (lo que se coloca antes de lo que se quita):
 *  - Nombres antiguos: lo que «Normalizar» lleva a su sitio (también por el título del juego).
 *  - Cambiados de tipo: carátulas y capturas al revés (y, para todo el sistema, Intercambiar).
 *  - Dañados y placeholders (y rutas rotas): lo que hay que quitar y volver a bajar.
 *  - Sobrantes (y copias repetidas en varios tipos): lo que no usa ningún juego.
 * Cada pestaña es la herramienta de siempre, sin su marco.
 */
export function MediaModal({ open, systemPath, tab, onClose, onDone, onOpenSwap, onRescrape }: MediaModalProps) {
  const { t } = useTranslation();
  const [current, setCurrent] = useState<MediaTab>("legacy");
  // Mientras una pestaña aplica algo no se cambia de pestaña (se desmontaría a medias)
  const busy = useActionBusy();

  useEffect(() => {
    if (open) setCurrent(TABS.includes(tab as MediaTab) ? (tab as MediaTab) : "legacy");
  }, [open, tab]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[860px] max-w-[calc(100vw-2rem)] h-[85vh] flex flex-col mx-4" data-shot="media-modal">
        <div className="px-5 pt-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("mediaTool.title")}</h3>
          <div className="flex gap-1 mt-3">
            {TABS.map((id) => (
              <button
                key={id}
                onClick={() => setCurrent(id)}
                disabled={busy && current !== id}
                data-shot={`media-tab-${id}`}
                className={`px-3 py-1.5 text-sm rounded-t border-b-2 -mb-px disabled:opacity-40 disabled:cursor-not-allowed ${
                  current === id ? "border-indigo-500 text-zinc-100" : "border-transparent text-zinc-400 hover:text-zinc-200"
                }`}
              >
                {t(`mediaTool.tab_${id}` as TranslationKeys)}
              </button>
            ))}
          </div>
        </div>
        {/* Una pestaña a la vez: al cambiar, la otra se desmonta y deja de buscar */}
        {current === "damaged" && (
          <UnusedMediaModal open embedded sections="damaged" systemPath={systemPath} onClose={onClose} onDone={onDone} onRescrape={onRescrape} />
        )}
        {current === "unused" && <UnusedMediaModal open embedded sections="unused" systemPath={systemPath} onClose={onClose} onDone={onDone} />}
        {current === "swapped" && <MediaReviewModal open embedded systemPath={systemPath} onClose={onClose} onDone={onDone} onOpenSwap={onOpenSwap} />}
        {current === "legacy" && <NormalizeMediaModal open embedded systemPath={systemPath} onClose={onClose} onDone={onDone} />}
      </div>
    </div>
  );
}
