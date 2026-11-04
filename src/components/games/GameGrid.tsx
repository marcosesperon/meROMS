import { useMemo, useRef } from "react";
import { useListKeyboard } from "../../hooks/useListKeyboard";
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
  // Ordenada por nombre (o por «nombre para ordenar» si lo tiene, como hace ES-DE).
  // Sin distinguir mayúsculas ni tildes, y con los números en su orden («2» antes que «10»).
  const sorted = useMemo(
    () =>
      [...games].sort((a, b) =>
        (a.sortname || a.name || a.path).localeCompare(b.sortname || b.name || b.path, undefined, {
          numeric: true,
          sensitivity: "base",
        })
      ),
    [games]
  );

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
