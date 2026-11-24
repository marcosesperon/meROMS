import { useTranslation } from "../../i18n";
import type { GalleryItem } from "../../hooks/useGameDetails";
import { useAppStore } from "../../store/useAppStore";
import { FramedVideo } from "../common/FramedVideo";

interface GameMediaViewerProps {
  media: GalleryItem[];
  shown: GalleryItem;
  onPick: (index: number) => void;
  onFailed: (url: string) => void;
  /** Miniaturas más grandes (ficha a pantalla completa) */
  large?: boolean;
  /** Ocupar el alto del hueco: el visor crece hasta él y deja sitio a las miniaturas debajo */
  fit?: boolean;
}

/** Visor de un medio (vídeo o imagen) con la tira de miniaturas para elegir otro */
export function GameMediaViewer({ media, shown, onPick, onFailed, large = false, fit = false }: GameMediaViewerProps) {
  const { t } = useTranslation();
  const system = useAppStore((s) => s.selectedSystem?.name) ?? "";
  const isVideo = shown.field === "video";
  // El cartel de la recreativa: el marquee del juego, si está en su galería
  const marquee = media.find((m) => m.field === "marquee")?.url ?? null;
  return (
    <div className={fit ? "h-full flex flex-col min-h-0" : ""}>
      {/* Con «fit», todo el alto que quede libre (la imagen o el vídeo, enteros dentro). El vídeo
          va en su tele (o recreativa), sin fondo negro detrás */}
      <div
        className={`w-full rounded-lg flex items-center justify-center ${isVideo ? "" : "bg-black overflow-hidden"} ${
          fit ? "flex-1 min-h-0" : "aspect-video"
        }`}
      >
        {isVideo ? (
          <FramedVideo src={shown.url} system={system} marqueeSrc={marquee} onFailed={() => onFailed(shown.url)} />
        ) : (
          <img src={shown.url} alt={t(shown.label)} onError={() => onFailed(shown.url)} className="w-full h-full object-contain" />
        )}
      </div>
      {media.length > 1 && (
        <div className="flex gap-2 mt-2 overflow-x-auto pb-1 shrink-0">
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
