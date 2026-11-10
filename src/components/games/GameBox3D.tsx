import { useEffect, useMemo, useState } from "react";
import { boxStyleFor, cachedSpineColor, spineColor, SPINE_FALLBACK, textColorOn } from "../../utils/boxStyles";
import { getSystemInfo, systemGradient } from "../../utils/systemInfo";
import { logoUrl } from "../common/SystemLogo";

interface GameBox3DProps {
  /** Carpeta del sistema: decide el tipo de caja, los colores y el logo */
  system: string;
  /** Carátula 2D ya resuelta a URL; sin ella (o si no carga), la caja genérica del sistema */
  coverSrc: string | null;
  title: string;
  /** Forma del frente («ancho/alto»), la de las tarjetas del sistema */
  aspectRatio: string;
  /** De frente aunque no tenga el ratón encima (juego seleccionado) */
  front: boolean;
  /** Giro fijo en grados (el carrusel coloca cada caja); sin él, el de la cuadrícula (App.css) */
  rotate?: number;
  /** Lomo también a la derecha (el carrusel enseña las cajas de la derecha por ese lado) */
  bothSpines?: boolean;
  /** Sombra en el suelo (el carrusel pone su propio reflejo) */
  shadow?: boolean;
  /** Destello sobre el frente: cada vez que cambia este número, una franja de luz lo cruza */
  glint?: number;
}

/** Franja de luz del destello: blanca en el centro y desvanecida a los lados */
const GLINT = "linear-gradient(90deg, transparent, rgba(255,255,255,0.25) 30%, rgba(255,255,255,0.75) 50%, rgba(255,255,255,0.25) 70%, transparent)";

/**
 * Una caja de juego en 3D hecha con CSS: el frente con la carátula, el lomo a la izquierda (del
 * color del borde de la carátula, con el título en vertical y, en las de plástico, la banda de
 * arriba con el logo) y el canto de arriba. La forma del frente es la de la tarjeta; el grosor y
 * el material, los de la caja de verdad del sistema (utils/boxStyles).
 */
