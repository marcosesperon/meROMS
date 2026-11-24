import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "../../i18n";
import { FRAME_EXTENT, ScreenFrame, screenLook } from "./ScreenFrame";
import { CloseIcon, ExpandIcon, SpeakerIcon, SpeakerOffIcon } from "./Icons";

interface FramedVideoProps {
  /** Vídeo ya resuelto a URL */
  src: string;
  /** Carpeta del sistema: recreativa (arcade) o tele, y el color de las molduras */
  system: string;
  /** Logo del juego (medio `marquee`) ya resuelto a URL, para el cartel de la recreativa */
  marqueeSrc?: string | null;
  onFailed?: () => void;
  /** Segundo por el que empieza (la capa de ampliar sigue donde iba el pequeño) */
  startAt?: number;
  /** Sin botón de ampliar (la propia capa ampliada) */
  noExpand?: boolean;
}

/**
 * Vídeo del juego dentro de una tele de tubo (o de una recreativa en los sistemas arcade) con el
 * cristal del tubo encima, como en la caja abierta. Ocupa el hueco que le dé el padre: la pantalla
 * toma la forma del vídeo y se hace lo más grande que cabe dejando sitio al marco, centrada.
 * Sin los controles del sistema (irían bajo el cristal): un clic en la imagen pausa o sigue, y
 * encima de la pantalla, al pasar el ratón, el sonido abajo a la izquierda y ampliar (una capa
 * grande dentro de la app) arriba a la derecha. Así todo el hueco es para la tele.
 */
