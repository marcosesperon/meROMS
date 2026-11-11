import { useEffect, useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { raProfile, type RaProfile } from "../../commands";
import { TrophyIcon } from "../common/Icons";

/**
 * Pantalla de inicio: tu perfil de RetroAchievements (avatar, puntos, puesto), tus últimos logros
 * con su icono y tus juegos recientes. Solo con cuenta configurada (Ajustes → General).
 */
export function RaProfileCard() {
  const { t } = useTranslation();
  const user = useAppStore((s) => s.settings.ra_user);
  const key = useAppStore((s) => s.settings.ra_apikey);
  const [profile, setProfile] = useState<RaProfile | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!user || !key) return;
    let alive = true;
    raProfile()
      .then((p) => alive && setProfile(p))
      .catch((e) => alive && setError(String(e)));
    return () => {
      alive = false;
    };
  }, [user, key]);

  if (!user || !key) return null;

  return (
    <section
      data-ra-profile
      className="home-rise relative overflow-hidden rounded-xl border border-amber-400/15 bg-[linear-gradient(135deg,rgba(180,83,9,0.22),rgba(255,255,255,0.02)_55%)] p-5"
      style={{ "--delay": "80ms" } as React.CSSProperties}
    >
      <span className="absolute -top-20 -left-10 w-56 h-56 rounded-full bg-amber-500/20 blur-3xl pointer-events-none" />
      <div className="relative flex flex-col md:flex-row gap-6">
        {/* Quién: avatar, nombre, puntos y puesto */}
        <div className="flex items-center gap-4 md:w-64 shrink-0">
          {profile?.user_pic ? (
            <img src={profile.user_pic} alt="" className="w-16 h-16 rounded-xl shadow-lg shadow-black/50" />
          ) : (
            <span className="w-16 h-16 rounded-xl bg-white/5 flex items-center justify-center text-amber-300">
              <TrophyIcon size={26} />
            </span>
          )}
          <div className="min-w-0">
            <p className="text-[10px] font-semibold uppercase tracking-[0.2em] text-amber-200/70">RetroAchievements</p>
            <p className="text-lg font-bold text-white truncate">{profile?.user ?? user}</p>
            {profile && (
              <p className="text-xs text-zinc-300 tabular-nums">
                {t("ra.points", { count: profile.points.toLocaleString() })}
                {profile.rank && profile.total_ranked ? ` · #${profile.rank.toLocaleString()} / ${profile.total_ranked.toLocaleString()}` : ""}
              </p>
            )}
          </div>
        </div>

        {error ? (
          <p className="text-sm text-red-300 break-words">{error}</p>
        ) : !profile ? (
          <p className="text-sm text-zinc-500">{t("ra.loading")}</p>
        ) : (
          <div className="flex-1 min-w-0 grid sm:grid-cols-2 gap-5">
            {/* Últimos logros */}
            <div className="min-w-0">
              <h4 className="text-[10px] font-semibold uppercase tracking-[0.2em] text-zinc-500 mb-2">{t("ra.recentAchievements")}</h4>
              {profile.recent_achievements.length === 0 ? (
                <p className="text-xs text-zinc-500">{t("stats.nothingYet")}</p>
              ) : (
                <ul className="space-y-1.5">
                  {profile.recent_achievements.slice(0, 5).map((a) => (
                    <li key={`${a.date}:${a.title}`} className="flex items-center gap-2.5 min-w-0" title={a.description}>
                      <img src={a.badge} alt="" className={`w-9 h-9 rounded-md shrink-0 ${a.hardcore ? "ring-1 ring-amber-400/70" : ""}`} />
                      <span className="min-w-0">
                        <span className="block text-xs text-zinc-100 truncate">{a.title}</span>
                        <span className="block text-[10px] text-zinc-500 truncate">
                          {a.game_title} · {a.points}
                        </span>
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </div>
            {/* Juegos recientes */}
            <div className="min-w-0">
              <h4 className="text-[10px] font-semibold uppercase tracking-[0.2em] text-zinc-500 mb-2">{t("ra.recentGames")}</h4>
              <ul className="space-y-1.5">
                {profile.recent_games.map((g) => (
                  <li key={g.game_id} className="flex items-center gap-2.5 min-w-0">
                    {g.icon && <img src={g.icon} alt="" className="w-9 h-9 rounded-md shrink-0" />}
                    <span className="min-w-0">
                      <span className="block text-xs text-zinc-100 truncate">{g.title}</span>
                      <span className="block text-[10px] text-zinc-500 truncate">
                        {g.console} · {g.last_played.slice(0, 10)}
                      </span>
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