export function GameBox3D({ system, coverSrc, title, aspectRatio, front, rotate, bothSpines = false, shadow = true, glint }: GameBox3DProps) {
  const style = useMemo(() => boxStyleFor(system), [system]);
  const [failed, setFailed] = useState(false);
  const cover = coverSrc && !failed ? coverSrc : null;
  useEffect(() => setFailed(false), [coverSrc]);

  // Color del lomo: el de la carátula (se lee una vez y se guarda), si no el del plástico
  const [edge, setEdge] = useState<string | null | undefined>(() => (cover ? cachedSpineColor(cover) : null));
  useEffect(() => {
    if (!cover) return setEdge(null);
    const known = cachedSpineColor(cover);
    if (known !== undefined) return setEdge(known);
    let alive = true;
    spineColor(cover).then((c) => alive && setEdge(c));
    return () => {
      alive = false;
    };
  }, [cover]);

  const [w, h] = aspectRatio.split("/").map(Number);
  const ratio = w > 0 && h > 0 ? w / h : 3 / 4;
  // El grosor es una parte del lado corto (las cajas apaisadas, como las europeas de SNES, no
  // son más gordas por ser anchas); va en % del ancho y el canto de arriba en % del alto
  const depthW = (style.depth * 100) / Math.max(1, ratio);
  const depthH = depthW * ratio;
  // Lomo: el color de la carátula; la caja genérica, el del sistema; si no, el del plástico
  const spine = edge ?? (cover ? null : getSystemInfo(system)?.gradient?.[0]) ?? style.plastic ?? SPINE_FALLBACK;
  const logo = logoUrl(system);
  const radius = `${style.radius}% / ${style.radius * ratio}%`;
  const logoFilter = style.darkLogo ? "brightness(0) opacity(0.7)" : undefined;

  const spineFace = (side: "left" | "right") => (
    <div
      key={side}
      className="box3d-face top-0 h-full overflow-hidden"
      style={{
        ...(side === "left"
          ? { left: `-${depthW}%`, transformOrigin: "right center", transform: "rotateY(-90deg)" }
          : { left: "100%", transformOrigin: "left center", transform: "rotateY(90deg)" }),
        width: `${depthW}%`,
        background: `linear-gradient(90deg, rgba(0,0,0,0.35), rgba(0,0,0,0.1) 40%, rgba(0,0,0,0.3)), ${spine}`,
        containerType: "size",
      }}
    >
      {style.band && (
        <div className="absolute inset-x-0 top-0 h-[17%]" style={{ background: style.band, containerType: "size" }}>
          {logo && (
            <img
              src={logo}
              alt=""
              className="absolute left-1/2 top-1/2 object-contain"
              style={{ width: "85cqh", height: "70cqw", transform: "translate(-50%, -50%) rotate(90deg)", filter: logoFilter }}
            />
          )}
        </div>
      )}
      <span
        className="absolute inset-x-0 bottom-[3%] flex items-center justify-center font-semibold overflow-hidden"
        style={{
          top: style.band ? "19%" : "3%",
          writingMode: "vertical-rl",
          fontSize: "min(52cqw, 6.5cqh)",
          color: textColorOn(spine),
          whiteSpace: "nowrap",
          textOverflow: "ellipsis",
        }}
      >
        {title}
      </span>
    </div>
  );

  return (
    <div className="absolute inset-0 flex items-center justify-center" style={{ perspective: "800px" }}>
      {/* Sombra en el suelo, sin girar */}
      {shadow && <div className="absolute left-[14%] right-[6%] bottom-[4%] h-[5%] rounded-[50%] bg-black/70 blur-md" />}
      {/* Más pequeña que la tarjeta: girada y con el lomo tiene que seguir cabiendo */}
      <div
        className="box3d relative"
        data-front={front}
        style={{ width: "84%", height: "84%", marginLeft: "8%", transform: rotate !== undefined ? `rotateY(${rotate}deg)` : undefined }}
      >

        {/* Lomo: girado 90° sobre el borde izquierdo del frente, hacia atrás (y, si se pide, otro
            igual en el derecho) */}
        {spineFace("left")}
        {bothSpines && spineFace("right")}

        {/* Canto de arriba: girado 90° sobre el borde de arriba, hacia atrás */}
        <div
          className="box3d-face left-0 w-full"
          style={{
            top: `-${depthH}%`,
            height: `${depthH}%`,
            transformOrigin: "center bottom",
            transform: "rotateX(90deg)",
            background: `linear-gradient(rgba(255,255,255,0.12), rgba(0,0,0,0.2)), ${style.plastic ?? spine}`,
          }}
        />

        {/* Frente */}
        <div
          className="box3d-face inset-0 overflow-hidden shadow-xl shadow-black/50"
          style={{
            borderRadius: radius,
            background: style.plastic ?? "#1c1c1f",
            // Canto de plástico alrededor de la carátula; el cartón va a sangre
            padding: style.plastic && style.family !== "jewel" ? "2.5%" : 0,
          }}
        >
          {cover ? (
            <img
              src={cover}
              alt={title}
              loading="lazy"
              className="w-full h-full object-cover"
              style={{ borderRadius: style.plastic ? `${style.radius * 0.6}%` : undefined }}
              onError={() => setFailed(true)}
            />
          ) : (
            <GenericFront system={system} title={title} logo={logo} band={style.band} logoFilter={logoFilter} />
          )}
          {/* Bisagra de la caja de CD, a la izquierda del frente */}
          {style.family === "jewel" && (
            <div className="absolute inset-y-0 left-0 w-[7%] bg-gradient-to-r from-black/55 via-white/15 to-black/30 border-r border-white/20" />
          )}
          {/* Brillo del plástico */}
          {style.gloss && (
            <div
              className="absolute inset-0 pointer-events-none"
              style={{ background: "linear-gradient(115deg, rgba(255,255,255,0.22) 0%, rgba(255,255,255,0.05) 35%, transparent 55%)" }}
            />
          )}
          {/* Destello (carrusel): con la key cambiada, la animación vuelve a empezar */}
          {glint !== undefined && glint > 0 && <div key={glint} className="box3d-glint" style={{ background: GLINT }} />}
          {/* El cartón: un pelín de luz en el borde, como el doblez de la caja */}
          {!style.plastic && <div className="absolute inset-0 pointer-events-none border border-white/10" style={{ borderRadius: radius }} />}
        </div>
      </div>
    </div>
  );
}

/** Frente de la caja genérica: los colores y el logo del sistema con el título del juego */
function GenericFront({ system, title, logo, band, logoFilter }: { system: string; title: string; logo?: string; band: string | null; logoFilter?: string }) {
  return (
    <div className="w-full h-full flex flex-col" style={{ background: systemGradient(system), backgroundColor: "#1c1c1f" }}>
      <div className="h-[16%] flex items-center justify-center px-[10%]" style={{ background: band ?? "rgba(0,0,0,0.35)" }}>
        {logo && <img src={logo} alt="" className="max-h-[70%] max-w-full object-contain" style={{ filter: band ? logoFilter : undefined }} />}
      </div>
      <div className="flex-1 flex items-center justify-center p-[10%]" style={{ containerType: "inline-size" }}>
        <p className="text-center font-bold text-white leading-tight drop-shadow-lg line-clamp-5 break-words" style={{ fontSize: "11cqw" }}>
          {title}
        </p>
      </div>
    </div>
  );
}
