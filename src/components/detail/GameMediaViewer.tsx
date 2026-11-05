import { useTranslation } from "../../i18n";
import type { GalleryItem } from "../../hooks/useGameDetails";

interface GameMediaViewerProps {
  media: GalleryItem[];
  shown: GalleryItem;
  onPick: (index: number) => void;
  onFailed: (url: string) => void;
  /** Miniaturas más grandes (ficha a pantalla completa) */
  large?: boolean;
  /** Que quepa en la ventana: el visor no pasa de su alto y se centra en vertical */
  fit?: boolean;
}

/** Visor de un medio (vídeo o imagen) con la tira de miniaturas para elegir otro */
export function GameMediaViewer({ media, shown, onPick, onFailed, large = false, fit = false }: GameMediaViewerProps) {
  const { t } = useTranslation();
  return (
    <div className={fit ? "h-full flex flex-col justify-center" : ""}>
      {/* Con «fit», 16:9 pero sin pasar del alto de la ventana menos la barra de arriba y las
          miniaturas (lo que sobra a los lados queda en negro) */}
      <div className={`aspect-video w-full bg-black rounded-lg overflow-hidden flex items-center justify-center ${fit ? "max-h-[calc(100vh-13rem)]" : ""}`}>
        {shown.field === "video" ? (
          <video key={shown.url} src={shown.url} onError={() => onFailed(shown.url)} controls autoPlay muted loop className="w-full h-full object-contain" />
        ) : (
          <img src={shown.url} alt={t(shown.label)} onError={() => onFailed(shown.url)} className="w-full h-full object-contain" />
        )}
      </div>
      {media.length > 1 && (
        <div className="flex gap-2 mt-2 overflow-x-auto pb-1">
          {media.map((m, i) => (
            <button
              key={m.field}
              onClick={() => onPick(i)}
              title={t(m.label)}
              // Sin borde: la elegida se ve entera y el resto algo apagadas
              className={`${large ? "w-24 h-16" : "w-16 h-11"} shrink-0 rounded-md overflow-hidden bg-zinc-800 transition-opacity ${
                m === shown ? "opacity-100" : "opacity-50 hover:opacity-80"
              }`}
            >
              {m.field === "video" ? (
                <span className="w-full h-full flex items-center justify-center text-zinc-300 text-lg">▶</span>
              ) : (
                <img src={m.url} alt="" onError={() => onFailed(m.url)} className="w-full h-full object-cover" />
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
