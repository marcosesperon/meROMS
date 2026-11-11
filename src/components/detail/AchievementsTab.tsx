import { useEffect, useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { Game } from "../../types";
import { raGame, type RaGameReport } from "../../commands";
import { TrophyIcon } from "../common/Icons";

/**
 * Pestaña «Logros» (RetroAchievements): si la ROM es una versión compatible, los logros del juego
 * con su icono (en gris los que no tienes), puntos, descripción y cuándo se consiguieron, y tu
 * progreso normal y hardcore. Sin cuenta configurada, dice cómo ponerla.
 */
export function AchievementsTab({ game, systemPath }: { game: Game; systemPath: string }) {
  const { t } = useTranslation();
  const hasAccount = useAppStore((s) => !!s.settings.ra_user && !!s.settings.ra_apikey);
  const [report, setReport] = useState<RaGameReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!hasAccount) return;
    let alive = true;
    setReport(null);
    setError(null);
    setLoading(true);
    raGame(systemPath, game.path)
      .then((r) => alive && setReport(r))
      .catch((e) => alive && setError(String(e)))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, [hasAccount, systemPath, game.path]);

  const note = (text: string) => <p className="py-8 text-center text-sm text-zinc-500">{text}</p>;
  if (!hasAccount) return note(t("ra.noAccount"));
  if (loading) return note(t("ra.loading"));
  if (error) return <p className="py-6 text-center text-sm text-red-300 break-words">{error}</p>;
  if (!report) return null;

  // La etiqueta de arriba: qué se sabe de esta ROM
  const verdict = !report.known_console
    ? { text: t("ra.consoleUnknown"), cls: "bg-zinc-800 text-zinc-400" }
    : report.matched === "hash"
      ? { text: t("ra.compatible"), cls: "bg-emerald-600/20 text-emerald-300 border-emerald-500/30" }
      : report.matched === "name"
        ? { text: t("ra.otherVersion"), cls: "bg-amber-600/15 text-amber-300 border-amber-500/30" }
        : !report.hashable
          ? { text: t("ra.discNotYet"), cls: "bg-zinc-800 text-zinc-400" }
          : { text: t("ra.noAchievements"), cls: "bg-zinc-800 text-zinc-400" };
  const g = report.game;

  return (
    <div data-ra-tab={report.matched} className="space-y-4">
      <div className={`flex items-center gap-2 px-3 py-2 rounded-lg border border-transparent text-sm ${verdict.cls}`}>
        <TrophyIcon size={15} />
        <span>{verdict.text}</span>
        {report.hash && <span className="ml-auto font-mono text-[10px] text-zinc-500" title={t("ra.hash")}>{report.hash}</span>}
      </div>

      {g && (
        <>
          {/* Progreso */}
          <div className="flex items-center gap-4">
            {g.icon && <img src={g.icon} alt="" className="w-14 h-14 rounded-lg shadow-lg shadow-black/50" />}
            <div className="flex-1 min-w-0 space-y-2">
              <p className="text-sm font-semibold text-zinc-100 truncate">
                {g.title} <span className="text-zinc-500 font-normal">· {t("ra.points", { count: g.points })}</span>
              </p>
              <Progress label={t("ra.softcore")} value={g.awarded} total={g.num_achievements} color="bg-indigo-500" />
              <Progress label={t("ra.hardcore")} value={g.awarded_hardcore} total={g.num_achievements} color="bg-amber-500" />
            </div>
          </div>

          {/* Logros */}
          <ul className="space-y-1.5">
            {g.achievements.map((a) => {
              const earned = a.earned_hardcore ?? a.earned;
              return (
                <li key={a.id} className={`flex items-center gap-3 p-1.5 rounded-lg ${earned ? "bg-white/[0.04]" : ""}`}>
                  <img src={earned ? a.badge : a.badge_locked} alt="" loading="lazy" className={`w-11 h-11 rounded-md shrink-0 ${earned ? "" : "opacity-60"}`} />
                  <div className="flex-1 min-w-0">
                    <p className={`text-sm truncate ${earned ? "text-zinc-100" : "text-zinc-400"}`}>{a.title}</p>
                    <p className="text-xs text-zinc-500 line-clamp-2">{a.description}</p>
                  </div>
                  <div className="shrink-0 text-right">
                    <p className="text-xs font-semibold text-zinc-300 tabular-nums">{a.points}</p>
                    {earned && (
                      <p className={`text-[10px] ${a.earned_hardcore ? "text-amber-300" : "text-zinc-500"}`}>
                        {a.earned_hardcore ? `${t("ra.hardcore")} · ` : ""}
                        {earned.slice(0, 10)}
                      </p>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        </>
      )}
    </div>
  );
}

function Progress({ label, value, total, color }: { label: string; value: number; total: number; color: string }) {
  const pct = total > 0 ? Math.round((value / total) * 100) : 0;
  return (
    <div className="flex items-center gap-2 text-[11px] text-zinc-400">
      <span className="w-16 shrink-0">{label}</span>
      <span className="flex-1 h-1.5 rounded-full bg-white/10 overflow-hidden">
        <span className={`block h-full rounded-full ${color}`} style={{ width: `${pct}%` }} />
      </span>
      <span className="w-20 shrink-0 text-right tabular-nums">
        {value}/{total} · {pct}%
      </span>
    </div>
  );
}
