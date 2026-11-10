import { useMemo } from "react";
import { useTranslation } from "../../i18n";
import type { Game } from "../../types";
import { LETTERS, letterOf } from "../../utils/letters";

interface LetterRowProps {
  /** Los juegos en el orden en que se ven */
  games: Game[];
  /** El texto por el que va ordenada la lista (de él sale la letra de cada juego) */
  nameOf: (game: Game) => string;
  /** Posición del juego que está ahora en el centro */
  current: number;
  /** Ir al juego de esa posición */
  onJump: (index: number) => void;
}

/**
 * «Saltar a letra» en fila, para el carrusel (la de la cuadrícula es LetterBar, en columna):
 * las letras sin juegos salen apagadas, un clic lleva al primer juego de esa letra y se resalta
 * la del juego del centro.
 */
export function LetterRow({ games, nameOf, current, onJump }: LetterRowProps) {
  const { t } = useTranslation();

  // Primer juego de cada letra (en el orden de la lista)
  const firstByLetter = useMemo(() => {
    const first = new Map<string, number>();
    games.forEach((game, index) => {
      const letter = letterOf(nameOf(game) || "");
      if (!first.has(letter)) first.set(letter, index);
    });
    return first;
  }, [games, nameOf]);
  const currentLetter = games[current] ? letterOf(nameOf(games[current]) || "") : "";

  return (
    <nav aria-label={t("letters.label")} className="flex-1 min-w-0 flex justify-center gap-0.5 select-none">
      {LETTERS.map((letter) => {
        const index = firstByLetter.get(letter);
        const has = index !== undefined;
        return (
          <button
            key={letter}
            onClick={() => has && onJump(index)}
            disabled={!has}
            title={has ? t("letters.jump", { letter }) : undefined}
            // Se reparten el ancho y encogen si no caben (máximo, el tamaño de un botón normal)
            className={`flex-1 min-w-0 max-w-7 h-6 rounded text-[11px] font-semibold leading-none transition-colors ${
              !has
                ? "text-white/20 cursor-default"
                : letter === currentLetter
                  ? "text-indigo-200 bg-indigo-500/40"
                  : "text-zinc-300 hover:text-white hover:bg-white/10"
            }`}
          >
            {letter}
          </button>
        );
      })}
    </nav>
  );
}
