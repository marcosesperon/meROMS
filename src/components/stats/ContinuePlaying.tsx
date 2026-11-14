import { useState } from "react";
import { useI18nStore, useTranslation } from "../../i18n";
import type { ContinueGame } from "../../commands";
import { SystemLogo } from "../common/SystemLogo";
import { PlayIcon } from "../common/Icons";
import { resolveImageSrc } from "../../utils/images";
import { esDateToMs, formatAgo } from "../../utils/format";
import { goToGame, playGameAt } from "../../utils/goToGame";
import { systemGradient } from "../../utils/systemInfo";
import { useAppStore } from "../../store/useAppStore";

/**
 * «Seguir jugando» en la pantalla de inicio: una fila que se desplaza en horizontal con los
 * juegos marcados «Jugando» y los últimos jugados (los elige el backend, `continue_playing`).
 * Cada tarjeta enseña la captura del último estado guardado (o la carátula), cuándo se jugó y
 * cuándo se guardó; Jugar lo lanza (cambiando de sistema si hace falta) y un clic lleva a su ficha.
 * Sin juegos, no se pinta nada.
 */
export function ContinuePlaying({ games }: { games: ContinueGame[] }) {
  const { t } = useTranslation();
  if (games.length === 0) return null;
  return (
    <section data-continue-playing>
      <h2 className="mb-3 text-[11px] font-semibold uppercase tracking-[0.22em] text-zinc-500">{t("continue.title")}</h2>
      <div className="flex gap-3 overflow-x-auto pb-2 -mx-1 px-1 snap-x">
        {games.map((g, i) => (
          <ContinueCard key={`${g.system_path}:${g.game_path}`} game={g} index={i} />
        ))}
      </div>
    </section>
  );
}

function ContinueCard({ game, index }: { game: ContinueGame; index: number }) {
  const { t } = useTranslation();
  const locale = useI18nStore((s) => s.locale);
  const running = useAppStore((s) => !!s.runningGame);
  const thumb = (game.last_save?.thumbnail && resolveImageSrc(game.last_save.thumbnail, game.system_path)) || null;
  const cover = (game.cover && resolveImageSrc(game.cover, game.system_path)) || null;
  const played = esDateToMs(game.lastplayed);
  // Abrir su ficha: su sistema, el juego elegido y la ficha completa
  const open = async () => {
    await goToGame(game.system_path, game.game_path);
    useAppStore.getState().openFullView();
  };

  return (
    <div
      data-continue-game={`${game.system_path}|${game.game_path}`}
      className="home-rise group relative shrink-0 w-60 snap-start overflow-hidden rounded-xl border border-white/10 bg-[#1a1715] shadow-lg shadow-black/30 transition-[transform,border-color] duration-200 hover:-translate-y-1 hover:border-white/25"
      style={{ "--delay": `${Math.min(index, 12) * 40}ms` } as React.CSSProperties}
    >
      <button onClick={open} className="block w-full text-left" title={game.name}>
        {/* Imagen: la captura del estado guardado o, si no hay, la carátula sobre el color del sistema */}
        <span className="relative block aspect-[4/3] overflow-hidden" style={{ background: `${systemGradient(game.system_name)}, #1a1715` }}>
          <Picture thumb={thumb} cover={cover} />
          <span className="absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-black/70 to-transparent" />
          {game.playing && (
            <span className="absolute top-2 left-2 px-2 py-0.5 rounded-full bg-emerald-600/90 text-[10px] font-semibold uppercase tracking-wider text-white">
              {t("continue.playing")}
            </span>
          )}
          <span className="absolute left-3 bottom-2.5">
            <SystemLogo name={game.system_name} width={70} maxHeight={18} className="drop-shadow-[0_2px_4px_rgba(0,0,0,0.7)]" />
          </span>
        </span>
        <span className="block px-3 pt-2.5 pb-3 pr-14">
          <span className="block truncate text-sm font-semibold text-zinc-100 group-hover:text-white">{game.name}</span>
          <span className="mt-0.5 block truncate text-[11px] text-zinc-500">
            {played ? t("continue.played", { when: formatAgo(played, locale) }) : t("continue.notPlayed")}
          </span>
          {game.last_save && (
            <span className="block truncate text-[11px] text-zinc-500">{t("continue.saved", { when: formatAgo(game.last_save.modified * 1000, locale) })}</span>
          )}
        </span>
      </button>
      {/* Jugar, encima de la esquina de la imagen */}
      <button
        onClick={() => playGameAt(game.system_path, game.game_path)}
        disabled={running}
        title={t("play.button")}
        data-continue-play
        className="absolute right-3 flex items-center justify-center w-10 h-10 rounded-full bg-emerald-600 text-white shadow-lg shadow-black/50 hover:bg-emerald-500 disabled:opacity-40 transition-colors"
        // Medio botón sobre el borde de abajo de la imagen (tarjeta de 15rem, imagen 4:3)
        style={{ top: "calc(15rem * 0.75 - 1.25rem)" }}
      >
        <PlayIcon size={16} />
      </button>
    </div>
  );
}

/** La captura del estado (llenando el hueco) o la carátula entera; si fallan, solo el color */
function Picture({ thumb, cover }: { thumb: string | null; cover: string | null }) {
  const [thumbFailed, setThumbFailed] = useState(false);
  const [coverFailed, setCoverFailed] = useState(false);
  if (thumb && !thumbFailed) {
    return <img src={thumb} alt="" loading="lazy" onError={() => setThumbFailed(true)} className="absolute inset-0 w-full h-full object-cover [image-rendering:pixelated]" />;
  }
  if (cover && !coverFailed) {
    return <img src={cover} alt="" loading="lazy" onError={() => setCoverFailed(true)} className="absolute inset-0 w-full h-full object-contain p-3 drop-shadow-[0_6px_14px_rgba(0,0,0,0.6)]" />;
  }
  return null;
}
