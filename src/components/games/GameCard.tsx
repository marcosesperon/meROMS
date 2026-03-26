import type { Game } from "../../types";
import { useAppStore } from "../../store/useAppStore";
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

  return (
    <div
      onClick={() => selectGame(game)}
      className={`rounded-lg overflow-hidden cursor-pointer transition-all border ${
        selectedGame?.path === game.path
          ? "border-indigo-500 ring-1 ring-indigo-500/50"
          : "border-zinc-700 hover:border-zinc-500"
      }`}
    >
      <div
        className="relative bg-zinc-800 flex items-center justify-center overflow-hidden"
        style={{ aspectRatio }}
      >
        {game.favorite === "true" && (
          <div className="absolute top-1.5 right-1.5 z-10 text-yellow-400 drop-shadow-lg">
            <StarIcon size={16} />
          </div>
        )}
        {imageSrc ? (
          <img
            src={imageSrc}
            alt={game.name || game.path}
            className="w-full h-full object-cover"
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
      <div className="p-3 bg-zinc-850">
        <p className="text-sm font-medium text-zinc-200 truncate">
          {game.name || game.path}
        </p>
        <div className="flex items-center justify-between mt-1">
          <span className="text-xs text-zinc-500 truncate">
            {game.genre || ""}
          </span>
          <span className="flex items-center gap-1">
            {getFlag(game.lang) && (
              <span className="text-xs" title={game.lang}>{getFlag(game.lang)}</span>
            )}
            {game.rating && (
              <span className="text-xs text-yellow-500">
                {formatRating(game.rating)}
              </span>
            )}
          </span>
        </div>
      </div>
    </div>
  );
}
