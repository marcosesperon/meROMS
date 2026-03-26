import type { Game } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { GameCard } from "./GameCard";
import { getSystemCardWidth, extractCustomRatios } from "../../utils/cartridgeSizes";

interface GameGridProps {
  games: Game[];
}

export function GameGrid({ games }: GameGridProps) {
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const systemConfigs = useAppStore((s) => s.settings.system_configs);
  const cardWidth = getSystemCardWidth(selectedSystem?.name || "", extractCustomRatios(systemConfigs));

  return (
    <div className="overflow-auto flex-1 p-4">
      <div
        className="grid gap-4"
        style={{
          gridTemplateColumns: `repeat(auto-fill, minmax(${cardWidth}px, 1fr))`,
        }}
      >
        {games.map((game) => (
          <GameCard key={game.path} game={game} />
        ))}
      </div>
    </div>
  );
}