export function FramedVideo({ src, system, marqueeSrc, onFailed, startAt, noExpand = false }: FramedVideoProps) {
  const { t } = useTranslation();
  const look = screenLook(system, marqueeSrc);
  const extent = look.arcade ? FRAME_EXTENT.arcade : FRAME_EXTENT.tv;

  const videoRef = useRef<HTMLVideoElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const [stage, setStage] = useState({ w: 0, h: 0 });
  // Forma del vídeo; hasta saberla, la de una tele (4:3)
  const [aspect, setAspect] = useState(4 / 3);
  const [muted, setMuted] = useState(true);
  const [expandedAt, setExpandedAt] = useState<number | null>(null);

  // Lo que mide el hueco, para calcular la pantalla más grande que cabe con su marco
  useLayoutEffect(() => {
    const el = stageRef.current;
    if (!el) return;
    const measure = () => setStage({ w: el.clientWidth, h: el.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Otro vídeo: vuelve a empezar como siempre (solo, en bucle y en silencio)
  useEffect(() => {
    setAspect(4 / 3);
  }, [src]);

  // Alto de la pantalla: el marco sobresale en % de ese alto por cada lado. Se deja un pelín de
  // margen para la sombra del marco.
  const fit = 0.96;
  const screenH = Math.max(
    0,
    Math.min(
      (stage.h * fit) / (1 + (extent.top + extent.bottom) / 100),
      (stage.w * fit) / (aspect + (extent.left + extent.right) / 100)
    )
  );
  const screenW = screenH * aspect;
  // El marco es más alto por un lado (cartel arriba, ruedas abajo): se corre la pantalla para
  // que el conjunto quede centrado
  const shiftY = ((extent.top - extent.bottom) / 200) * screenH;
  const shiftX = ((extent.left - extent.right) / 200) * screenH;

  const togglePlay = () => {
    const video = videoRef.current;
    if (!video) return;
    if (video.paused) video.play().catch(() => {});
    else video.pause();
  };
  const toggleMute = () => {
    const video = videoRef.current;
    if (!video) return;
    video.muted = !video.muted;
    setMuted(video.muted);
  };
  const expand = () => {
    const video = videoRef.current;
    setExpandedAt(video?.currentTime ?? 0);
    video?.pause();
  };
  // Al cerrar la capa, el pequeño sigue por donde iba el grande
  const closeExpanded = (at: number) => {
    setExpandedAt(null);
    const video = videoRef.current;
    if (!video) return;
    video.currentTime = at;
    video.play().catch(() => {});
  };

  // Botones sobre la imagen: se ven al pasar el ratón por la pantalla
  const button =
    "absolute z-10 p-1.5 rounded-md bg-black/55 text-zinc-200 hover:text-white hover:bg-black/80 opacity-0 group-hover/screen:opacity-100 focus-visible:opacity-100 transition-opacity";

  return (
    <div className="w-full h-full min-h-0 flex flex-col" data-framed-video>
      <div ref={stageRef} className="flex-1 min-h-0 relative flex items-center justify-center">
        {screenH > 0 && (
          <div
            className="group/screen relative shrink-0"
            style={{ width: screenW, height: screenH, containerType: "size", transform: `translate(${shiftX}px, ${shiftY}px)` }}
          >
            <ScreenFrame {...look}>
              <video
                ref={videoRef}
                key={src}
                src={src}
                autoPlay
                muted={muted}
                loop
                playsInline
                onError={onFailed}
                onClick={togglePlay}
                onLoadedMetadata={(e) => {
                  const v = e.currentTarget;
                  if (v.videoWidth > 0 && v.videoHeight > 0) setAspect(v.videoWidth / v.videoHeight);
                  if (startAt) v.currentTime = startAt;
                }}
                className="block w-full h-full object-contain cursor-pointer"
              />
            </ScreenFrame>
            {/* Encima del cristal: sonido abajo a la izquierda, ampliar arriba a la derecha */}
            <button onClick={toggleMute} className={button} style={{ left: "4cqh", bottom: "4cqh" }} title={t(muted ? "video.unmute" : "video.mute")}>
              {muted ? <SpeakerOffIcon size={14} /> : <SpeakerIcon size={14} />}
            </button>
            {!noExpand && (
              <button onClick={expand} className={button} style={{ right: "4cqh", top: "4cqh" }} title={t("video.expand")}>
                <ExpandIcon size={14} />
              </button>
            )}
          </div>
        )}
      </div>

      {expandedAt !== null && (
        <ExpandedVideo src={src} system={system} marqueeSrc={marqueeSrc} startAt={expandedAt} onClose={closeExpanded} />
      )}
    </div>
  );
}

interface ExpandedVideoProps {
  src: string;
  system: string;
  marqueeSrc?: string | null;
  startAt: number;
  /** Se cierra diciendo por qué segundo iba */
  onClose: (at: number) => void;
}

/**
 * Capa de ampliar: la tele (o la recreativa) en grande sobre toda la app. Esc, un clic fuera o
 * la X la cierran. Va al final del body (portal): dentro del editor, un padre con transform
 * dejaría la capa encerrada en él. Es un `fixed inset-0`: mientras está abierta, el editor, la
 * ficha y el teclado de la lista no hacen caso a las teclas.
 */
function ExpandedVideo({ src, system, marqueeSrc, startAt, onClose }: ExpandedVideoProps) {
  const { t } = useTranslation();
  const boxRef = useRef<HTMLDivElement>(null);
  const close = () => onClose(boxRef.current?.querySelector("video")?.currentTime ?? startAt);
  const closeRef = useRef(close);
  closeRef.current = close;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      closeRef.current();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  return createPortal(
    <div className="fixed inset-0 z-[70] bg-black/90 flex items-center justify-center p-10" onClick={close}>
      <div ref={boxRef} className="w-[90vw] h-[85vh]" onClick={(e) => e.stopPropagation()}>
        <FramedVideo src={src} system={system} marqueeSrc={marqueeSrc} startAt={startAt} noExpand />
      </div>
      <button
        onClick={close}
        title={t("video.close")}
        className="absolute top-4 right-4 text-white bg-black/50 hover:bg-black/80 w-10 h-10 rounded-full flex items-center justify-center transition-colors"
      >
        <CloseIcon size={20} />
      </button>
    </div>,
    document.body
  );
}
