import { useState } from "react";
import { useTranslation } from "../../i18n";
import type { LibraryStats } from "../../commands";
import { resolveImageSrc } from "../../utils/images";
import { formatPlayTime } from "../../utils/playStats";
import { useAppStore } from "../../store/useAppStore";
import { DiceIcon, GamepadIcon } from "../common/Icons";
// El icono de la app tal cual (el mismo SVG del que salen los iconos)
import appIcon from "../../../src-tauri/icons/source/meroms.svg";

/**
 * Cabecera de la pantalla de inicio: un mosaico inclinado de carátulas de tu biblioteca (lo más
 * jugado y lo reciente primero) en penumbra, con grano y un degradado encima; delante, meROMS y
 * las cifras grandes de la biblioteca. Las cifras entran escalonadas.
 */
export function HomeHero({ stats, version }: { stats: LibraryStats | null; version: string }) {
  const { t } = useTranslation();
  const sum = (pick: (s: LibraryStats["systems"][number]) => number) => stats?.systems.reduce((n, s) => n + pick(s), 0) ?? 0;
  const figures = [
    { value: sum((s) => s.games).toLocaleString(), label: t("stats.games") },
    { value: String(stats?.systems.length ?? 0), label: t("stats.systems") },
    { value: formatPlayTime(String(sum((s) => s.gametime))) || "0 min", label: t("stats.playTime") },
    { value: sum((s) => s.playcount).toLocaleString(), label: t("stats.plays") },
  ];

  return (
    <section data-home-hero className="relative overflow-hidden rounded-2xl border border-white/5 bg-[#120f0e] min-h-[260px] isolate">
      {/* Mosaico de carátulas: más grande que la cabecera, girado, desenfocado y apagado */}
      <div className="absolute -inset-x-24 -inset-y-32 -z-10 grid grid-cols-8 gap-3 rotate-[-8deg] opacity-45 blur-[1.5px] saturate-[1.15]">
        {tiles(stats?.mosaic ?? []).map((cover, i) => (
          <MosaicCover key={`${i}:${cover.image}`} src={resolveImageSrc(cover.image, cover.system_path) ?? ""} index={i} />
        ))}
      </div>
      {/* Penumbra: más oscuro abajo y a la izquierda (donde va el texto) y un brillo violeta */}
      <div
        className="absolute inset-0 -z-10"
        style={{
          background:
            "radial-gradient(ellipse 60% 80% at 85% 0%, rgba(139,92,246,0.28), transparent 60%), linear-gradient(100deg, rgba(14,13,12,0.96) 25%, rgba(14,13,12,0.55) 70%, rgba(14,13,12,0.35)), linear-gradient(to top, rgba(14,13,12,0.95), transparent 55%)",
        }}
      />
      <div className="absolute inset-0 -z-10 home-grain pointer-events-none" />

      <div className="relative px-8 pt-8 pb-7 flex flex-col gap-7">
        <div className="flex items-center gap-4 home-rise">
          <img src={appIcon} alt="" className="w-16 h-16 drop-shadow-[0_8px_20px_rgba(0,0,0,0.6)]" />
          <div>
            <h1 className="text-4xl font-black tracking-tight text-white leading-none">
              meROMS
              {version && <span className="ml-3 align-middle text-xs font-medium tracking-[0.2em] text-zinc-400">v{version}</span>}
            </h1>
            <p className="mt-2 text-sm text-zinc-300/90">{t("home.tagline")}</p>
          </div>
          <div className="ml-auto self-start flex gap-2">
            {/* Modo sofá, empezando en la pantalla de sistemas («Seguir jugando» en el centro) */}
            <button onClick={() => useAppStore.getState().setCouchMode(true)} data-shot="couch-home" className={heroButton}>
              <GamepadIcon size={16} />
              {t("couch.short")}
            </button>
            {/* «¿A qué juego?» entre toda la biblioteca */}
            <button onClick={() => useAppStore.getState().setRandomOpen(true)} data-shot="random-home" className={heroButton}>
              <DiceIcon size={16} />
              {t("random.button")}
            </button>
          </div>
        </div>

        {/* Las cifras de la biblioteca, grandes */}
        <dl className="grid grid-cols-2 sm:grid-cols-4 gap-x-8 gap-y-4 max-w-3xl">
          {figures.map((f, i) => (
            <div key={f.label} className="home-rise" style={{ "--delay": `${120 + i * 70}ms` } as React.CSSProperties}>
              <dd className="text-3xl font-black tracking-tight text-white tabular-nums leading-none">{stats ? f.value : "–"}</dd>
              <dt className="mt-1.5 text-[10px] font-semibold uppercase tracking-[0.22em] text-indigo-200/70">{f.label}</dt>
            </div>
          ))}
        </dl>
      </div>
    </section>
  );
}

const heroButton =
  "flex items-center gap-2 px-4 py-2.5 rounded-xl bg-white/10 border border-white/15 text-sm font-semibold text-white backdrop-blur hover:bg-white/20 transition-colors";

/** Huecos del mosaico (5 filas de 8): con menos carátulas, se repiten en bucle */
const MOSAIC_SLOTS = 40;
function tiles<T>(covers: T[]): T[] {
  return covers.length === 0 ? [] : Array.from({ length: MOSAIC_SLOTS }, (_, i) => covers[i % covers.length]);
}

/** Una carátula del mosaico: aparece suave cuando ha cargado (y si falla, no se ve) */
function MosaicCover({ src, index }: { src: string; index: number }) {
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);
  if (failed || !src) return <div className="aspect-[3/4] rounded-md bg-white/5" />;
  return (
    <img
      src={src}
      alt=""
      loading="lazy"
      onLoad={() => setLoaded(true)}
      onError={() => setFailed(true)}
      className="aspect-[3/4] w-full rounded-md object-cover shadow-lg shadow-black/60 transition-opacity duration-700"
      style={{ opacity: loaded ? 1 : 0, transitionDelay: `${(index % 8) * 60}ms` }}
    />
  );
}
