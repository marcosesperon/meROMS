import { useEffect, useRef, useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { Game } from "../../types";
import { useGameActions } from "../../hooks/useGameActions";
import { useGameDetails } from "../../hooks/useGameDetails";
import { ScrapeSearchModal } from "../common/ScrapeSearchModal";
import { StatusMenu } from "../common/StatusMenu";
import { CloseIcon, PencilIcon, PlayIcon, StarIcon, StarOutlineIcon, TrashIcon } from "../common/Icons";
import { GameTabs } from "./GameTabs";

/**
 * Ficha de un juego a pantalla completa (idea de RomM): lo del panel derecho en grande, con
 * flechas para pasar al juego anterior/siguiente de la lista tal como se ve (filtros y orden).
 * Se abre con el ajuste «Al pulsar un juego» o con el botón de ampliar del panel.
 */
export function GameFullView() {
  const open = useAppStore((s) => s.fullViewOpen);
  const game = useAppStore((s) => s.selectedGame);
  const closeFullView = useAppStore((s) => s.closeFullView);
  // Sin juego (borrado, otro sistema, selección múltiple…) no hay nada que enseñar
  useEffect(() => {
    if (open && !game) closeFullView();
  }, [open, game, closeFullView]);
  if (!open || !game) return null;
  return <FullView game={game} />;
}

function FullView({ game }: { game: Game }) {
  const { t } = useTranslation();
  const games = useAppStore((s) => s.games);
  const visibleOrder = useAppStore((s) => s.visibleOrder);
  const selectGame = useAppStore((s) => s.selectGame);
  const openEditor = useAppStore((s) => s.openEditor);
  const playGame = useAppStore((s) => s.playGame);
  const playing = useAppStore((s) => !!s.runningGame);
  const closeFullView = useAppStore((s) => s.closeFullView);
  const toggleGameFlag = useAppStore((s) => s.toggleGameFlag);
  const isFavorite = game.favorite === "true";
  const { scrape, remove, scrapeTarget, closeScrape, applyScraped } = useGameActions();
  const { systemPath, markFailed, media, shown, setCurrent, cover, hero, heroStyle, details } = useGameDetails(game);
  const rootRef = useRef<HTMLDivElement>(null);

  // Posición en la lista que se ve; el anterior/siguiente se elige de ahí
  const index = visibleOrder.indexOf(game.path);
  const pathAt = (i: number) => (i >= 0 && i < visibleOrder.length ? visibleOrder[i] : undefined);
  const prevPath = pathAt(index - 1);
  const nextPath = pathAt(index + 1);
  const go = (path?: string) => {
    const target = path && games.find((g) => g.path === path);
    if (target) selectGame(target);
  };

  // Teclado: ← → para moverse y Esc para cerrar. Se escucha antes que nadie (captura) y se cede
  // si hay otra ventana encima (el editor, la búsqueda de scraper, una confirmación…)
  const latest = useRef({ prevPath, nextPath, go });
  latest.current = { prevPath, nextPath, go };
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const others = Array.from(document.querySelectorAll(".fixed.inset-0")).some((el) => el !== rootRef.current);
      // Un menú desplegable abierto (estados…): Esc y las flechas son para él
      if (others || document.querySelector("[data-popup-open]")) return;
      const target = e.target as HTMLElement | null;
      if (target?.closest("input, textarea, select, [contenteditable='true']")) return;
      const { prevPath, nextPath, go } = latest.current;
      if (e.key === "Escape") closeFullView();
      // En el modo sofá, A (Intro) dentro de la ficha juega
      else if (e.key === "Enter" && useAppStore.getState().couchMode) {
        const { selectedGame, playGame } = useAppStore.getState();
        if (selectedGame) playGame(selectedGame);
      }
      else if (e.key === "ArrowLeft") go(prevPath);
      else if (e.key === "ArrowRight") go(nextPath);
      else return;
      e.preventDefault();
      e.stopPropagation();
    }
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [closeFullView]);

  // Al cerrar, el listado enseña el juego en el que te quedaste
  useEffect(
    () => () => {
      const path = useAppStore.getState().selectedGame?.path;
      if (path) document.querySelector(`[data-game-path="${CSS.escape(path)}"]`)?.scrollIntoView({ block: "nearest" });
    },
    []
  );

  const subtitle = [game.developer, game.releasedate?.slice(0, 4), game.genre].filter(Boolean).join(" · ");
  const fits = useFitsScreen();

  // Piezas que comparten las dos disposiciones
  const coverImg = (size: string) =>
    cover && (
      <img
        src={cover}
        alt=""
        onError={() => markFailed(cover)}
        // Tamaño según su forma: alta o apaisada, sin deformarse
        className={`w-auto h-auto ${size} shrink-0 rounded-md shadow-2xl shadow-black/70 ring-1 ring-black/50`}
      />
    );
  const heading = (
    <>
      <h1 className="text-3xl font-bold text-white leading-tight drop-shadow">{game.name || game.path}</h1>
      {subtitle && <p className="mt-2 text-sm text-indigo-300">{subtitle}</p>}

      {/* Acciones (las mismas que el panel derecho) */}
      <div className="mt-5 flex flex-wrap gap-2">
        <button
          onClick={() => playGame(game)}
          disabled={playing}
          className="flex items-center gap-2 px-6 py-2 text-sm font-semibold bg-emerald-600 text-white rounded-md hover:bg-emerald-500 transition-colors disabled:opacity-50"
        >
          <PlayIcon size={14} />
          {t("play.button")}
        </button>
        <button
          onClick={openEditor}
          className="flex items-center gap-2 px-5 py-2 text-sm font-semibold bg-indigo-600 text-white rounded-md hover:bg-indigo-500 transition-colors"
        >
          <PencilIcon size={14} />
          {t("info.edit")}
        </button>
        <button
          onClick={() => scrape(game)}
          disabled={!!scrapeTarget}
          className="px-4 py-2 text-sm bg-zinc-800 text-emerald-400 rounded-md hover:bg-zinc-700 transition-colors disabled:opacity-50"
        >
          {t("detail.scrape")}
        </button>
        <button
          onClick={() => toggleGameFlag(game.path, "favorite")}
          title={t(isFavorite ? "info.unfavorite" : "info.favorite")}
          className={`px-3 py-2 text-sm bg-zinc-800 rounded-md hover:bg-zinc-700 transition-colors ${
            isFavorite ? "text-yellow-400" : "text-zinc-400 hover:text-yellow-400"
          }`}
        >
          {isFavorite ? <StarIcon size={14} /> : <StarOutlineIcon size={14} />}
        </button>
        <StatusMenu game={game} buttonClassName="px-3 py-2 text-sm bg-zinc-800 rounded-md hover:bg-zinc-700 transition-colors" />
        <button
          onClick={() => remove(game)}
          title={t("detail.delete")}
          className="px-3 py-2 text-sm bg-zinc-800 text-red-400 rounded-md hover:bg-zinc-700 transition-colors"
        >
          <TrashIcon size={14} />
        </button>
      </div>
    </>
  );
  const detailList = (
    <dl className="grid grid-cols-3 gap-x-6 gap-y-3">
      {details
        .filter(([, value]) => !!value)
        .map(([label, value]) => (
          <div key={label} className="min-w-0">
            <dt className="text-[11px] text-zinc-500">{t(label)}</dt>
            <dd className="text-sm text-zinc-200 truncate" title={value}>{value}</dd>
          </div>
        ))}
    </dl>
  );
  const tabs = (fill: boolean) => (
    <GameTabs game={game} systemPath={systemPath} media={media} shown={shown} onPick={setCurrent} onFailed={markFailed} fill={fill} />
  );
  const arrow = "absolute top-1/2 -translate-y-1/2 z-10 w-11 h-11 rounded-full bg-black/50 text-zinc-200 text-2xl leading-none hover:bg-black/80 hover:text-white disabled:opacity-0 transition";

  return (
    <div ref={rootRef} data-full-view className="fixed inset-0 z-[35] bg-zinc-950 flex flex-col">
      {/* Fondo: la escena (o la carátula desenfocada) muy apagada */}
      {hero && (
        <img src={hero} alt="" onError={() => markFailed(hero)} className={`absolute inset-0 w-full h-full object-cover opacity-30 ${heroStyle}`} />
      )}
      <div className="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/85 to-zinc-950/40" />

      {/* Barra de arriba: posición en la lista y cerrar */}
      <div className="relative z-10 flex items-center justify-between px-6 py-4">
        <span className="text-xs text-zinc-400">
          {index >= 0 && t("fullView.position", { n: index + 1, total: visibleOrder.length })}
        </span>
        <button
          onClick={closeFullView}
          title={t("fullView.close")}
          className="p-2 rounded-full bg-black/50 text-zinc-300 hover:text-white hover:bg-black/80"
        >
          <CloseIcon size={18} />
        </button>
      </div>

      <button onClick={() => go(prevPath)} disabled={!prevPath} title={t("fullView.prev")} className={`${arrow} left-4`}>
        ‹
      </button>
      <button onClick={() => go(nextPath)} disabled={!nextPath} title={t("fullView.next")} className={`${arrow} right-4`}>
        ›
      </button>

      {fits ? (
        // Ventana grande: todo cabe sin desplazarse. Arriba la cabecera (carátula, título y
        // botones); debajo, desde la base de la carátula, datos a la izquierda y pestañas a la derecha
        <div className="relative flex-1 min-h-0 w-full max-w-[1600px] mx-auto px-20 pb-8 flex flex-col">
          <div className="flex gap-6 items-end shrink-0">
            {coverImg("max-h-[30vh] max-w-[240px]")}
            <div className="min-w-0 flex-1">{heading}</div>
          </div>
          <div
            className="mt-6 flex-1 min-h-0 grid gap-10"
            // Una sola fila del alto que queda: las columnas no pueden crecer más que la ventana
            style={{ gridTemplateColumns: "minmax(0, 5fr) minmax(0, 6fr)", gridTemplateRows: "minmax(0, 1fr)" }}
          >
            <div className="flex flex-col min-h-0">
              {detailList}
              {game.desc && (
                <p className="mt-5 flex-1 min-h-0 overflow-y-auto pr-2 text-sm leading-relaxed text-zinc-300 whitespace-pre-line">{game.desc}</p>
              )}
              <p className="mt-3 text-[11px] text-zinc-600 font-mono truncate" title={game.path}>{game.path}</p>
            </div>
            {tabs(true)}
          </div>
        </div>
      ) : (
        // Ventana pequeña: todo seguido, desplazándose (las pestañas al final)
        <div className="relative flex-1 overflow-y-auto">
          <div className="max-w-6xl mx-auto px-20 pb-12 pt-2">
            <div className="flex gap-10 items-start">
              {coverImg("max-h-[420px] max-w-[300px]")}
              <div className="min-w-0 flex-1">
                {heading}
                {game.desc && <p className="mt-6 text-sm leading-relaxed text-zinc-300 whitespace-pre-line max-w-3xl">{game.desc}</p>}
                <div className="mt-6 max-w-3xl">{detailList}</div>
              </div>
            </div>
            <div className="mt-10">{tabs(false)}</div>
            <p className="mt-8 text-[11px] text-zinc-600 font-mono break-all text-center">{game.path}</p>
          </div>
        </div>
      )}

      {/* Scrapear = búsqueda manual; lo elegido se guarda en el acto */}
      {scrapeTarget && (
        <ScrapeSearchModal systemPath={systemPath} game={scrapeTarget} onClose={closeScrape} onApplied={applyScraped} />
      )}
    </div>
  );
}

// Desde este tamaño de ventana la ficha va en dos columnas y cabe entera sin desplazarse
const FIT_MIN_WIDTH = 1100;
const FIT_MIN_HEIGHT = 620;

/** ¿La ventana es lo bastante grande para la ficha en dos columnas? (se recalcula al redimensionar) */
function useFitsScreen(): boolean {
  const check = () => window.innerWidth >= FIT_MIN_WIDTH && window.innerHeight >= FIT_MIN_HEIGHT;
  const [fits, setFits] = useState(check);
  useEffect(() => {
    const onResize = () => setFits(check());
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return fits;
}
