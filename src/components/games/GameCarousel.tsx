import { useEffect, useMemo, useRef, useState } from "react";
import type { Game } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { GameBox3D } from "./GameBox3D";
import { cardAspectRatio } from "../../utils/cartridgeSizes";
import { getSystemView, shapeField } from "../../utils/systemView";
import { nameText, sortGames, sortnameText } from "../../utils/playStats";
import { systemBackdrop } from "../../utils/systemInfo";
import { resolveImageSrc } from "../../utils/images";
import { formatRating } from "../../utils/format";
import { getFlag } from "../../utils/flags";
import { statusBadge, statusText } from "../../utils/gameStatus";
import { StarIcon } from "../common/Icons";
import { LetterRow } from "./LetterRow";
import { clickGame, doubleClickGame } from "../../utils/gameClick";

/** Cajas que se pintan a cada lado del centro (las demás no existen: va igual con miles) */
const WINDOW = 12;
/** Tamaño de las cajas de los lados respecto a la del centro */
const SIDE_SCALE = 0.8;
/** Giro de las cajas de los lados (casi de canto: se ve sobre todo el lomo) */
const SIDE_ANGLE = 64;
/** Lo que hay que desplazar la rueda o el trackpad (px) para pasar un juego */
const WHEEL_STEP = 60;
/** Lo que espera el carrusel parado antes de seleccionar el juego del centro */
const SELECT_DELAY = 150;
/** Alto reservado debajo de las cajas: título, datos y la barra */
const BOTTOM = 150;

/**
 * Vista «Carrusel»: las cajas 3D en fila, la del centro grande y de frente y las de los lados
 * giradas y apiladas enseñando el lomo, sobre los colores del sistema y con su reflejo. El juego
 * del centro es el seleccionado (el panel de la derecha lo enseña). Se mueve con ← →, Inicio/Fin,
 * la rueda o el trackpad, un clic en una caja de los lados o la barra de abajo.
 */
