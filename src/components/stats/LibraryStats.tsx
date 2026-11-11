import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { libraryDiskCached, libraryDiskUsage, type DiskUsage, type LibraryStats as Stats, type TopGame } from "../../commands";
import type { PlayStatus } from "../../commands";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "../common/TaskProgress";
import { SystemLogo, hasSystemLogo } from "../common/SystemLogo";
import { ChartIcon, ClockIcon, FolderIcon, HistoryIcon, StarIcon, TrophyIcon } from "../common/Icons";
import { STATUS_FLAGS, STATUS_META } from "../../utils/gameStatus";
import { formatPlayTime } from "../../utils/playStats";
import { formatBytes, formatReleaseDate } from "../../utils/format";
import { resolveImageSrc } from "../../utils/images";
import { goToGame } from "../../utils/goToGame";

/**
 * Colores del gráfico de estados, por orden (el de los textos de la app es demasiado claro para
 * rellenos): pasan el validador de paletas sobre fondo oscuro (franja de luminosidad, croma,
 * separación con daltonismo y sin él). Mismo tono que cada estado: verde, ámbar, azul, rojo, violeta.
 */
const STATUS_CHART: { status: PlayStatus; color: string }[] = [
  { status: "finished", color: "#059669" },
  { status: "completed", color: "#b45309" },
  { status: "incomplete", color: "#0284c7" },
  { status: "abandoned", color: "#e11d48" },
  { status: "never", color: "#8b5cf6" },
];

/** Tipos de medio de la cobertura, en este orden (campo de la gamelist → texto) */
const MEDIA: { field: string; label: TranslationKeys }[] = [
  { field: "image", label: "images.boxArt" },
  { field: "thumbnail", label: "images.thumbnail" },
  { field: "titleshot", label: "images.titleshot" },
  { field: "marquee", label: "images.marquee" },
  { field: "video", label: "images.video" },
  { field: "manual", label: "images.manual" },
  { field: "fanart", label: "images.fanart" },
  { field: "boxart", label: "images.boxart" },
  { field: "boxback", label: "images.boxback" },
  { field: "cartridge", label: "images.cartridge" },
  { field: "mix", label: "images.mix" },
];

/**
 * Las estadísticas de la pantalla de inicio, debajo de la cabecera y los sistemas: actividad
 * (más jugados y recientes con su carátula, tiempo por sistema), estados, colección (favoritos,
 * ocultos, sin ROM y cobertura de medios) y espacio en disco. El espacio se mide aparte (tarda) y
 * se guarda: si ya se midió, sale lo guardado con su fecha.
 */
