import { useEffect, useMemo, useRef, useState } from "react";
import { boxStyleFor, cachedSpineColor, spineColor, SPINE_FALLBACK, textColorOn } from "../../utils/boxStyles";
import { getSystemInfo, systemGradient } from "../../utils/systemInfo";
import { ScreenFrame, screenLook, type ScreenFrameLook } from "../common/ScreenFrame";
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
  /** Vídeo del juego ya resuelto a URL: con `open`, la caja se abre y lo enseña dentro */
  videoSrc?: string | null;
  /** Abrir la caja (la tapa gira sobre el lomo) y poner el vídeo; sin vídeo no se abre */
  open?: boolean;
  /**
   * Sitio libre a cada lado de la tarjeta (en % de su ancho) para la caja abierta: abierta queda
   * centrada en la tarjeta y, si se saldría por un lado (bordes de la cuadrícula), se corre hacia
   * dentro. Sin él (carrusel), siempre centrada.
   */
  openRoom?: { left: number; right: number };
  /** Logo del juego (medio `marquee`) ya resuelto a URL: va en el cartel de la recreativa */
  marqueeSrc?: string | null;
  /**
   * Abierta: medio ancho de lo abierto, en anchos de tarjeta (0.84 con la forma de la carátula;
   * más o menos según la del vídeo). El carrusel lo usa para apartar las cajas de los lados.
   */
  onOpenHalfWidth?: (half: number) => void;
}

/**
 * Cuánto se corre la caja abierta (en % del ancho de la tarjeta) para quedar centrada en ella.
 * La caja mide `boxWidth` % y está centrada con un margen del 8 % a la izquierda, así que su
 * borde izquierdo queda en el 54 % - boxWidth / 2. Abierta, la tapa queda tumbada a la izquierda
 * de ese borde, así que lo abierto va de borde - boxWidth a borde + boxWidth: su centro es el
 * borde. Con `room`, se corre lo justo para no pasarse del sitio libre de cada lado.
 */
export function openOffset(boxWidth: number, room?: { left: number; right: number }): number {
  let offset = boxWidth / 2 - 4;
  if (!room) return offset;
  // Centrada en la tarjeta, lo abierto va de 50 - boxWidth a 50 + boxWidth
  const overLeft = -room.left - (50 - boxWidth);
  if (overLeft > 0) offset += overLeft;
  const overRight = 50 + boxWidth + (offset - (boxWidth / 2 - 4)) - (100 + room.right);
  if (overRight > 0) offset -= overRight;
  return offset;
}

/** Franja de luz del destello: blanca en el centro y desvanecida a los lados */
const GLINT = "linear-gradient(90deg, transparent, rgba(255,255,255,0.25) 30%, rgba(255,255,255,0.75) 50%, rgba(255,255,255,0.25) 70%, transparent)";

/** Volumen del vídeo de la caja abierta (bajito: sale solo al pararse en un juego) */
const BOX_VIDEO_VOLUME = 0.3;

/** Forma (ancho / alto) de cada vídeo ya visto: la caja se abre la siguiente vez ya con ella */
const videoAspects = new Map<string, number>();

/**
 * Una caja de juego en 3D hecha con CSS: el frente con la carátula, el lomo a la izquierda (del
 * color del borde de la carátula, con el título en vertical y, en las de plástico, la banda de
 * arriba con el logo) y el canto de arriba. La forma del frente es la de la tarjeta; el grosor y
 * el material, los de la caja de verdad del sistema (utils/boxStyles).
 */
