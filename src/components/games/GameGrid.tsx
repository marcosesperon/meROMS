import { useMemo, useRef } from "react";
import { useListKeyboard } from "../../hooks/useListKeyboard";
import type { Game } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { GameCard } from "./GameCard";
import { getSystemCardWidth, extractCustomRatios } from "../../utils/cartridgeSizes";
import { sortGames } from "../../utils/playStats";

interface GameGridProps {
  games: Game[];
}

export function GameGrid({ games }: GameGridProps) {
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const systemConfigs = useAppStore((s) => s.settings.system_configs);
  const cardWidth = getSystemCardWidth(selectedSystem?.name || "", extractCustomRatios(systemConfigs));
  // Orden elegido junto al cambio de vista (por defecto, por nombre o «nombre para ordenar»
  // como ES-DE; los que no tienen el dato del orden van al final)
  const gridSort = useAppStore((s) => s.settings.grid_sort);
  const sorted = useMemo(() => sortGames(games, gridSort), [games, gridSort]);

  const scrollRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  useListKeyboard({
    items: sorted,
    containerRef: scrollRef,
    // Las columnas dependen del ancho: se cuentan las tarjetas de la primera fila
    getColumns: () => {
      const cards = Array.from(gridRef.current?.children ?? []) as HTMLElement[];
      if (cards.length === 0) return 1;
      const top = cards[0].offsetTop;
      return cards.filter((c) => c.offsetTop === top).length;
    },
  });

  return (
    <div ref={scrollRef} className="overflow-auto flex-1 p-5">
      <div
        ref={gridRef}
        className="grid gap-x-3 gap-y-4"
        style={{
          gridTemplateColumns: `repeat(auto-fill, minmax(${cardWidth}px, 1fr))`,
        }}
      >
        {sorted.map((game) => (
          <GameCard key={game.path} game={game} />
        ))}
      </div>
    </div>
  );
}
