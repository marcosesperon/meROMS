import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { useFilteredGames } from "../../hooks/useFilteredGames";
import { GAMEPAD_ACTIVITY } from "../../hooks/useGamepad";
import { GameCarousel } from "../games/GameCarousel";
import { SystemCarousel } from "./SystemCarousel";
import { SystemLogo } from "../common/SystemLogo";
import { StarIcon } from "../common/Icons";
import type { SystemInfo } from "../../types";
import { systemBackdrop } from "../../utils/systemInfo";
import { goToSystem, openRandom, playSelected, switchSystem, toggleFavorite, toggleOnlyFavorites } from "../../utils/padActions";

/**
 * Tiempos del modo demostración (un objeto y no constantes para que la prueba del modo capturas
 * pueda acortarlos): sin tocar nada `idle` ms, empieza; cada `every` ms, otro juego.
 */
export const DEMO_TIMES = { idle: 2 * 60 * 1000, every: 15 * 1000 };

/**
 * Modo sofá: el carrusel del sistema a pantalla completa, para manejarlo con un mando (o con el
 * teclado). Dos pantallas: los juegos (con la que se entra) y los sistemas. Arriba el logo del
 * sistema, abajo la leyenda de botones. Las teclas (o los botones, que useGamepad convierte en
 * teclas): ← → moverse e Intro abrir (las atiende el carrusel de cada pantalla); en los juegos,
 * RePág/AvPág sistema, [ ] letra (carrusel), F favorito, S solo favoritos; Esc: de los juegos a
 * los sistemas y, desde los sistemas, salir.
 *
 * Ojo: la capa NO lleva las clases «fixed inset-0» (va con estilo en línea). La app usa esas
 * clases para saber que hay una ventana encima, y con ellas el carrusel y la ficha completa
 * dejarían de responder al teclado.
 */
export function CouchMode() {
  const { t } = useTranslation();
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const games = useFilteredGames();
  const onlyFavorites = useAppStore((s) => !!s.filters.favorite);
  const padConnected = usePadConnected();
  // Pantalla: los juegos del sistema o la lista de sistemas (y cuál está ahí en el centro)
  const [screen, setScreen] = useState<"games" | "systems">("games");
  const [focused, setFocused] = useState<SystemInfo | null>(null);
  const screenRef = useRef(screen);
  screenRef.current = screen;
  const demo = useDemo(games, screen === "games");

  // Pantalla completa mientras dure (y al salir, como estaba)
  useEffect(() => {
    const win = getCurrentWindow();
    let wasFull = true;
    win
      .isFullscreen()
      .then((full) => {
        wasFull = full;
        if (!full) return win.setFullscreen(true);
      })
      .catch(() => {});
    return () => {
      if (!wasFull) win.setFullscreen(false).catch(() => {});
    };
  }, []);

  // Teclas propias del modo (las de moverse y abrir son del carrusel)
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const target = e.target instanceof Element ? e.target : null;
      if (target?.closest("input, textarea, select, [contenteditable='true']")) return;
      // La ficha completa o cualquier ventana encima: las teclas son suyas
      if (document.querySelector(".fixed.inset-0")) return;
      if (e.key === "Escape") {
        e.preventDefault();
        // Volver: de los juegos a los sistemas; desde los sistemas, fuera del modo sofá
        if (screenRef.current === "games") setScreen("systems");
        else useAppStore.getState().setCouchMode(false);
        return;
      }
      const action = screenRef.current === "games" ? COUCH_KEYS[e.key] : undefined;
      if (!action) return;
      e.preventDefault();
      action();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  if (!selectedSystem) return null;
  const onSystems = screen === "systems";
  // Elegir un sistema en la lista: sus juegos (sigue con «solo favoritos» si estaba puesto)
  const pickSystem = (system: SystemInfo) => {
    if (system.path !== selectedSystem.path) goToSystem(system);
    setScreen("games");
  };

  return (
    <div
      data-couch
      className="z-30 flex flex-col select-none"
      style={{
        position: "fixed",
        top: 0,
        right: 0,
        bottom: 0,
        left: 0,
        // En la lista de sistemas, los colores del que está en el centro
        background: systemBackdrop((onSystems ? focused?.name : null) ?? selectedSystem.name),
        transition: "background 0.4s",
      }}
    >
      {/* Arriba: el logo del sistema (o «Sistemas») y si solo se ven los favoritos */}
      <div className="h-24 shrink-0 flex items-center justify-center gap-4 relative">
        {onSystems ? (
          <h1 className="text-2xl font-semibold tracking-wide text-white">{t("couch.systems")}</h1>
        ) : (
          <SystemLogo name={selectedSystem.name} width={220} maxHeight={56} />
        )}
        {!onSystems && onlyFavorites && (
          <span className="absolute right-8 flex items-center gap-1.5 px-3 py-1 rounded-full bg-black/40 text-sm text-yellow-300">
            <StarIcon size={14} className="text-yellow-400" />
            {t("couch.onlyFavorites")}
          </span>
        )}
        {demo && (
          <span data-couch-demo className="absolute left-8 px-3 py-1 rounded-full bg-black/40 text-sm text-zinc-200">
            {t("couch.demo")}
          </span>
        )}
      </div>

      {/* El carrusel de siempre, sin su fondo (el del modo sofá ya es el del sistema) */}
      <div className="flex-1 min-h-0 flex">
        {onSystems ? (
          <SystemCarousel onPick={pickSystem} onFocus={setFocused} />
        ) : games.length > 0 ? (
          <GameCarousel games={games} couch />
        ) : (
          <p className="m-auto text-zinc-300">{t(onlyFavorites ? "couch.noFavorites" : "couch.noGames")}</p>
        )}
      </div>

      {/* Leyenda: los botones del mando o, sin mando, las teclas */}
      <div className="h-12 shrink-0 flex items-center justify-center gap-6 bg-black/30 text-sm text-zinc-300">
        {(onSystems ? SYSTEMS_LEGEND : LEGEND).map(({ pad, key, label }) => (
          <span key={label} className="flex items-center gap-2">
            <kbd className="min-w-7 px-1.5 py-0.5 rounded-md bg-white/15 text-center text-xs font-semibold text-white">
              {padConnected ? pad : key}
            </kbd>
            {t(label)}
          </span>
        ))}
      </div>
    </div>
  );
}

