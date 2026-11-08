import { useEffect, useMemo, useRef } from "react";
import { useListKeyboard } from "../../hooks/useListKeyboard";
import type { Game } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { GameCard } from "./GameCard";
import { cardAspectRatio, cardWidthFor } from "../../utils/cartridgeSizes";
import { effectiveGridField, getSystemView } from "../../utils/systemView";
import { sortGames } from "../../utils/playStats";
import { LetterBar } from "./LetterBar";

// Texto por el que se ordena por nombre (como `sortGames`): de él sale la letra de la barra
const sortText = (g: Game) => g.sortname || g.name || g.path;

interface GameGridProps {
  games: Game[];
}

export function GameGrid({ games }: GameGridProps) {
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const settings = useAppStore((s) => s.settings);
  // Forma de las tarjetas: la elegida para el sistema en el perfil, la que más se repite en el
  // medio que se enseña (medida por el store al cargar el sistema) o la tabla fija
  const view = getSystemView(settings, selectedSystem?.name);
  const gridField = effectiveGridField(settings, selectedSystem?.name);
  const mediaShape = useAppStore((s) => s.mediaShape);
  const refreshMediaShape = useAppStore((s) => s.refreshMediaShape);
  const measured = mediaShape && mediaShape.system === selectedSystem?.path && mediaShape.field === gridField ? mediaShape : null;
  const aspectRatio = cardAspectRatio(selectedSystem?.name || "", view.aspect_ratio, measured);
  const cardWidth = cardWidthFor(aspectRatio);
  // Al cambiar el medio de la cuadrícula (en Ajustes o en el sistema), se mide el nuevo
  useEffect(() => {
    if (selectedSystem && (mediaShape?.system !== selectedSystem.path || mediaShape?.field !== gridField)) {
      refreshMediaShape(selectedSystem.path);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedSystem?.path, gridField]);
  // Orden elegido junto al cambio de vista (por defecto, por nombre o «nombre para ordenar»
  // como ES-DE; los que no tienen el dato del orden van al final)
  const gridSort = useAppStore((s) => s.settings.grid_sort);
  const sorted = useMemo(() => sortGames(games, gridSort), [games, gridSort]);
  // Clave de cada tarjeta: su ruta. Hay gamelists con la misma ruta en varias entradas (p. ej.
  // una por región) y, con claves repetidas, React se lía al cambiar la lista y deja tarjetas
  // viejas a la vista; a la segunda y siguientes se les añade «#2», «#3»…
  const keys = useMemo(() => {
    const seen = new Map<string, number>();
    return sorted.map((g) => {
      const n = (seen.get(g.path) ?? 0) + 1;
      seen.set(g.path, n);
      return n === 1 ? g.path : `${g.path}#${n}`;
    });
  }, [sorted]);
  // El orden que se ve, para el anterior/siguiente de la ficha a pantalla completa
  const setVisibleOrder = useAppStore((s) => s.setVisibleOrder);
  useEffect(() => setVisibleOrder(sorted.map((g) => g.path)), [sorted, setVisibleOrder]);
  const byName = !gridSort || gridSort === "name";

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
    <div className="flex-1 flex overflow-hidden">
      <div ref={scrollRef} className="overflow-auto flex-1 p-5">
        <div
          ref={gridRef}
          className="grid gap-x-3 gap-y-4"
          style={{
            gridTemplateColumns: `repeat(auto-fill, minmax(${cardWidth}px, 1fr))`,
          }}
        >
          {sorted.map((game, i) => (
            <GameCard key={keys[i]} game={game} aspectRatio={aspectRatio} />
          ))}
        </div>
      </div>
      {/* Saltar a letra: solo con la lista por nombre (con otro orden las letras van salteadas) */}
      {byName && <LetterBar games={sorted} nameOf={sortText} containerRef={scrollRef} />}
    </div>
  );
}
