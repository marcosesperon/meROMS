import { useEffect, useState, type ReactNode } from "react";
import { isArcadeSystem } from "../../utils/arcade";
import { getSystemInfo } from "../../utils/systemInfo";
import { SPINE_FALLBACK } from "../../utils/boxStyles";
import { logoUrl } from "./SystemLogo";

/**
 * Lo que sobresale el marco alrededor de la pantalla, en % del alto de la pantalla (las
 * medidas van en `cqh`): la recreativa lleva el cartel arriba; la tele, las ruedas abajo.
 * Quien coloca la pantalla lo necesita para dejar sitio al marco.
 */
export const FRAME_EXTENT = {
  arcade: { top: 24, right: 4, bottom: 4, left: 4 },
  tv: { top: 5, right: 5, bottom: 12, left: 5 },
} as const;

export interface ScreenFrameLook {
  /** Marco de máquina recreativa (con cartel arriba); si no, tele de tubo */
  arcade: boolean;
  /** Imagen del cartel de la recreativa: el logo del juego o, si no tiene, el del sistema */
  marquee: string | null;
  /** El cartel lleva el logo del sistema (SVG blanco): se pinta más pequeño */
  marqueeIsLogo: boolean;
  /** Color de las molduras laterales de la recreativa (el del sistema) */
  molding: string;
}

/** Cómo es el marco de un sistema: recreativa en los arcade (con el marquee del juego), tele en el resto */
export function screenLook(system: string, marqueeSrc?: string | null): ScreenFrameLook {
  const logo = logoUrl(system);
  return {
    arcade: isArcadeSystem(system),
    marquee: marqueeSrc || logo || null,
    marqueeIsLogo: !marqueeSrc,
    molding: getSystemInfo(system)?.gradient?.[0] ?? SPINE_FALLBACK,
  };
}

interface ScreenFrameProps extends ScreenFrameLook {
  /** Lo que va en la pantalla (el vídeo) */
  children: ReactNode;
}

/**
 * Pantalla con su marco: mueble de recreativa o tele de tubo alrededor y, encima de lo que se
 * enseña, el cristal del tubo (`.crt-glass` en App.css: líneas, viñeta y reflejo).
 * Se pinta en un padre posicionado que mide lo que la pantalla y con `containerType: "size"`:
 * el marco va en `cqh` (alto de la pantalla), así es igual de grueso en pequeño que en grande.
 */
export function ScreenFrame({ arcade, marquee, marqueeIsLogo, molding, children }: ScreenFrameProps) {
  const [marqueeFailed, setMarqueeFailed] = useState(false);
  useEffect(() => setMarqueeFailed(false), [marquee]);
  const e = arcade ? FRAME_EXTENT.arcade : FRAME_EXTENT.tv;
  const inset = `-${e.top}cqh -${e.right}cqh -${e.bottom}cqh -${e.left}cqh`;

  return (
    <>
      {arcade ? (
        // Recreativa: mueble negro con molduras del color del sistema y el cartel iluminado arriba
        <div
          className="absolute flex flex-col pointer-events-none"
          style={{
            inset,
            background: "linear-gradient(180deg, #141416, #0a0a0b)",
            borderLeft: `1.2cqh solid ${molding}`,
            borderRight: `1.2cqh solid ${molding}`,
            borderRadius: "1.5cqh",
            boxShadow: "0 2cqh 5cqh rgba(0,0,0,0.6)",
          }}
        >
          <div
            className="flex items-center justify-center overflow-hidden"
            style={{
              height: "17cqh",
              margin: "1.5cqh 1.5cqh 0",
              borderRadius: "0.8cqh",
              // Luz de detrás del cartel
              background: "radial-gradient(ellipse at center, #3b3550 0%, #1b1824 70%, #111015 100%)",
              boxShadow: "inset 0 0 2cqh rgba(255,255,255,0.12)",
            }}
          >
            {marquee && !marqueeFailed && (
              <img
                src={marquee}
                alt=""
                className="object-contain"
                style={{ maxWidth: "90%", maxHeight: marqueeIsLogo ? "60%" : "85%", filter: "drop-shadow(0 0 1.2cqh rgba(255,255,255,0.35))" }}
                onError={() => setMarqueeFailed(true)}
              />
            )}
          </div>
        </div>
      ) : (
        // Tele de tubo: carcasa de plástico con dos ruedas y una luz abajo a la derecha
        <div
          className="absolute pointer-events-none"
          style={{
            inset,
            background: "linear-gradient(160deg, #4a4641 0%, #2c2a27 45%, #1a1917 100%)",
            borderRadius: "5cqh",
            boxShadow: "0 2cqh 5cqh rgba(0,0,0,0.6), inset 0 0.4cqh 0.6cqh rgba(255,255,255,0.12)",
          }}
        >
          <div className="absolute flex items-center" style={{ right: "6cqh", bottom: "2.5cqh", gap: "2.4cqh" }}>
            <span className="rounded-full" style={{ width: "1.4cqh", height: "1.4cqh", background: "#e0412f", boxShadow: "0 0 1cqh #e0412f" }} />
            {[0, 1].map((k) => (
              <span
                key={k}
                className="rounded-full"
                style={{ width: "4.5cqh", height: "4.5cqh", background: "radial-gradient(circle at 35% 35%, #6b665f, #24221f 70%)", boxShadow: "0 0.3cqh 0.6cqh rgba(0,0,0,0.6)" }}
              />
            ))}
          </div>
        </div>
      )}

      {/* Pantalla: lo que se enseña con el cristal encima */}
      <div className="absolute inset-0 bg-black overflow-hidden" style={{ borderRadius: arcade ? "0.6cqh" : "3.5cqh" }}>
        {children}
        <div className="crt-glass" />
      </div>
    </>
  );
}