export function LibraryStats({ stats, paths, fullName }: { stats: Stats; paths: string[]; fullName: (path: string) => string }) {
  const { t, locale } = useTranslation();

  // Espacio: lo guardado o, si nunca se midió, medirlo ya
  const diskTask = useTask();
  const [disk, setDisk] = useState<DiskUsage | null>(null);
  const measure = () => diskTask.run((task) => libraryDiskUsage(paths, task)).then((r) => r && setDisk(r)).catch(() => {});
  useEffect(() => {
    if (paths.length === 0) return;
    libraryDiskCached()
      .then((cached) => (cached ? setDisk(cached) : measure()))
      .catch(() => measure());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [paths]);

  const sum = (pick: (s: Stats["systems"][number]) => number) => stats.systems.reduce((n, s) => n + pick(s), 0);
  const totalGames = sum((s) => s.games);
  const byTime = stats.systems.filter((s) => s.gametime > 0).sort((a, b) => b.gametime - a.gametime);
  const statusTotal = (key: string) => sum((s) => s.statuses[key] ?? 0);
  const withStatus = stats.systems.filter((s) => Object.keys(s.statuses).length > 0);

  return (
    <div data-library-stats className="grid gap-4 lg:grid-cols-2">
      {/* Actividad */}
      <Card title={t("stats.activity")} icon={<ClockIcon size={15} />} accent="#8b5cf6" delay={0}>
        <div className="grid sm:grid-cols-2 gap-5">
          <GameList title={t("stats.mostPlayed")} icon={<TrophyIcon size={13} />} games={stats.most_played} detail={(g) => t("stats.times", { count: g.playcount })} />
          <GameList title={t("stats.recent")} icon={<HistoryIcon size={13} />} games={stats.recent} detail={(g) => formatReleaseDate(g.lastplayed ?? undefined)} />
        </div>
        {byTime.length > 0 && (
          <Bars
            title={t("stats.timeBySystem")}
            rows={byTime.map((s) => ({ key: s.path, label: s.name, value: s.gametime, text: formatPlayTime(String(s.gametime)), hint: fullName(s.path) }))}
          />
        )}
      </Card>

      {/* Estados */}
      <Card title={t("stats.statuses")} icon={<TrophyIcon size={15} />} accent="#059669" delay={60}>
        <StatusBar totals={STATUS_CHART.map((c) => ({ ...c, count: statusTotal(c.status) }))} />
        <div className="flex flex-wrap gap-2 mt-4">
          {STATUS_FLAGS.map((f) => (
            <span key={f.key} className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-white/5 border border-white/5 text-xs text-zinc-300">
              <f.icon size={13} className={f.color} />
              {t(f.label)}
              <span className="font-bold text-zinc-100 tabular-nums">{statusTotal(f.key)}</span>
            </span>
          ))}
        </div>
        {withStatus.length > 0 ? (
          <div className="mt-4 max-h-56 overflow-y-auto">
            <table className="w-full text-xs">
              <thead className="text-zinc-500">
                <tr>
                  <th className="text-left font-normal py-1">{t("stats.system")}</th>
                  {[...STATUS_CHART.map((c) => STATUS_META[c.status]), ...STATUS_FLAGS].map((m) => (
                    <th key={m.label} className="font-normal py-1 w-9" title={t(m.label)}>
                      <m.icon size={13} className={`inline ${m.color}`} />
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="text-zinc-300 tabular-nums">
                {withStatus.map((s) => (
                  <tr key={s.path} className="border-t border-white/5">
                    <td className="py-1 truncate max-w-0 w-full" title={fullName(s.path)}>
                      {fullName(s.path)}
                    </td>
                    {[...STATUS_CHART.map((c) => c.status), ...STATUS_FLAGS.map((f) => f.key)].map((key) => (
                      <td key={key} className="py-1 text-center">
                        {s.statuses[key] || <span className="text-zinc-600">–</span>}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="mt-4 text-xs text-zinc-500">{t("stats.noStatuses")}</p>
        )}
      </Card>

      {/* Colección: favoritos, ocultos, sin ROM y qué medios tienen los juegos */}
      <Card title={t("stats.collection")} icon={<StarIcon size={15} />} accent="#b45309" delay={120}>
        <div className="grid grid-cols-3 gap-2">
          <Figure label={t("stats.favorites")} value={sum((s) => s.favorites)} />
          <Figure label={t("stats.hidden")} value={sum((s) => s.hidden)} />
          <Figure label={t("stats.missing")} value={sum((s) => s.missing)} warn />
        </div>
        <Bars
          title={t("stats.coverage")}
          max={totalGames}
          rows={MEDIA.map((m) => {
            const count = sum((s) => s.media[m.field] ?? 0);
            const pct = totalGames > 0 ? Math.round((count / totalGames) * 100) : 0;
            return { key: m.field, label: t(m.label), value: count, text: `${pct} %`, hint: `${count} / ${totalGames}`, plain: true };
          })}
        />
      </Card>

      {/* Espacio en disco */}
      <Card title={t("stats.space")} icon={<FolderIcon size={15} />} accent="#0284c7" delay={180}>
        {diskTask.running ? (
          <TaskProgress progress={diskTask.progress} label={t("stats.measuring")} onCancel={diskTask.cancel} cancelling={diskTask.cancelling} compact />
        ) : disk ? (
          <>
            <SpaceTable disk={disk} fullName={fullName} />
            <div className="mt-3 flex items-center justify-between gap-2 text-[11px] text-zinc-500">
              <span>{t("stats.measuredAt", { date: new Date(disk.measured_at * 1000).toLocaleString(locale) })}</span>
              <button onClick={measure} className="px-2.5 py-1 rounded-full border border-white/10 text-zinc-300 hover:bg-white/5">
                {t("stats.measureAgain")}
              </button>
            </div>
          </>
        ) : null}
      </Card>
    </div>
  );
}

/** Tarjeta de un bloque: icono en una pastilla del color del bloque, borde fino y entrada escalonada */
function Card({ title, icon, accent, delay, children }: { title: string; icon: React.ReactNode; accent: string; delay: number; children: React.ReactNode }) {
  return (
    <section
      className="home-rise relative min-w-0 overflow-hidden rounded-xl border border-white/5 bg-[linear-gradient(160deg,rgba(255,255,255,0.045),rgba(255,255,255,0.01)_60%)] p-5"
      style={{ "--delay": `${delay}ms` } as React.CSSProperties}
    >
      {/* Un toque del color del bloque en la esquina */}
      <span className="absolute -top-16 -right-16 w-40 h-40 rounded-full blur-3xl opacity-25 pointer-events-none" style={{ background: accent }} />
      <h3 className="relative flex items-center gap-2.5 text-sm font-semibold text-zinc-100 mb-4">
        <span className="flex items-center justify-center w-7 h-7 rounded-lg text-white" style={{ background: accent }}>
          {icon}
        </span>
        {title}
      </h3>
      <div className="relative">{children}</div>
    </section>
  );
}

/** Una cifra con su etiqueta */
function Figure({ label, value, warn = false }: { label: string; value: number; warn?: boolean }) {
  return (
    <div className="rounded-lg bg-white/[0.04] border border-white/5 px-3 py-2.5 min-w-0">
      <div className={`text-2xl font-black tabular-nums leading-none ${warn && value > 0 ? "text-red-300" : "text-white"}`}>{value.toLocaleString()}</div>
      <div className="mt-1 text-[10px] uppercase tracking-[0.18em] text-zinc-500 truncate">{label}</div>
    </div>
  );
}

interface BarRow {
  key: string;
  /** Carpeta del sistema (sale su logo) o, con `plain`, el texto tal cual */
  label: string;
  value: number;
  /** Lo que se escribe al final de la barra */
  text: string;
  hint?: string;
  plain?: boolean;
}

/**
 * Barras horizontales finas de una sola serie (un solo color, el de acento: hay sistemas con
 * colores casi negros que no se verían), con el extremo redondeado y el valor en texto al lado
 */
function Bars({ title, rows, max }: { title: string; rows: BarRow[]; max?: number }) {
  const top = max ?? Math.max(1, ...rows.map((r) => r.value));
  return (
    <div className="mt-5">
      <h4 className="text-[10px] font-semibold uppercase tracking-[0.2em] text-zinc-500 mb-2.5">{title}</h4>
      <div className="max-h-64 overflow-y-auto pr-1 space-y-1.5">
        {rows.map((r) => (
          <div key={r.key} title={r.hint ? `${r.hint}: ${r.text}` : r.text} className="flex items-center gap-3">
            <span className="w-24 shrink-0 flex items-center text-xs text-zinc-300 truncate">
              {r.plain || !hasSystemLogo(r.label) ? r.label : <SystemLogo name={r.label} width={80} maxHeight={18} />}
            </span>
            <span className="flex-1 h-2 rounded-full bg-white/5 overflow-hidden">
              <span
                className="block h-full rounded-full"
                style={{ width: `${Math.max(r.value > 0 ? 1.5 : 0, (r.value / top) * 100)}%`, background: "linear-gradient(90deg,#6366f1,#a78bfa)" }}
              />
            </span>
            <span className="w-16 shrink-0 text-right text-xs text-zinc-300 tabular-nums">{r.text}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

/** Los juegos con estado, repartidos por estado en una barra (con hueco entre tramos) y la leyenda */
function StatusBar({ totals }: { totals: { status: PlayStatus; color: string; count: number }[] }) {
  const { t } = useTranslation();
  const total = totals.reduce((n, s) => n + s.count, 0);
  return (
    <div>
      <div className="flex h-4 gap-0.5 rounded-full overflow-hidden bg-white/5">
        {total > 0 &&
          totals
            .filter((s) => s.count > 0)
            .map((s) => (
              <span key={s.status} title={`${t(STATUS_META[s.status].label)}: ${s.count}`} style={{ width: `${(s.count / total) * 100}%`, background: s.color }} />
            ))}
      </div>
      <div className="grid grid-cols-2 sm:grid-cols-3 gap-x-4 gap-y-2 mt-3">
        {totals.map((s) => {
          const meta = STATUS_META[s.status];
          return (
            <span key={s.status} className="flex items-center gap-2 text-xs text-zinc-300">
              <span className="w-2.5 h-2.5 rounded-sm shrink-0" style={{ background: s.color }} />
              <meta.icon size={13} className={meta.color} />
              <span className="truncate">{t(meta.label)}</span>
              <span className="ml-auto font-bold text-zinc-100 tabular-nums">{s.count}</span>
            </span>
          );
        })}
      </div>
    </div>
  );
}

/** Lista corta de juegos de toda la biblioteca, con su carátula; un clic va al juego */
function GameList({ title, icon, games, detail }: { title: string; icon: React.ReactNode; games: TopGame[]; detail: (g: TopGame) => string }) {
  const { t } = useTranslation();
  return (
    <div className="min-w-0">
      <h4 className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.2em] text-zinc-500 mb-2.5">
        {icon}
        {title}
      </h4>
      {games.length === 0 ? (
        <p className="text-xs text-zinc-500">{t("stats.nothingYet")}</p>
      ) : (
        <ul className="space-y-1">
          {games.map((g) => (
            <li key={`${g.system_path}:${g.game_path}`}>
              <button
                onClick={() => goToGame(g.system_path, g.game_path)}
                className="group w-full flex items-center gap-2.5 p-1 rounded-lg text-xs hover:bg-white/5 text-left"
              >
                <Cover src={(g.cover && resolveImageSrc(g.cover, g.system_path)) || ""} />
                <span className="flex-1 min-w-0">
                  <span className="block truncate text-zinc-100 group-hover:text-white">{g.name}</span>
                  <span className="mt-0.5 flex items-center gap-1.5 text-zinc-500">
                    <SystemLogo name={g.system_name} width={40} maxHeight={11} className="opacity-70" />
                    <span className="tabular-nums">{detail(g)}</span>
                  </span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** Carátula pequeña de una fila (si no hay o falla, un hueco del mismo tamaño) */
function Cover({ src }: { src: string }) {
  const [failed, setFailed] = useState(false);
  return src && !failed ? (
    <img src={src} alt="" loading="lazy" onError={() => setFailed(true)} className="w-8 h-11 shrink-0 rounded object-cover shadow-md shadow-black/50" />
  ) : (
    <span className="w-8 h-11 shrink-0 rounded bg-white/5 flex items-center justify-center text-zinc-600">
      <ChartIcon size={12} />
    </span>
  );
}

/** Espacio por sistema: ROMs, medios y total, de mayor a menor, con la suma al final */
function SpaceTable({ disk, fullName }: { disk: DiskUsage; fullName: (path: string) => string }) {
  const { t } = useTranslation();
  const rows = [...disk.systems].sort((a, b) => b.roms + b.media - (a.roms + a.media));
  const roms = rows.reduce((n, r) => n + r.roms, 0);
  const media = rows.reduce((n, r) => n + r.media, 0);
  return (
    <div className="max-h-64 overflow-y-auto">
      <table data-space-table className="w-full text-xs tabular-nums">
        <thead className="text-zinc-500">
          <tr>
            <th className="text-left font-normal py-1">{t("stats.system")}</th>
            <th className="text-right font-normal py-1">{t("stats.roms")}</th>
            <th className="text-right font-normal py-1">{t("stats.media")}</th>
            <th className="text-right font-normal py-1">{t("stats.total")}</th>
          </tr>
        </thead>
        <tbody className="text-zinc-300">
          {rows.map((r) => (
            <tr key={r.path} className="border-t border-white/5">
              <td className="py-1 truncate max-w-0 w-full" title={fullName(r.path)}>
                {fullName(r.path)}
              </td>
              <td className="py-1 text-right pl-3">{formatBytes(r.roms)}</td>
              <td className="py-1 text-right pl-3">{formatBytes(r.media)}</td>
              <td className="py-1 text-right pl-3 text-zinc-100">{formatBytes(r.roms + r.media)}</td>
            </tr>
          ))}
          <tr className="border-t border-white/15 font-bold text-zinc-100">
            <td className="py-1.5">{t("stats.total")}</td>
            <td className="py-1.5 text-right pl-3">{formatBytes(roms)}</td>
            <td className="py-1.5 text-right pl-3">{formatBytes(media)}</td>
            <td className="py-1.5 text-right pl-3">{formatBytes(roms + media)}</td>
          </tr>
        </tbody>
      </table>
    </div>
  );
}