/** Lo que hace cada tecla propia del modo sofá */
const COUCH_KEYS: Record<string, () => void> = {
  Escape: () => useAppStore.getState().setCouchMode(false),
  PageUp: () => switchSystem(-1),
  PageDown: () => switchSystem(1),
  f: () => toggleFavorite(),
  F: () => toggleFavorite(),
  s: () => toggleOnlyFavorites(),
  S: () => toggleOnlyFavorites(),
  x: () => playSelected(),
  X: () => playSelected(),
  r: openRandom,
  R: openRandom,
};

/** La leyenda de abajo: botón del mando, tecla equivalente y qué hace */
const LEGEND = [
  { pad: "X", key: "X", label: "play.button" },
  { pad: "A", key: "↵", label: "couch.open" },
  { pad: "B", key: "Esc", label: "couch.toSystems" },
  { pad: "L / R", key: "RePág / AvPág", label: "couch.system" },
  { pad: "LT / RT", key: "[ / ]", label: "couch.letter" },
  { pad: "Y", key: "F", label: "couch.favorite" },
  { pad: "Select", key: "S", label: "couch.onlyFavoritesToggle" },
  { pad: "R3", key: "R", label: "random.short" },
] as const;
const SYSTEMS_LEGEND = [
  { pad: "A", key: "↵", label: "couch.enter" },
  { pad: "B", key: "Esc", label: "couch.exit" },
] as const;

/** ¿Hay un mando conectado? (para la leyenda) */
function usePadConnected(): boolean {
  const check = () => Array.from(navigator.getGamepads?.() ?? []).some((p) => !!p?.connected);
  const [connected, setConnected] = useState(check);
  useEffect(() => {
    const update = () => setConnected(check());
    window.addEventListener("gamepadconnected", update);
    window.addEventListener("gamepaddisconnected", update);
    // Usar el mando también cuenta (algunos sistemas no lo enseñan hasta pulsar un botón)
    window.addEventListener(GAMEPAD_ACTIVITY, update);
    return () => {
      window.removeEventListener("gamepadconnected", update);
      window.removeEventListener("gamepaddisconnected", update);
      window.removeEventListener(GAMEPAD_ACTIVITY, update);
    };
  }, []);
  return connected;
}

/**
 * Modo demostración: sin tocar nada `DEMO_TIMES.idle`, va eligiendo juegos al azar (con vídeo, si
 * hay) cada `DEMO_TIMES.every`; la caja del centro se abre sola con su vídeo. Cualquier tecla,
 * botón, ratón o rueda lo para (esa pulsación no hace nada más) y vuelve al juego de antes.
 * No empieza con la ficha u otra ventana encima. Devuelve si está en marcha.
 */
function useDemo(games: ReturnType<typeof useFilteredGames>, enabled: boolean): boolean {
  const [running, setRunning] = useState(false);
  const runningRef = useRef(false);
  runningRef.current = running;
  // El juego en el que estabas al empezar (se vuelve a él al parar)
  const savedPath = useRef<string | null>(null);
  const gamesRef = useRef(games);
  gamesRef.current = games;
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;

  // Espera sin actividad; cualquier actividad la reinicia (y para la demostración)
  useEffect(() => {
    let idleTimer = 0;
    const arm = () => {
      window.clearTimeout(idleTimer);
      idleTimer = window.setTimeout(() => {
        const { fullViewOpen, selectedGame, runningGame } = useAppStore.getState();
        // Solo en la pantalla de juegos, sin jugar y sin la ficha u otra ventana encima
        if (!enabledRef.current || runningGame || fullViewOpen || document.querySelector(".fixed.inset-0")) return arm();
        savedPath.current = selectedGame?.path ?? null;
        setRunning(true);
      }, DEMO_TIMES.idle);
    };
    const onActivity = (e: Event) => {
      if (runningRef.current) {
        // La pulsación que despierta no hace nada más
        if (e.type === "keydown") {
          e.preventDefault();
          e.stopPropagation();
        }
        setRunning(false);
        const back = gamesRef.current.find((g) => g.path === savedPath.current);
        if (back) useAppStore.getState().selectGame(back);
      }
      arm();
    };
    const events = ["keydown", "mousemove", "mousedown", "wheel", GAMEPAD_ACTIVITY];
    // En captura: se entera antes que el carrusel (y puede frenar la tecla)
    events.forEach((name) => window.addEventListener(name, onActivity, true));
    arm();
    return () => {
      window.clearTimeout(idleTimer);
      events.forEach((name) => window.removeEventListener(name, onActivity, true));
    };
  }, []);

  // En marcha: un juego al azar ya y luego cada `every` (de los que tienen vídeo, si hay)
  useEffect(() => {
    if (!running) return;
    const pick = () => {
      const list = gamesRef.current;
      const withVideo = list.filter((g) => g.video);
      const pool = withVideo.length > 0 ? withVideo : list;
      if (pool.length > 0) useAppStore.getState().selectGame(pool[Math.floor(Math.random() * pool.length)]);
    };
    pick();
    const timer = window.setInterval(pick, DEMO_TIMES.every);
    return () => window.clearInterval(timer);
  }, [running]);

  return running;
}