export function GameCarousel({ games }: { games: Game[] }) {
  const { t } = useTranslation();
  const settings = useAppStore((s) => s.settings);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const selectedPath = useAppStore((s) => s.selectedGame?.path);
  const systemName = selectedSystem?.name || "";

  // Los mismos juegos y en el mismo orden que la cuadrícula
  const gridSort = useAppStore((s) => s.settings.grid_sort);
  const sorted = useMemo(() => sortGames(games, gridSort), [games, gridSort]);
  const setVisibleOrder = useAppStore((s) => s.setVisibleOrder);
  useEffect(() => setVisibleOrder(sorted.map((g) => g.path)), [sorted, setVisibleOrder]);

  // Forma de las cajas: la elegida para el sistema o la medida en las carátulas
  const field = shapeField(settings, systemName, "carousel");
  const mediaShape = useAppStore((s) => s.mediaShape);
  const refreshMediaShape = useAppStore((s) => s.refreshMediaShape);
  const measured = mediaShape && mediaShape.system === selectedSystem?.path && mediaShape.field === field ? mediaShape : null;
  const aspectRatio = cardAspectRatio(systemName, getSystemView(settings, systemName).aspect_ratio, measured);
  useEffect(() => {
    if (selectedSystem && (mediaShape?.system !== selectedSystem.path || mediaShape?.field !== field)) {
      refreshMediaShape(selectedSystem.path);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedSystem?.path, field]);

  // El juego del centro: al entrar, el que ya estuviera seleccionado
  const [center, setCenter] = useState(() => Math.max(0, sorted.findIndex((g) => g.path === selectedPath)));
  const last = sorted.length - 1;
  const go = (index: number) => setCenter(Math.min(Math.max(index, 0), Math.max(last, 0)));
  const centerRef = useRef(center);
  centerRef.current = center;

  // Si la selección cambia desde fuera (búsqueda global, anterior/siguiente de la ficha…), ir a él
  useEffect(() => {
    const index = sorted.findIndex((g) => g.path === selectedPath);
    if (index >= 0 && index !== centerRef.current) setCenter(index);
  }, [selectedPath, sorted]);
  // La lista se ha acortado (filtros): que el centro no se salga
  useEffect(() => {
    if (center > last) setCenter(Math.max(last, 0));
  }, [center, last]);
  // Destello: qué caja brilla y un número que sube cada vez (para que la animación empiece de
  // nuevo); solo la caja que se ha quedado en el centro, no las que pasan por él
  const [glint, setGlint] = useState({ index: -1, count: 0 });
  // Parado un momento en un juego: se selecciona (yendo rápido no se recarga el panel a cada paso)
  // y su caja brilla una vez
  useEffect(() => {
    const game = sorted[center];
    if (!game) return;
    const timer = window.setTimeout(() => {
      setGlint((g) => ({ index: center, count: g.count + 1 }));
      const { selectedGame, selectGame } = useAppStore.getState();
      if (selectedGame?.path !== game.path) selectGame(game);
    }, SELECT_DELAY);
    return () => window.clearTimeout(timer);
  }, [center, sorted]);

  // Clic en la caja del centro: como en la cuadrícula y, con «panel», además abre el panel
  const clickCentre = (game: Game) => {
    clickGame(game);
    const { settings, setCarouselPanelOpen } = useAppStore.getState();
    if (settings.click_action !== "full") setCarouselPanelOpen(true);
  };

  // Intro, como «Al pulsar un juego»: ficha completa o, con el panel, el editor (como la cuadrícula)
  const openWithEnter = (game: Game) => {
    const { selectGame, openFullView, openEditor, settings } = useAppStore.getState();
    selectGame(game);
    if (settings.click_action === "full") openFullView();
    else openEditor();
  };

  // Teclado: ← → Inicio Fin e Intro, salvo escribiendo o con una ventana abierta
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      // El destino puede no ser un elemento (la ventana misma)
      const target = e.target instanceof Element ? e.target : null;
      if (target?.closest("input:not([type='range']), textarea, select, [contenteditable='true']")) return;
      if (document.querySelector(".fixed.inset-0")) return;
      const now = centerRef.current;
      const moves: Record<string, number> = { ArrowLeft: now - 1, ArrowRight: now + 1, Home: 0, End: Number.MAX_SAFE_INTEGER };
      if (e.key in moves) {
        e.preventDefault();
        go(moves[e.key]);
      } else if (e.key === "Enter" && !target?.closest("button, a")) {
        const game = sorted[now];
        if (!game) return;
        e.preventDefault();
        openWithEnter(game);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sorted, last]);

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
      // Lo que más se mueva, horizontal o vertical; la rueda «por líneas» cuenta como un paso
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

  // Medidas de la caja del centro: lo que quepa en alto, sin pasar de un tercio del ancho
  const [rw, rh] = aspectRatio.split("/").map(Number);
  const ratio = rw > 0 && rh > 0 ? rw / rh : 3 / 4;
  const stageHeight = Math.max(0, size.height - BOTTOM);
  let boxH = stageHeight * 0.82;
  let boxW = boxH * ratio;
  if (boxW > size.width * 0.34) {
    boxW = size.width * 0.34;
    boxH = boxW / ratio;
  }
  const boxTop = Math.max(0, (stageHeight - boxH) / 2 + 10);
  // Separación: la primera de cada lado, junto a la del centro; las demás, apiladas
  const firstGap = boxW * 0.5 + boxW * SIDE_SCALE * 0.32 + 12;
  const stackStep = Math.max(16, boxW * SIDE_SCALE * 0.17);

  const placement = (d: number) => {
    if (d === 0) return { x: 0, scale: 1, angle: 0, light: 1 };
    const side = Math.sign(d);
    return {
      x: side * (firstGap + (Math.abs(d) - 1) * stackStep),
      scale: SIDE_SCALE,
      angle: side < 0 ? SIDE_ANGLE : -SIDE_ANGLE,
      light: Math.max(0.35, 0.85 - 0.05 * Math.abs(d)),
    };
  };

  const current = sorted[center];
  const byName = !gridSort || gridSort === "name" || gridSort === "sortname";
  const from = Math.max(0, center - WINDOW);
  const to = Math.min(last, center + WINDOW);
  const visible: number[] = [];
  for (let i = from; i <= to; i++) visible.push(i);

  return (
    // «isolate»: los z-index de las cajas (hasta 100) se quedan dentro del carrusel; sin él se
    // pintaban por encima de las ventanas (z-50), como el editor
    <div className="flex-1 relative isolate overflow-hidden select-none bg-zinc-950" style={{ background: systemBackdrop(systemName) }}>
      <div ref={stageRef} className="absolute inset-0">
        {size.width > 0 &&
          visible.map((i) => {
            const game = sorted[i];
            const d = i - center;
            const p = placement(d);
            return (
              <div
                key={`${i}:${game.path}`}
                className="absolute cursor-pointer"
                // Una de los lados viene al centro; la del centro, como un clic en la cuadrícula
                // (ficha completa si así está en Ajustes; doble clic, el editor)
                onClick={() => (d === 0 ? clickCentre(game) : go(i))}
                onDoubleClick={() => d === 0 && doubleClickGame(game)}
                style={{
                  left: "50%",
                  top: boxTop,
                  width: boxW,
                  height: boxH,
                  marginLeft: -boxW / 2,
                  zIndex: 100 - Math.abs(d),
                  transform: `translateX(${p.x}px) scale(${p.scale})`,
                  filter: p.light < 1 ? `brightness(${p.light})` : undefined,
                  transition: "transform 0.35s cubic-bezier(0.2, 0.7, 0.3, 1), filter 0.35s",
                  // Reflejo en el suelo, pegado a la base (la caja ocupa el 84 % del alto, centrada)
                  WebkitBoxReflect: `below ${-boxH * 0.16}px linear-gradient(transparent 72%, rgba(255,255,255,0.22))`,
                }}
              >
                <GameBox3D
                  system={systemName}
                  coverSrc={resolveImageSrc(game.image, selectedSystem?.path || "") || null}
                  title={game.name || game.path}
                  aspectRatio={aspectRatio}
                  front={d === 0}
                  rotate={p.angle}
                  bothSpines
                  shadow={false}
                  glint={d === 0 && glint.index === i ? glint.count : undefined}
                />
              </div>
            );
          })}
      </div>

      {/* Debajo: título y datos del juego del centro, y la barra para saltar por la lista */}
      {current && <CenterInfo game={current} />}
      {sorted.length > 1 && (
        <div className="absolute inset-x-0 bottom-3 flex items-center gap-3 px-8">
          {/* Por nombre, saltar a letra (como la cuadrícula); con otro orden irían salteadas: la barra */}
          {byName ? (
            <LetterRow games={sorted} nameOf={gridSort === "sortname" ? sortnameText : nameText} current={center} onJump={go} />
          ) : (
            <input
              type="range"
              min={0}
              max={last}
              value={center}
              onChange={(e) => go(Number(e.target.value))}
              className="carousel-range flex-1"
              // Hasta dónde va lo recorrido (lo pinta App.css más claro)
              style={{ "--progress": `${last > 0 ? (center / last) * 100 : 0}%` } as React.CSSProperties}
              title={t("carousel.jump")}
            />
          )}
          <span className="text-xs text-zinc-300 tabular-nums w-24 text-right">
            {center + 1} / {sorted.length}
          </span>
        </div>
      )}
    </div>
  );
}