export function GameBox3D({
  system,
  coverSrc,
  title,
  aspectRatio,
  front,
  rotate,
  bothSpines = false,
  shadow = true,
  glint,
  videoSrc,
  open = false,
  openRoom,
  marqueeSrc,
  onOpenHalfWidth,
}: GameBox3DProps) {
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

  // Caja abierta: solo con un vídeo que se pueda cargar (si falla, se vuelve a cerrar)
  const [videoFailed, setVideoFailed] = useState(false);
  useEffect(() => setVideoFailed(false), [videoSrc]);
  const isOpen = open && !!videoSrc && !videoFailed;
  // Forma del vídeo: abierta, la caja (tapa + interior) toma esa forma para que no queden franjas
  const [videoAspect, setVideoAspect] = useState<number | null>(() => (videoSrc ? videoAspects.get(videoSrc) ?? null : null));
  useEffect(() => setVideoAspect(videoSrc ? videoAspects.get(videoSrc) ?? null : null), [videoSrc]);
  // Abierta mide dos tapas de ancho por una de alto: cada mitad, la mitad del ancho del vídeo a
  // esa altura. Con la tapa abierta la carátula no se ve, así que cambiar el ancho no la deforma.
  const boxWidth = isOpen && videoAspect ? (84 * videoAspect) / (2 * ratio) : 84;
  useEffect(() => {
    if (isOpen) onOpenHalfWidth?.(boxWidth / 100);
  }, [isOpen, boxWidth, onOpenHalfWidth]);

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
    <div
      className="absolute inset-0 flex items-center justify-center"
      style={{
        perspective: "800px",
        // Abierta mide el doble (la tapa queda a la izquierda): se corre para quedar centrada
        transform: isOpen ? `translateX(${openOffset(boxWidth, openRoom)}%)` : undefined,
        transition: "transform 0.6s cubic-bezier(0.2, 0.7, 0.3, 1)",
      }}
    >
      {/* Sombra en el suelo, sin girar */}
      {shadow && <div className="absolute left-[14%] right-[6%] bottom-[4%] h-[5%] rounded-[50%] bg-black/70 blur-md" />}
      {/* Más pequeña que la tarjeta: girada y con el lomo tiene que seguir cabiendo */}
      <div
        className="box3d relative"
        data-front={front}
        style={{
          width: `${boxWidth}%`,
          height: "84%",
          marginLeft: "8%",
          transform: rotate !== undefined ? `rotateY(${rotate}deg)` : undefined,
          // A la vez que la tapa: abre ajustándose al vídeo y, al cerrar, vuelve a su ancho
          transition: `width 0.6s cubic-bezier(0.2, 0.7, 0.3, 1) ${isOpen ? "0s" : "0.2s"}`,
        }}
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

        {/* Interior de la caja: lo que queda a la vista a la derecha al abrir la tapa */}
        {isOpen && (
          <div
            className="box3d-face inset-0"
            style={{ borderRadius: radius, background: "#0b0b0d", transform: "translateZ(-2px)" }}
          />
        )}

        {/* Tapa: el frente por fuera y el plástico por dentro; al abrirse gira 180° sobre el lomo
            (borde izquierdo) hacia ti y queda tumbada a la izquierda */}
        <div
          className="absolute inset-0"
          style={{
            transformStyle: "preserve-3d",
            transformOrigin: "left center",
            transform: isOpen ? "rotateY(-180deg)" : undefined,
            // Al cerrar, primero se va el vídeo y luego vuelve la tapa
            transition: `transform 0.6s cubic-bezier(0.2, 0.7, 0.3, 1) ${isOpen ? "0s" : "0.2s"}`,
          }}
        >
          {/* Cara de dentro de la tapa (vuelta del revés: se ve cuando la tapa está abierta) */}
          <div className="box3d-face inset-0" style={{ borderRadius: radius, background: "#0b0b0d", transform: "rotateY(180deg)" }} />

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

        {/* Vídeo: ocupa la caja abierta entera (tapa + interior), un pelín por delante, dentro de
            una pantalla (recreativa en los sistemas arcade, tele de tubo en el resto) */}
        {isOpen && videoSrc && (
          <BoxScreen
            src={videoSrc}
            {...screenLook(system, marqueeSrc)}
            onFailed={() => setVideoFailed(true)}
            onAspect={(aspect) => {
              videoAspects.set(videoSrc, aspect);
              setVideoAspect(aspect);
            }}
          />
        )}
      </div>
    </div>
  );
}

interface BoxScreenProps extends ScreenFrameLook {
  src: string;
  onFailed: () => void;
  onAspect: (aspect: number) => void;
}

/**
 * Vídeo de la caja abierta, dentro de una pantalla: aparece cuando la tapa ya está casi abierta,
 * con sonido bajo (si el sistema no deja arrancar un vídeo con sonido sin un clic antes, arranca
 * sin sonido). El vídeo llena la caja abierta y el marco sobresale alrededor, así que el vídeo
 * conserva su forma. Las medidas del marco van en `cqh` (alto de la caja), para que sea igual de
 * grueso en una tarjeta pequeña que en el carrusel. Encima, el cristal de tubo (App.css).
 */
function BoxScreen({ src, arcade, marquee, marqueeIsLogo, molding, onFailed, onAspect }: BoxScreenProps) {
  const ref = useRef<HTMLVideoElement>(null);
  const [shown, setShown] = useState(false);

  useEffect(() => {
    const video = ref.current;
    if (!video) return;
    video.volume = BOX_VIDEO_VOLUME;
    video.play().catch(() => {
      video.muted = true;
      video.play().catch(() => {});
    });
    // Aparece con la tapa casi abierta (gira en 0,6 s)
    const timer = window.setTimeout(() => setShown(true), 450);
    return () => window.clearTimeout(timer);
  }, [src]);

  return (
    // El hueco va en un div: un <video> con posición absoluta se quedaba a su tamaño original
    // (arriba a la izquierda) en vez de llenar la caja abierta
    <div
      className="absolute top-0 h-full pointer-events-none"
      style={{
        left: "-100%",
        width: "200%",
        transform: "translateZ(2px)",
        opacity: shown ? 1 : 0,
        transition: "opacity 0.25s",
        containerType: "size",
      }}
    >
      <ScreenFrame arcade={arcade} marquee={marquee} marqueeIsLogo={marqueeIsLogo} molding={molding}>
        <video
          ref={ref}
          src={src}
          data-box-video
          loop
          playsInline
          onError={onFailed}
          // En cuanto se sabe su tamaño, la caja abierta toma su forma
          onLoadedMetadata={(e) => {
            const v = e.currentTarget;
            if (v.videoWidth > 0 && v.videoHeight > 0) onAspect(v.videoWidth / v.videoHeight);
          }}
          className="block w-full h-full object-contain"
        />
      </ScreenFrame>
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
