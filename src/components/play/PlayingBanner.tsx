import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { stopGame } from "../../commands";
import { PlayIcon } from "../common/Icons";

/**
 * Barra de arriba mientras hay un juego abierto: «Jugando a X…» y un botón para cerrar el
 * emulador (si se cuelga o no responde). No tapa la app: se puede seguir usando por detrás.
 * Va por encima del modo sofá; no lleva «inset-0» (no es una ventana: no bloquea teclas).
 */
export function PlayingBanner() {
  const { t } = useTranslation();
  const running = useAppStore((s) => s.runningGame);
  if (!running) return null;

  return (
    <div data-playing className="fixed top-0 inset-x-0 z-40 flex justify-center pointer-events-none">
      <div className="mt-2 flex items-center gap-3 pl-4 pr-2 py-2 rounded-full bg-emerald-700/95 text-white text-sm shadow-xl shadow-black/40 pointer-events-auto">
        <PlayIcon size={14} className="animate-pulse" />
        <span className="max-w-[50vw] truncate">{t("play.running", { name: running.name })}</span>
        <button
          onClick={() => stopGame()}
          className="px-3 py-1 rounded-full bg-black/25 hover:bg-black/40 text-xs font-medium transition-colors"
        >
          {t("play.stop")}
        </button>
      </div>
    </div>
  );
}