/** Título grande y una línea con año · desarrollador · género · nota, bandera y distintivos */
function CenterInfo({ game }: { game: Game }) {
  const { t } = useTranslation();
  const romMissing = useAppStore((s) => s.missingRomPaths.has(game.path));
  const status = useAppStore((s) => s.statuses.get(game.path));
  const badge = statusBadge(status);
  const details = [game.releasedate?.slice(0, 4), game.developer, game.genre].filter(Boolean).join(" · ");
  const flag = getFlag(game.lang);
  return (
    <div className="absolute inset-x-0 bottom-12 px-8 text-center pointer-events-none">
      <div className="flex items-center justify-center gap-2">
        {game.favorite === "true" && <StarIcon size={18} className="text-yellow-400 shrink-0" />}
        {badge && (
          <span title={statusText(status, t)} className="shrink-0">
            <badge.icon size={16} className={badge.color} />
          </span>
        )}
        <h2 className="text-2xl font-semibold text-white drop-shadow-lg truncate">{game.name || game.path}</h2>
        {romMissing && (
          <span className="shrink-0 px-1.5 py-0.5 rounded bg-red-600/90 text-[11px] font-semibold text-white">{t("missingRoms.badge")}</span>
        )}
      </div>
      <p className="mt-1 text-sm text-zinc-300 truncate">
        {details}
        {flag && <span className="ml-2" title={game.lang}>{flag}</span>}
        {game.rating && <span className="ml-2 text-yellow-400">{formatRating(game.rating)}</span>}
      </p>
    </div>
  );
}
