import { useEffect, useRef, useState } from "react";
import type { SystemInfo } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { SystemLogo, hasSystemLogo } from "../common/SystemLogo";
import { getSystemInfo, localize } from "../../utils/systemInfo";
import { sidebarSystems, systemGameCount } from "../../utils/systems";

/** Logos que se pintan a cada lado del centro */
const WINDOW = 6;
/** Separación (px): del centro al primero de cada lado y entre los demás; y lo que encogen */
const FIRST_GAP = 380;
const STEP = 260;
const SIDE_SCALE = 0.5;
/** Lo que hay que desplazar la rueda o el trackpad (px) para pasar un sistema */
const WHEEL_STEP = 60;

interface SystemCarouselProps {
  /** Entrar en los juegos de ese sistema (A, Intro o clic en el del centro) */
  onPick: (system: SystemInfo) => void;
  /** El sistema que está en el centro (para el fondo del modo sofá) */
  onFocus: (system: SystemInfo) => void;
}

/**
 * Pantalla de sistemas del modo sofá: los logos de la barra lateral en fila, el del centro grande
 * y los demás más pequeños y apagados; debajo, nombre, fabricante · año y número de juegos.
 * ← → / rueda / clic en un logo para moverse; Intro (A) o clic en el del centro para entrar.
 */
export function SystemCarousel({ onPick, onFocus }: SystemCarouselProps) {
  const { t, locale } = useTranslation();
  const systems = useAppStore((s) => s.systems);
  const onlyWithGames = useAppStore((s) => s.onlySystemsWithGames);
  const selectedPath = useAppStore((s) => s.selectedSystem?.path);
  const list = sidebarSystems(systems, onlyWithGames, selectedPath);
  const last = list.length - 1;

  // Empieza con el sistema actual en el centro
  const [center, setCenter] = useState(() => Math.max(0, list.findIndex((s) => s.path === selectedPath)));
  const go = (index: number) => setCenter(Math.min(Math.max(index, 0), Math.max(last, 0)));
  const centerRef = useRef(center);
  centerRef.current = center;
  const current = list[center];

  useEffect(() => {
    if (current) onFocus(current);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current?.path]);

  // Teclado (y mando, que llega como teclas): ← → Inicio Fin e Intro; nada con una ventana encima
  const latest = useRef({ list, onPick });
  latest.current = { list, onPick };
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (document.querySelector(".fixed.inset-0")) return;
      const now = centerRef.current;
      const moves: Record<string, number> = { ArrowLeft: now - 1, ArrowRight: now + 1, Home: 0, End: Number.MAX_SAFE_INTEGER };
      if (e.key in moves) {
        e.preventDefault();
        go(moves[e.key]);
      } else if (e.key === "Enter") {
        const system = latest.current.list[now];
        if (!system) return;
        e.preventDefault();
        latest.current.onPick(system);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [last]);

  // Rueda / trackpad (escucha nativa: React no deja frenar el desplazamiento)
  const stageRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
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
    return () => stage.removeEventListener("wheel", onWheel);
  }, [last]);

  if (!current) return <p className="m-auto text-zinc-300">{t("couch.noSystems")}</p>;

  const info = getSystemInfo(current.name);
  const details = [info ? localize(info.manufacturer, locale) : undefined, info?.year ?? undefined, `${systemGameCount(current)} ${t("header.games")}`]
    .filter(Boolean)
    .join(" · ");

  return (
    <div ref={stageRef} data-couch-systems className="flex-1 relative overflow-hidden">
      {list.map((system, i) => {
        const d = i - center;
        if (Math.abs(d) > WINDOW) return null;
        const far = Math.abs(d);
        return (
          <button
            key={system.path}
            data-system-path={system.path}
            data-system-center={d === 0 || undefined}
            onClick={() => (d === 0 ? onPick(system) : go(i))}
            className="absolute left-1/2 top-[42%] w-[360px] h-[160px] -ml-[180px] -mt-[80px] flex items-center justify-center"
            style={{
              transform: `translateX(${d === 0 ? 0 : Math.sign(d) * (FIRST_GAP + (far - 1) * STEP)}px) scale(${d === 0 ? 1 : SIDE_SCALE})`,
              opacity: d === 0 ? 1 : Math.max(0.15, 0.6 - 0.1 * far),
              zIndex: 100 - far,
              transition: "transform 0.35s cubic-bezier(0.2, 0.7, 0.3, 1), opacity 0.35s",
            }}
          >
            {/* Sin logo: su nombre, en grande */}
            {hasSystemLogo(system.name) ? (
              <SystemLogo name={system.name} width={340} maxHeight={140} />
            ) : (
              <span className="text-4xl font-bold text-white text-center leading-tight">{system.full_name || system.name}</span>
            )}
          </button>
        );
      })}
      {/* Debajo del centro: nombre y datos */}
      <div className="absolute inset-x-0 top-[62%] px-8 text-center pointer-events-none">
        <h2 className="text-3xl font-semibold text-white drop-shadow-lg truncate">{current.full_name}</h2>
        <p className="mt-2 text-base text-zinc-200">{details}</p>
      </div>
    </div>
  );
}
