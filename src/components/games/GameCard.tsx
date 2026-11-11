import { useLayoutEffect, useRef, useState } from "react";
import type { Game } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { resolveImageSrc } from "../../utils/images";
import { formatRating } from "../../utils/format";
import { StarIcon } from "../common/Icons";
import { getFlag } from "../../utils/flags";
import { DatTag } from "../common/DatTag";
import { clickGame, doubleClickGame } from "../../utils/gameClick";
import { statusBadge, statusText } from "../../utils/gameStatus";
import { effectiveGridField } from "../../utils/systemView";
import { GameBox3D } from "./GameBox3D";
import { useOpenAfter } from "../../hooks/useOpenAfter";

interface GameCardProps {
  game: Game;
  /** Forma de la tarjeta («ancho/alto»), la misma para todo el sistema (la calcula GameGrid) */
  aspectRatio: string;
  /** Dibujar la tarjeta como una caja 3D (con la carátula 2D, o la genérica del sistema) */
  box3d?: boolean;
}

export function GameCard({ game, aspectRatio, box3d = false }: GameCardProps) {
  const selectedGame = useAppStore((s) => s.selectedGame);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  // ROM no longer on disk: dimmed card with a "No ROM" label
  const romMissing = useAppStore((s) => s.missingRomPaths.has(game.path));
  const multiSelect = useAppStore((s) => s.multiSelect);
  const checked = useAppStore((s) => s.selectedPaths.has(game.path));
  const { t } = useTranslation();
  // El medio de la cuadrícula de este sistema (el suyo en el perfil o el global)
  const gridMediaField = useAppStore((s) => effectiveGridField(s.settings, s.selectedSystem?.name));

  // Use configured media field, with fallback chain
  const mediaValue =
    (game as unknown as Record<string, string | undefined>)[gridMediaField]
    || game.image
    || game.thumbnail;

  const imageSrc = resolveImageSrc(
    mediaValue,
    selectedSystem?.path || ""
  );


  const selected = multiSelect ? checked : selectedGame?.path === game.path;
  // En la caja 3D va siempre la carátula 2D, sea cual sea el medio de la cuadrícula
  const coverSrc = box3d ? resolveImageSrc(game.image, selectedSystem?.path || "") : null;
  const status = useAppStore((s) => s.statuses.get(game.path));
  const badge = statusBadge(status);
  const year = game.releasedate?.slice(0, 4);
  const subtitle = [game.developer, year].filter(Boolean).join(" · ");

  // Caja 3D: con el ratón quieto encima 2 s, se abre y enseña el vídeo del juego
  const boxVideo = useAppStore((s) => s.settings.box_video !== false);
  const videoSrc = box3d && boxVideo && !multiSelect ? resolveImageSrc(game.video, selectedSystem?.path || "") : null;
  const [hovering, setHovering] = useState(false);
  const open = useOpenAfter(hovering && !!videoSrc);
  const { slotRef, shift } = useOpenShift(open);

  return (
    <div
      data-game-path={game.path}
      // Clic: marcar (selección múltiple) o enseñar el juego; doble clic: el editor (utils/gameClick)
      onClick={() => clickGame(game)}
      onDoubleClick={() => doubleClickGame(game)}
      onMouseEnter={() => setHovering(true)}
      onMouseLeave={() => setHovering(false)}
      // Abierta, la caja pasa por encima de las tarjetas de al lado
      className={`group relative ${open ? "z-20" : ""} rounded-lg p-2 cursor-pointer scroll-mt-5 transition-colors ${
        selected ? "bg-zinc-800 ring-2 ring-indigo-500" : "hover:bg-zinc-800/70"
      } ${romMissing ? "opacity-50" : ""}`}
    >
      <div
        ref={slotRef}
        className={`relative rounded-md flex items-center justify-center ${box3d ? "" : "bg-zinc-800 overflow-hidden shadow-lg shadow-black/40"}`}
        style={{ aspectRatio }}
      >
        {multiSelect && (
          <div
            className={`absolute bottom-1.5 right-1.5 z-10 w-5 h-5 rounded border-2 flex items-center justify-center text-[11px] text-white ${
              checked ? "bg-indigo-600 border-indigo-400" : "bg-black/50 border-white/70"
            }`}
          >
            {checked ? "✓" : ""}
          </div>
        )}
        {/* Clon / BIOS según el DAT del sistema */}
        <DatTag path={game.path} className="absolute bottom-1.5 left-1.5 z-10" />
        {romMissing && (
          <div className="absolute top-1.5 left-1.5 z-10 px-1.5 py-0.5 rounded bg-red-600/90 text-[10px] font-semibold text-white">
            {t("missingRoms.badge")}
          </div>
        )}
        {/* Arriba a la derecha: favorito y, debajo, el estado del juego */}
        {(game.favorite === "true" || badge) && (
          <div className="absolute top-1.5 right-1.5 z-10 flex flex-col items-center gap-1 drop-shadow-lg">
            {game.favorite === "true" && <StarIcon size={16} className="text-yellow-400" />}
            {badge && (
              <span className="p-0.5 rounded bg-black/60" title={statusText(status, t)}>
                <badge.icon size={13} className={badge.color} />
              </span>
            )}
          </div>
        )}
        {box3d ? (
          <GameBox3D
            system={selectedSystem?.name || ""}
            coverSrc={coverSrc || null}
            title={game.name || game.path}
            aspectRatio={aspectRatio}
            front={selected}
            videoSrc={videoSrc}
            open={open}
            openShift={shift}
          />
        ) : imageSrc ? (
          <img
            src={imageSrc}
            alt={game.name || game.path}
            loading="lazy"
            className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-[1.03]"
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = "none";
            }}
          />
        ) : (
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1} className="text-zinc-600">
            <rect x="2" y="6" width="20" height="12" rx="2" />
            <path d="M6 12h2M9 10v4" />
            <circle cx="16" cy="10" r="1" />
            <circle cx="18" cy="12" r="1" />
          </svg>
        )}
      </div>
      <div className="pt-2 px-0.5">
        <p className="text-[13px] font-medium text-zinc-100 truncate" title={game.name || game.path}>
          {game.name || game.path}
        </p>
        <div className="flex items-center justify-between gap-2 mt-0.5">
          {/* Algo más claro que el gris de siempre: va sobre los colores del sistema */}
          <span className="text-[11px] text-zinc-400 truncate">{subtitle || game.genre || ""}</span>
          <span className="flex items-center gap-1 shrink-0">
            {getFlag(game.lang) && (
              <span className="text-xs" title={game.lang}>{getFlag(game.lang)}</span>
            )}
            {game.rating && (
              <span className="text-[11px] text-yellow-500">
                {formatRating(game.rating)}
              </span>
            )}
          </span>
        </div>
      </div>
    </div>
  );
}

/**
 * Hacia dónde se abre la caja de una tarjeta: abierta mide el doble, así que normalmente queda
 * centrada en su sitio (0.5), pero en la primera o la última columna se corre hacia dentro para
 * no salirse de la cuadrícula. Se mide al abrirse, antes de pintar.
 */
function useOpenShift(open: boolean) {
  const slotRef = useRef<HTMLDivElement>(null);
  const [shift, setShift] = useState(0.5);
  useLayoutEffect(() => {
    const slot = slotRef.current;
    const area = slot?.closest(".overflow-auto");
    if (!open || !slot || !area) return;
    const card = slot.getBoundingClientRect();
    const bounds = area.getBoundingClientRect();
    // La caja ocupa el 84 % de la tarjeta y empieza en el 12 % (GameBox3D: centrada y corrida un 8 %)
    const boxW = card.width * 0.84;
    const boxLeft = card.left + card.width * 0.12;
    const roomLeft = boxLeft - bounds.left;
    const roomRight = bounds.right - (boxLeft + boxW);
    setShift(Math.min(Math.max(0.5, 1 - roomLeft / boxW), Math.max(0, roomRight / boxW)));
  }, [open]);
  return { slotRef, shift };
}
