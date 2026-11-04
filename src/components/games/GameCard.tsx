import type { Game } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { resolveImageSrc } from "../../utils/images";
import { formatRating } from "../../utils/format";
import { getSystemAspectRatio, extractCustomRatios } from "../../utils/cartridgeSizes";
import { StarIcon } from "../common/Icons";
import { getFlag } from "../../utils/flags";

interface GameCardProps {
  game: Game;
}

export function GameCard({ game }: GameCardProps) {
  const selectGame = useAppStore((s) => s.selectGame);
  const selectedGame = useAppStore((s) => s.selectedGame);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const openEditor = useAppStore((s) => s.openEditor);
  // ROM no longer on disk: dimmed card with a "No ROM" label
  const romMissing = useAppStore((s) => s.missingRomPaths.has(game.path));
  const { t } = useTranslation();
  const gridMediaField = useAppStore((s) => s.settings.grid_media_field) || "image";

  // Use configured media field, with fallback chain
  const mediaValue =
    (game as unknown as Record<string, string | undefined>)[gridMediaField]
    || game.image
    || game.thumbnail;

  const imageSrc = resolveImageSrc(
    mediaValue,
    selectedSystem?.path || ""
  );

  const systemConfigs = useAppStore((s) => s.settings.system_configs);
  const aspectRatio = getSystemAspectRatio(selectedSystem?.name || "", extractCustomRatios(systemConfigs));

  const selected = selectedGame?.path === game.path;
  const year = game.releasedate?.slice(0, 4);
  const subtitle = [game.developer, year].filter(Boolean).join(" · ");

  return (
    <div
      data-game-path={game.path}
      onClick={() => selectGame(game)}
      // Doble clic: abrir directamente el editor de ese juego
      onDoubleClick={() => {
        selectGame(game);
        openEditor();
      }}
      className={`group rounded-lg p-2 cursor-pointer transition-colors ${
        selected ? "bg-zinc-800 ring-2 ring-indigo-500" : "hover:bg-zinc-800/70"
      } ${romMissing ? "opacity-50" : ""}`}
    >
      <div
        className="relative bg-zinc-800 rounded-md flex items-center justify-center overflow-hidden shadow-lg shadow-black/40"
        style={{ aspectRatio }}
      >
        {romMissing && (
          <div className="absolute top-1.5 left-1.5 z-10 px-1.5 py-0.5 rounded bg-red-600/90 text-[10px] font-semibold text-white">
            {t("missingRoms.badge")}
          </div>
        )}
        {game.favorite === "true" && (
          <div className="absolute top-1.5 right-1.5 z-10 text-yellow-400 drop-shadow-lg">
            <StarIcon size={16} />
          </div>
        )}
        {imageSrc ? (
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
          <span className="text-[11px] text-zinc-500 truncate">{subtitle || game.genre || ""}</span>
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
