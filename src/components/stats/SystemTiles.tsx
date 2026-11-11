import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { SystemStats } from "../../commands";
import { SystemLogo, hasSystemLogo } from "../common/SystemLogo";
import { systemGradient } from "../../utils/systemInfo";
import { sidebarSystems, systemGameCount } from "../../utils/systems";

/**
 * Rejilla de sistemas de la pantalla de inicio: los de la barra lateral, en su orden. Cada
 * tarjeta con el degradado de su sistema, el logo grande, sus juegos y cuántos se han jugado.
 * Al pasar el ratón se eleva; un clic entra en el sistema.
 */
export function SystemTiles({ stats }: { stats: SystemStats[] | null }) {
  const { t } = useTranslation();
  const systems = useAppStore((s) => s.systems);
  const onlyWithGames = useAppStore((s) => s.onlySystemsWithGames);
  const selectSystem = useAppStore((s) => s.selectSystem);
  const list = sidebarSystems(systems, onlyWithGames);
  const byPath = new Map((stats ?? []).map((s) => [s.path, s]));

  return (
    <section>
      <h2 className="mb-3 text-[11px] font-semibold uppercase tracking-[0.22em] text-zinc-500">{t("home.systems")}</h2>
      <div className="grid gap-3 grid-cols-[repeat(auto-fill,minmax(190px,1fr))]">
        {list.map((system, i) => {
          const own = byPath.get(system.path);
          const games = own?.games ?? systemGameCount(system);
          const played = own?.played ?? 0;
          return (
            <button
              key={system.path}
              data-system-tile={system.path}
              onClick={() => selectSystem(system)}
              title={system.full_name}
              className="home-rise group relative h-32 overflow-hidden rounded-xl border border-white/10 text-left shadow-lg shadow-black/30 transition-[transform,box-shadow,border-color] duration-200 hover:-translate-y-1 hover:border-white/25 hover:shadow-xl hover:shadow-black/50"
              style={{ background: `${systemGradient(system.name)}, #1a1715`, "--delay": `${Math.min(i, 20) * 35}ms` } as React.CSSProperties}
            >
              {/* Brillo diagonal y oscurecido abajo, para que se lean los números */}
              <span className="absolute inset-0 bg-[linear-gradient(135deg,rgba(255,255,255,0.14),transparent_45%)]" />
              <span className="absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-black/55 to-transparent" />

              <span className="absolute inset-x-4 top-3 bottom-11 flex items-center justify-center transition-transform duration-300 group-hover:scale-105">
                {hasSystemLogo(system.name) ? (
                  <SystemLogo name={system.name} width={150} maxHeight={50} className="drop-shadow-[0_3px_8px_rgba(0,0,0,0.55)]" />
                ) : (
                  <span className="text-lg font-bold text-white text-center leading-tight line-clamp-2">{system.full_name || system.name}</span>
                )}
              </span>

              <span className="absolute inset-x-4 bottom-3">
                <span className="flex items-baseline justify-between gap-2 text-[11px] text-white/85">
                  <span>
                    <span className="text-sm font-bold text-white tabular-nums">{games.toLocaleString()}</span> {t("header.games")}
                  </span>
                  {played > 0 && <span className="tabular-nums text-white/70">{t("home.played", { count: played })}</span>}
                </span>
                {/* Jugados de sus juegos */}
                <span className="mt-1.5 block h-1 rounded-full bg-white/15 overflow-hidden">
                  <span className="block h-full rounded-full bg-white/80" style={{ width: `${games > 0 ? (played / games) * 100 : 0}%` }} />
                </span>
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}
