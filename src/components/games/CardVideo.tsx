import { useEffect, useRef, useState } from "react";
import { ScreenFrame, screenLook } from "../common/ScreenFrame";
import { BOX_VIDEO_VOLUME, videoAspects } from "./GameBox3D";

interface CardVideoProps {
  /** El vídeo del juego (URL ya resuelta) */
  src: string;
  system: string;
  marqueeSrc?: string | null;
  /** Forma de la tarjeta («3/4»…) */
  aspectRatio: string;
  /** Sitio libre a cada lado de la tarjeta, en % de su ancho (para no salirse de la cuadrícula) */
  room?: { left: number; right: number };
  /** El vídeo no se puede cargar: la tarjeta vuelve a su carátula */
  onFailed: () => void;
}

/** Alto de la pantalla, en % del alto del hueco de la carátula (como la caja 3D abierta) */
const SCREEN_HEIGHT = 84;
/** Hueco que se deja al borde de la cuadrícula, en % del ancho de la tarjeta */
const EDGE = 4;

/**
 * Vídeo al pararse en un juego en la cuadrícula plana (sin cajas 3D): una pantalla flotante con su
 * marco (tele de tubo o recreativa con la marquesina) encima de la tarjeta. Toma la forma del vídeo
 * a la altura de la carátula, así que suele ser más ancha que la tarjeta: queda centrada en ella y,
 * en la primera o la última columna, se corre hacia dentro para no salirse.
 */
export function CardVideo({ src, system, marqueeSrc, aspectRatio, room, onFailed }: CardVideoProps) {
  const ref = useRef<HTMLVideoElement>(null);
  const [shown, setShown] = useState(false);
  // Hasta saber su forma, 4:3 (la de casi todos); la siguiente vez ya sale con la buena
  const [videoAspect, setVideoAspect] = useState(() => videoAspects.get(src) ?? 4 / 3);

  useEffect(() => {
    const video = ref.current;
    if (!video) return;
    video.volume = BOX_VIDEO_VOLUME;
    // Si el sistema no deja sonar sin un clic, en silencio
    video.play().catch(() => {
      video.muted = true;
      video.play().catch(() => {});
    });
    setShown(false);
    const timer = window.setTimeout(() => setShown(true), 50);
    return () => window.clearTimeout(timer);
  }, [src]);

  const [w, h] = aspectRatio.split("/").map(Number);
  const cardRatio = w > 0 && h > 0 ? w / h : 3 / 4;
  // Ancho de la pantalla en % del ancho de la tarjeta, a SCREEN_HEIGHT % de su alto
  const width = (SCREEN_HEIGHT * videoAspect) / cardRatio;
  // Centrada en la tarjeta; corrida lo justo si se sale por un lado
  let left = 50 - width / 2;
  if (room) {
    left = Math.max(left, -room.left + EDGE);
    left = Math.min(left, 100 + room.right - EDGE - width);
  }

  return (
    <div
      // Por encima de las marcas de la tarjeta (favorito, estado, logros…)
      className="absolute z-20 pointer-events-none"
      style={{
        top: `${(100 - SCREEN_HEIGHT) / 2}%`,
        height: `${SCREEN_HEIGHT}%`,
        left: `${left}%`,
        width: `${width}%`,
        opacity: shown ? 1 : 0,
        transform: shown ? "scale(1)" : "scale(0.9)",
        transition: "opacity 0.3s, transform 0.4s cubic-bezier(0.2, 0.7, 0.3, 1)",
        containerType: "size",
      }}
    >
      <ScreenFrame {...screenLook(system, marqueeSrc)}>
        <video
          ref={ref}
          src={src}
          data-box-video
          loop
          playsInline
          onError={onFailed}
          onLoadedMetadata={(e) => {
            const v = e.currentTarget;
            if (v.videoWidth > 0 && v.videoHeight > 0) {
              const aspect = v.videoWidth / v.videoHeight;
              videoAspects.set(src, aspect);
              setVideoAspect(aspect);
            }
          }}
          className="block w-full h-full object-contain bg-black"
        />
      </ScreenFrame>
    </div>
  );
}
