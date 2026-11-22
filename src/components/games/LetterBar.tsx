import { useMemo } from "react";
import { useTranslation } from "../../i18n";
import type { Game } from "../../types";
import { LETTERS, letterOf } from "../../utils/letters";

interface LetterBarProps {
  /** Los juegos en el orden en que se ven */
  games: Game[];
  /** El texto por el que va ordenada la lista (de él sale la letra de cada juego) */
  nameOf: (game: Game) => string;
  /** Juegos a la vista ahora mismo: de `visibleFrom` a `visibleTo` (sin incluir) de `games` */
  visibleFrom: number;
  visibleTo: number;
  /** Llevar la lista hasta un juego (la lista solo pinta lo que se ve: lo sabe ella) */
  onJump: (path: string) => void;
  /** Lista de la Z a la A: las letras van al revés */
  descending?: boolean;
}

/**
 * Barra «saltar a letra» a la derecha del listado (idea de RomM): las letras sin juegos salen
 * apagadas, un clic lleva al primer juego de esa letra y se resaltan las de los juegos a la vista.
 */
export function LetterBar({ games, nameOf, visibleFrom, visibleTo, onJump, descending = false }: LetterBarProps) {
  const { t } = useTranslation();
  const letters = descending ? [...LETTERS].reverse() : LETTERS;

  // Letra de cada juego y el primero de cada letra (en el orden de la lista)
  const { letterByPath, firstByLetter } = useMemo(() => {
    const letterByPath = new Map<string, string>();
    const firstByLetter = new Map<string, string>();
    for (const game of games) {
      const letter = letterOf(nameOf(game) || "");
      letterByPath.set(game.path, letter);
      if (!firstByLetter.has(letter)) firstByLetter.set(letter, game.path);
    }
    return { letterByPath, firstByLetter };
  }, [games, nameOf]);

  // Letras de los juegos a la vista
  const visibleLetters = useMemo(() => {
    const set = new Set<string>();
    for (const game of games.slice(visibleFrom, visibleTo)) {
      const letter = letterByPath.get(game.path);
      if (letter) set.add(letter);
    }
    return set;
  }, [games, visibleFrom, visibleTo, letterByPath]);

  function jump(letter: string) {
    const path = firstByLetter.get(letter);
    if (path) onJump(path);
  }

  return (
    <nav
      aria-label={t("letters.label")}
      className="w-6 shrink-0 flex flex-col items-center gap-px py-2 mr-1 overflow-y-auto select-none"
      // «safe»: si no caben, centradas empujarían las primeras fuera de alcance por arriba
      style={{ justifyContent: "safe center" }}
    >
      {letters.map((letter) => {
        const has = firstByLetter.has(letter);
        return (
          <button
            key={letter}
            onClick={() => jump(letter)}
            disabled={!has}
            title={has ? t("letters.jump", { letter }) : undefined}
            className={`w-5 h-[18px] shrink-0 rounded text-[10px] font-semibold leading-none transition-colors ${
              !has
                ? "text-zinc-700 cursor-default"
                : visibleLetters.has(letter)
                  ? "text-indigo-300 bg-indigo-500/20"
                  : "text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800"
            }`}
          >
            {letter}
          </button>
        );
      })}
    </nav>
  );
}
