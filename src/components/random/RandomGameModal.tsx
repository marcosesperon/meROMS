import { useEffect, useMemo, useRef, useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { libraryGames, type LibraryGame } from "../../commands";
import { useFilteredGames } from "../../hooks/useFilteredGames";
import { resolveImageSrc } from "../../utils/images";
import { goToGame, playGameAt } from "../../utils/goToGame";
import { SystemLogo } from "../common/SystemLogo";
import { DiceIcon, PlayIcon } from "../common/Icons";

/** Un juego que puede salir: de un sistema concreto, con su carátula ya resuelta */
interface Candidate {
  systemPath: string;
  systemName: string;
  gamePath: string;
  name: string;
  cover: string;
  year: string;
  genre: string;
  played: boolean;
  favorite: boolean;
}

/** Carátulas de la tira de la ruleta, en cuál se para y lo que mide cada hueco (px) */
const STRIP = 44;
const WIN_AT = 38;
const SLOT = 124;
const ITEM = 112;
/** Lo que tarda en frenar */
const SPIN_MS = 3200;

const pickRandom = <T,>(list: T[]): T => list[Math.floor(Math.random() * list.length)];

/**
 * «¿A qué juego?»: elige un juego al azar y lo enseña con una ruleta de carátulas que frena en el
 * elegido; luego su carátula, nombre y datos, con Jugar, Ver ficha y Otra vez. Dentro de un
 * sistema sortea entre los juegos que se ven; en la pantalla de inicio, entre toda la biblioteca
 * con un selector antes (sistema, sin jugar, favoritos, género). Los ocultos no entran nunca.
 * Teclado (y mando, que llega como teclas): ← → botón, Intro pulsar, R otra vez, Esc cerrar.
 */
export function RandomGameModal() {
  const open = useAppStore((s) => s.randomOpen);
  if (!open) return null;
  return <RandomGame />;
}

function RandomGame() {
  const { t } = useTranslation();
  const setOpen = useAppStore((s) => s.setRandomOpen);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const systems = useAppStore((s) => s.systems);
  const visible = useFilteredGames();
  const close = () => setOpen(false);

  // De dónde se sortea: los juegos que se ven del sistema, o toda la biblioteca (se pide al abrir)
  const libraryMode = !selectedSystem;
  const [library, setLibrary] = useState<LibraryGame[] | null>(null);
  useEffect(() => {
    if (!libraryMode) return;
    libraryGames(systems.filter((s) => s.has_gamelist).map((s) => s.path)).then(setLibrary).catch(() => setLibrary([]));
  }, [libraryMode, systems]);

  // Selector de la biblioteca
  const [systemFilter, setSystemFilter] = useState("");
  const [onlyUnplayed, setOnlyUnplayed] = useState(false);
  const [onlyFavorites, setOnlyFavorites] = useState(false);
  const [genre, setGenre] = useState("");

  const pool: Candidate[] = useMemo(() => {
    if (!libraryMode) {
      const system = selectedSystem!;
      return visible
        .filter((g) => g.hidden !== "true")
        .map((g) => ({
          systemPath: system.path,
          systemName: system.name,
          gamePath: g.path,
          name: g.name || g.path,
          cover: resolveImageSrc(g.image, system.path) ?? "",
          year: g.releasedate?.slice(0, 4) ?? "",
          genre: g.genre ?? "",
          played: Number(g.playcount ?? 0) > 0 || !!g.lastplayed,
          favorite: g.favorite === "true",
        }));
    }
    return (library ?? [])
      .filter((g) => (!systemFilter || g.system_path === systemFilter) && (!onlyUnplayed || !g.played) && (!onlyFavorites || g.favorite) && (!genre || g.genre === genre))
      .map((g) => ({
        systemPath: g.system_path,
        systemName: g.system_name,
        gamePath: g.game_path,
        name: g.name,
        cover: (g.cover && resolveImageSrc(g.cover, g.system_path)) || "",
        year: g.year ?? "",
        genre: g.genre ?? "",
        played: g.played,
        favorite: g.favorite,
      }));
  }, [libraryMode, selectedSystem, visible, library, systemFilter, onlyUnplayed, onlyFavorites, genre]);

  const genres = useMemo(
    () => [...new Set((library ?? []).filter((g) => !systemFilter || g.system_path === systemFilter).map((g) => g.genre).filter((g): g is string => !!g))].sort(),
    [library, systemFilter],
  );

  // Fases: el selector (solo en la biblioteca), la ruleta girando y el resultado
  const [phase, setPhase] = useState<"options" | "spinning" | "result">(libraryMode ? "options" : "spinning");
  const [strip, setStrip] = useState<Candidate[]>([]);
  const [spinKey, setSpinKey] = useState(0);
  const winner = strip[WIN_AT];

  const spin = () => {
    if (pool.length === 0) return;
    const chosen = pickRandom(pool);
    setStrip(Array.from({ length: STRIP }, (_, i) => (i === WIN_AT ? chosen : pickRandom(pool))));
    setSpinKey((k) => k + 1);
    // Con «menos movimiento» en el sistema, directo al resultado
    setPhase(window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "result" : "spinning");
  };
  // Dentro de un sistema se tira al abrir
  const started = useRef(false);
  useEffect(() => {
    if (!libraryMode && !started.current && pool.length > 0) {
      started.current = true;
      spin();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pool.length]);

  const play = async () => {
    if (!winner) return;
    close();
    await playGameAt(winner.systemPath, winner.gamePath);
  };
  const details = async () => {
    if (!winner) return;
    close();
    await goToGame(winner.systemPath, winner.gamePath);
    useAppStore.getState().openFullView();
  };
  const actions = [play, details, spin];
  const [focus, setFocus] = useState(0);

  // Teclado: lo atiende esta ventana antes que nadie (y no deja pasar las teclas a lo de debajo)
  // Siempre lo de ahora (la lista que se sortea cambia con el selector)
  const latest = useRef({ phase, actions, focus, spin });
  latest.current = { phase, actions, focus, spin };
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const target = e.target instanceof Element ? e.target : null;
      const typing = !!target?.closest("input:not([type='checkbox']), select, textarea");
      const { phase, actions, focus, spin } = latest.current;
      let handled = true;
      if (e.key === "Escape") close();
      else if (typing) handled = false;
      else if (e.key === "r" || e.key === "R") spin();
      else if (phase === "options" && e.key === "Enter") spin();
      else if (phase === "result" && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        // También en la referencia: un Intro justo detrás ya pulsa el botón nuevo
        const next = (focus + (e.key === "ArrowRight" ? 1 : 2)) % 3;
        latest.current.focus = next;
        setFocus(next);
      } else if (phase === "result" && e.key === "Enter") actions[focus]();
      else handled = false;
      if (handled) {
        e.preventDefault();
        e.stopPropagation();
      }
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const label = "text-[10px] font-semibold uppercase tracking-[0.2em] text-zinc-500";

  return (
    <div data-random className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm" onMouseDown={close}>
      <div
        className="relative w-[760px] max-w-[calc(100vw-2rem)] overflow-hidden rounded-2xl border border-white/10 bg-[#141110] shadow-2xl shadow-black/70"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="absolute -top-24 left-1/2 -translate-x-1/2 w-96 h-48 rounded-full bg-indigo-600/25 blur-3xl pointer-events-none" />
        <div className="relative px-6 pt-5 flex items-center gap-2.5">
          <span className="flex items-center justify-center w-8 h-8 rounded-lg bg-indigo-600 text-white">
            <DiceIcon size={17} />
          </span>
          <h2 className="text-lg font-bold text-white">{t("random.title")}</h2>
          <span className="ml-auto text-xs text-zinc-500 tabular-nums">{t("random.pool", { count: pool.length })}</span>
        </div>

        {phase === "options" ? (
          // Selector de la biblioteca
          <div className="relative px-6 py-5 space-y-4">
            <div className="grid sm:grid-cols-2 gap-3">
              <label className="block">
                <span className={label}>{t("random.system")}</span>
                <select
                  value={systemFilter}
                  onChange={(e) => {
                    setSystemFilter(e.target.value);
                    setGenre("");
                  }}
                  className="mt-1 w-full bg-zinc-900 border border-zinc-700 rounded-md px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
                >
                  <option value="">{t("random.allSystems")}</option>
                  {systems
                    .filter((s) => s.has_gamelist)
                    .map((s) => (
                      <option key={s.path} value={s.path}>
                        {s.full_name}
                      </option>
                    ))}
                </select>
              </label>
              <label className="block">
                <span className={label}>{t("random.genre")}</span>
                <select
                  value={genre}
                  onChange={(e) => setGenre(e.target.value)}
                  className="mt-1 w-full bg-zinc-900 border border-zinc-700 rounded-md px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
                >
                  <option value="">{t("random.anyGenre")}</option>
                  {genres.map((g) => (
                    <option key={g} value={g}>
                      {g}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="flex flex-wrap gap-5 text-sm text-zinc-300">
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="checkbox" checked={onlyUnplayed} onChange={(e) => setOnlyUnplayed(e.target.checked)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                {t("random.unplayed")}
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="checkbox" checked={onlyFavorites} onChange={(e) => setOnlyFavorites(e.target.checked)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                {t("random.favorites")}
              </label>
            </div>
            <div className="flex justify-end">
              <button
                data-random-spin
                onClick={spin}
                disabled={pool.length === 0}
                className="flex items-center gap-2 px-5 py-2.5 text-sm font-bold bg-indigo-600 text-white rounded-lg hover:bg-indigo-500 disabled:opacity-40 transition-colors"
              >
                <DiceIcon size={15} />
                {library === null ? t("random.loading") : pool.length === 0 ? t("random.empty") : t("random.spin")}
              </button>
            </div>
          </div>
        ) : pool.length === 0 ? (
          <p className="relative px-6 py-10 text-center text-sm text-zinc-400">{t("random.empty")}</p>
        ) : (
          <>
            {/* La ruleta: la tira se desliza y frena con la elegida bajo el marcador */}
            <Reel key={spinKey} strip={strip} spinning={phase === "spinning"} onStop={() => setPhase("result")} />

            {/* El resultado */}
            {phase === "result" && winner && (
              <div data-random-result={`${winner.systemPath}|${winner.gamePath}`} className="relative px-6 pb-6 home-rise">
                <div className="flex items-center gap-2 text-xs text-zinc-400">
                  <SystemLogo name={winner.systemName} width={70} maxHeight={16} />
                  <span>{[winner.year, winner.genre].filter(Boolean).join(" · ")}</span>
                </div>
                <h3 className="mt-1 text-2xl font-black tracking-tight text-white line-clamp-2">{winner.name}</h3>
                <div className="mt-4 flex flex-wrap gap-2">
                  {[
                    { text: t("play.button"), icon: <PlayIcon size={13} />, cls: "bg-emerald-600 hover:bg-emerald-500 text-white" },
                    { text: t("random.details"), icon: null, cls: "bg-zinc-800 hover:bg-zinc-700 text-zinc-100" },
                    { text: t("random.again"), icon: <DiceIcon size={13} />, cls: "bg-zinc-800 hover:bg-zinc-700 text-zinc-100" },
                  ].map((b, i) => (
                    <button
                      key={b.text}
                      data-random-action={i}
                      onClick={() => actions[i]()}
                      onMouseEnter={() => setFocus(i)}
                      className={`flex items-center gap-2 px-4 py-2 text-sm font-semibold rounded-lg transition-colors ${b.cls} ${
                        focus === i ? "ring-2 ring-white/70 ring-offset-2 ring-offset-[#141110]" : ""
                      }`}
                    >
                      {b.icon}
                      {b.text}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

/**
 * La tira de carátulas: arranca quieta y, en el siguiente fotograma, se desliza hasta dejar la de
 * WIN_AT en el centro, frenando (curva muy cerrada al final). Avisa al pararse.
 */
function Reel({ strip, spinning, onStop }: { strip: Candidate[]; spinning: boolean; onStop: () => void }) {
  const boxRef = useRef<HTMLDivElement>(null);
  const [offset, setOffset] = useState(0);
  useEffect(() => {
    const width = boxRef.current?.clientWidth ?? 700;
    // Un poco de juego dentro del hueco, para que no pare siempre en el centro exacto
    const jitter = (Math.random() - 0.5) * ITEM * 0.5;
    const target = -(WIN_AT * SLOT) + width / 2 - ITEM / 2 + jitter;
    if (!spinning) {
      setOffset(-(WIN_AT * SLOT) + width / 2 - ITEM / 2);
      return;
    }
    const frame = requestAnimationFrame(() => requestAnimationFrame(() => setOffset(target)));
    // Por si el evento del final de la transición no llega (ventana oculta…)
    const timer = window.setTimeout(onStop, SPIN_MS + 200);
    return () => {
      cancelAnimationFrame(frame);
      window.clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div ref={boxRef} className="relative mt-5 mb-5 h-[176px] overflow-hidden">
      {/* Bordes que se funden y el marcador del centro */}
      <div className="absolute inset-y-0 left-0 w-24 z-10 bg-gradient-to-r from-[#141110] to-transparent pointer-events-none" />
      <div className="absolute inset-y-0 right-0 w-24 z-10 bg-gradient-to-l from-[#141110] to-transparent pointer-events-none" />
      <div className="absolute left-1/2 -translate-x-1/2 top-0 bottom-0 w-[124px] z-10 rounded-xl border-2 border-indigo-400/80 shadow-[0_0_30px_rgba(129,140,248,0.35)] pointer-events-none" />
      <div
        className="absolute top-2 left-0 flex gap-3"
        style={{
          transform: `translateX(${offset}px)`,
          transition: spinning && offset !== 0 ? `transform ${SPIN_MS}ms cubic-bezier(0.08, 0.75, 0.12, 1)` : "none",
        }}
      >
        {strip.map((c, i) => (
          <div key={i} className="w-[112px] h-[160px] shrink-0 overflow-hidden rounded-lg bg-zinc-800 shadow-lg shadow-black/60">
            {c.cover ? (
              <img src={c.cover} alt="" className="w-full h-full object-cover" />
            ) : (
              <div className="w-full h-full flex items-center justify-center p-2 text-center text-[11px] font-semibold text-zinc-300">{c.name}</div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
