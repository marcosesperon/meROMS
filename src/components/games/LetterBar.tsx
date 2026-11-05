import { useEffect, useMemo, useState, type RefObject } from "react";
import { useTranslation } from "../../i18n";
import type { Game } from "../../types";
import { LETTERS, letterOf } from "../../utils/letters";

interface LetterBarProps {
  /** Los juegos en el orden en que se ven */
  games: Game[];
  /** El texto por el que va ordenada la lista (de él sale la letra de cada juego) */
  nameOf: (game: Game) => string;
  /** El contenedor que se desplaza (cada juego lleva data-game-path) */
  containerRef: RefObject<HTMLDivElement | null>;
  /** Lista de la Z a la A: las letras van al revés */
  descending?: boolean;
}

/**
 * Barra «saltar a letra» a la derecha del listado (idea de RomM): las letras sin juegos salen
 * apagadas, un clic lleva al primer juego de esa letra y se resaltan las de los juegos a la vista.
 */
export function LetterBar({ games, nameOf, containerRef, descending = false }: LetterBarProps) {
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

  // Juegos a la vista ahora mismo: se apuntan y se borran según entran y salen del contenedor
  const [visiblePaths, setVisiblePaths] = useState<Set<string>>(new Set());
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const seen = new Set<string>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const path = (entry.target as HTMLElement).dataset.gamePath;
          if (!path) continue;
          if (entry.isIntersecting) seen.add(path);
          else seen.delete(path);
        }
        setVisiblePaths(new Set(seen));
      },
      { root: container }
    );
    container.querySelectorAll<HTMLElement>("[data-game-path]").forEach((el) => observer.observe(el));
    return () => observer.disconnect();
  }, [containerRef, games]);

  const visibleLetters = useMemo(() => {
    const set = new Set<string>();
    visiblePaths.forEach((path) => {
      const letter = letterByPath.get(path);
      if (letter) set.add(letter);
    });
    return set;
  }, [visiblePaths, letterByPath]);

  function jump(letter: string) {
    const path = firstByLetter.get(letter);
    if (!path) return;
    containerRef.current
      ?.querySelector<HTMLElement>(`[data-game-path="${CSS.escape(path)}"]`)
      ?.scrollIntoView({ block: "start" });
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
