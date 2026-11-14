import { useEffect, useRef, useState } from "react";
import type { ContinueGame } from "../../commands";
import { useI18nStore, useTranslation } from "../../i18n";
import { GameBox3D } from "../games/GameBox3D";
import { SystemLogo } from "../common/SystemLogo";
import { resolveImageSrc } from "../../utils/images";
import { getSystemAspectRatio } from "../../utils/cartridgeSizes";
import { esDateToMs, formatAgo } from "../../utils/format";
import { goToGame, playGameAt } from "../../utils/goToGame";
import { useAppStore } from "../../store/useAppStore";

/** Cajas que se pintan a cada lado del centro, y cómo se colocan las de los lados */
const WINDOW = 5;
const SIDE_SCALE = 0.62;
const SIDE_ANGLE = 50;
/** Lo que hay que desplazar la rueda o el trackpad (px) para pasar un juego */
const WHEEL_STEP = 60;

interface ContinueCarouselProps {
  games: ContinueGame[];
  /** El sistema del juego del centro (para el fondo del modo sofá) */
  onFocus: (systemName: string) => void;
}

/**
 * «Seguir jugando» en el modo sofá: los juegos de varios sistemas en fila como cajas 3D (cada una
 * con la forma y el estuche de su sistema), la del centro de frente. Debajo, nombre, sistema,
 * cuándo se jugó y la última partida guardada. ← → / rueda / clic para moverse; Intro (A) o clic
 * en la del centro abre su ficha y X juega (cambiando de sistema si hace falta). El carrusel normal
 * no sirve aquí: trabaja con los juegos de un solo sistema.
 */
export function ContinueCarousel({ games, onFocus }: ContinueCarouselProps) {
  const { t } = useTranslation();
  const locale = useI18nStore((s) => s.locale);
  const last = games.length - 1;
  const [center, setCenter] = useState(0);
  const go = (index: number) => setCenter(Math.min(Math.max(index, 0), Math.max(last, 0)));
  const current = games[Math.min(center, Math.max(last, 0))];

  useEffect(() => {
    if (current) onFocus(current.system_name);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current?.system_name]);

  // Abrir su ficha (sobre el modo sofá) o jugar
  const open = async (game: ContinueGame) => {
    await goToGame(game.system_path, game.game_path);
    useAppStore.getState().openFullView();
  };
  const play = (game: ContinueGame) => {
    if (!useAppStore.getState().runningGame) playGameAt(game.system_path, game.game_path);
  };

  // Teclado (y mando, que llega como teclas); nada con una ventana encima (la ficha incluida)
  const latest = useRef({ games, center, open, play });
  latest.current = { games, center, open, play };
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (document.querySelector(".fixed.inset-0")) return;
      const { games, center, open, play } = latest.current;
      const game = games[center];
      const moves: Record<string, number> = { ArrowLeft: center - 1, ArrowRight: center + 1, Home: 0, End: Number.MAX_SAFE_INTEGER };
      if (e.key in moves) {
        e.preventDefault();
        go(moves[e.key]);
      } else if (e.key === "Enter" && game) {
        e.preventDefault();
        open(game);
      } else if ((e.key === "x" || e.key === "X") && game) {
        e.preventDefault();
        play(game);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [last]);

  // Tamaño disponible y rueda/trackpad (escucha nativa: React no deja frenar el desplazamiento)
  const stageRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const observer = new ResizeObserver(() => setSize({ width: stage.clientWidth, height: stage.clientHeight }));
    observer.observe(stage);
    let pending = 0;
    function onWheel(e: WheelEvent) {
      e.preventDefault();
      const delta = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
      pending += e.deltaMode === 1 ? delta * WHEEL_STEP : delta;
      const steps = Math.trunc(pending / WHEEL_STEP);
      if (steps !== 0) {
        pending -= steps * WHEEL_STEP;
        setCenter((c) => Math.min(Math.max(c + steps, 0), Math.max(last, 0)));
      }
    }
    stage.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      observer.disconnect();
      stage.removeEventListener("wheel", onWheel);
    };
  }, [last]);

  if (!current) return <p className="m-auto max-w-lg text-center text-zinc-300">{t("continue.empty")}</p>;

  // Alto de las cajas: el de la zona de arriba (el texto va debajo); cada una con su forma
  const boxH = Math.max(0, size.height * 0.55);
  const boxTop = size.height * 0.06;
  const widthOf = (game: ContinueGame) => {
    const [w, h] = getSystemAspectRatio(game.system_name).split("/").map(Number);
    return boxH * (w > 0 && h > 0 ? w / h : 3 / 4);
  };
  const centreW = widthOf(current);
  const played = esDateToMs(current.lastplayed);

  return (
    <div ref={stageRef} data-couch-continue className="flex-1 relative isolate overflow-hidden select-none">
      {size.width > 0 &&
        games.map((game, i) => {
          const d = i - center;
          if (Math.abs(d) > WINDOW) return null;
          const w = widthOf(game);
          const side = Math.sign(d);
          // Las de los lados, junto a la del centro y luego apiladas
          const x = d === 0 ? 0 : side * (centreW / 2 + w * SIDE_SCALE * 0.4 + 24 + (Math.abs(d) - 1) * w * SIDE_SCALE * 0.38);
          const cover = game.cover ? resolveImageSrc(game.cover, game.system_path) || null : null;
          return (
            <div
              key={`${game.system_path}:${game.game_path}`}
              data-continue-box={d === 0 ? "center" : undefined}
              className="absolute cursor-pointer"
              onClick={() => (d === 0 ? open(game) : go(i))}
              style={{
                left: "50%",
                top: boxTop,
                width: w,
                height: boxH,
                marginLeft: -w / 2,
                zIndex: 100 - Math.abs(d),
                transform: `translateX(${x}px) scale(${d === 0 ? 1 : SIDE_SCALE})`,
                filter: d === 0 ? undefined : `brightness(${Math.max(0.35, 0.8 - 0.08 * Math.abs(d))})`,
                transition: "transform 0.35s cubic-bezier(0.2, 0.7, 0.3, 1), filter 0.35s",
              }}
            >
              <GameBox3D
                system={game.system_name}
                coverSrc={cover}
                title={game.name}
                aspectRatio={getSystemAspectRatio(game.system_name)}
                front={d === 0}
                rotate={d === 0 ? 0 : side < 0 ? SIDE_ANGLE : -SIDE_ANGLE}
                bothSpines
                shadow={false}
              />
            </div>
          );
        })}

      {/* Debajo del centro: nombre, sistema, cuándo se jugó y la última partida (con su captura) */}
      <div className="absolute inset-x-0 top-[66%] px-8 flex items-center justify-center gap-6 pointer-events-none">
        {current.last_save?.thumbnail && (
          <img
            src={resolveImageSrc(current.last_save.thumbnail, current.system_path) || ""}
            alt=""
            className="h-24 rounded-lg border border-white/20 shadow-xl shadow-black/50 [image-rendering:pixelated]"
          />
        )}
        <div className="min-w-0 text-center">
          <h2 className="text-3xl font-semibold text-white drop-shadow-lg truncate">{current.name}</h2>
          <p className="mt-2 flex items-center justify-center gap-3 text-base text-zinc-200">
            <SystemLogo name={current.system_name} width={90} maxHeight={22} />
            <span>{played ? t("continue.played", { when: formatAgo(played, locale) }) : t("continue.notPlayed")}</span>
            {current.last_save && <span>· {t("continue.saved", { when: formatAgo(current.last_save.modified * 1000, locale) })}</span>}
          </p>
        </div>
      </div>
    </div>
  );
}
