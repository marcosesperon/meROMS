import { useEffect, useState } from "react";
import { checkBios, systemLocations, type SystemLocations } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { TranslationKeys as TranslationKey } from "../../i18n/en";
import type { Game } from "../../types";
import { formatRating, formatReleaseDate } from "../../utils/format";
import { useGameDetails } from "../../hooks/useGameDetails";
import { formatPlayTime, isPlayed } from "../../utils/playStats";
import { useGameActions } from "../../hooks/useGameActions";
import { getSystemInfo, localize, systemGradient, type SystemInfoEntry } from "../../utils/systemInfo";
import { SystemLogo } from "../common/SystemLogo";
import { ScrapeSearchModal } from "../common/ScrapeSearchModal";
import { StatusMenu } from "../common/StatusMenu";
import { GameTabs } from "./GameTabs";
import {
  ArcadeBoardIcon,
  CardIcon,
  CartridgeIcon,
  CassetteIcon,
  ChartIcon,
  ClockIcon,
  CpuIcon,
  DiscIcon,
  ExpandIcon,
  FloppyIcon,
  GamepadIcon,
  HistoryIcon,
  ImageIcon,
  KeyIcon,
  MemoryIcon,
  MonitorIcon,
  PaletteIcon,
  PencilIcon,
  SpeakerIcon,
  StarIcon,
  StarOutlineIcon,
  TextIcon,
  TrashIcon,
  TrophyIcon,
  VideoIcon,
} from "../common/Icons";

/**
 * Ficha fija de la derecha (estilo LaunchBox): el juego seleccionado en grande, de solo
 * lectura. Para cambiar datos está «Editar», que abre el editor de siempre.
 * Sin juego seleccionado enseña un resumen del sistema.
 */
export function GameInfoPanel() {
  const game = useAppStore((s) => s.selectedGame);
  const collapsed = useAppStore((s) => s.infoPanelCollapsed);
  const toggleInfoPanel = useAppStore((s) => s.toggleInfoPanel);
  const { t } = useTranslation();

  if (collapsed) {
    return (
      <aside className="w-9 shrink-0 border-l border-zinc-800 bg-zinc-950 flex flex-col items-center pt-3">
        <button
          onClick={toggleInfoPanel}
          title={t("info.expand")}
          className="text-zinc-500 hover:text-zinc-200 text-lg leading-none"
        >
          ‹
        </button>
      </aside>
    );
  }

  return (
    <aside className="w-[420px] shrink-0 border-l border-zinc-800 bg-zinc-950 flex flex-col overflow-hidden relative">
      <button
        onClick={toggleInfoPanel}
        title={t("info.collapse")}
        className="absolute top-3 left-3 z-20 w-7 h-7 rounded-full bg-black/50 text-zinc-300 hover:text-white hover:bg-black/70 text-lg leading-none"
      >
        ›
      </button>
      <div className="flex-1 overflow-y-auto">{game ? <GameInfo game={game} /> : <SystemSummary />}</div>
    </aside>
  );
}

function GameInfo({ game }: { game: Game }) {
  const { t } = useTranslation();
  const openEditor = useAppStore((s) => s.openEditor);
  const openFullView = useAppStore((s) => s.openFullView);
  const toggleGameFlag = useAppStore((s) => s.toggleGameFlag);
  const isFavorite = game.favorite === "true";
  const { scrape, remove, scrapeTarget, closeScrape, applyScraped } = useGameActions();
  const { systemPath, markFailed, media, shown, setCurrent, cover, hero, heroStyle, details } = useGameDetails(game);

  return (
    <div>
      {/* Cabecera */}
      <div className="relative h-52 overflow-hidden bg-zinc-900">
        {hero && (
          <img
            src={hero}
            alt=""
            onError={() => markFailed(hero)}
            // Capturas: ampliadas sin suavizar (píxeles nítidos). Carátula de relleno: muy desenfocada
            className={`absolute inset-0 w-full h-full object-cover ${heroStyle}`}
          />
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/10 to-transparent" />
        <div className="absolute top-3 right-3 flex items-center gap-2">
          <button
            onClick={openFullView}
            title={t("fullView.open")}
            className="p-1.5 rounded-md bg-black/60 text-zinc-300 hover:text-white hover:bg-black/80"
          >
            <ExpandIcon size={14} />
          </button>
          {game.rating && (
            <span className="px-2 py-1 rounded-md bg-black/60 text-xs text-yellow-400 font-semibold">
              {formatRating(game.rating)}
            </span>
          )}
          {game.favorite === "true" && (
            <span className="p-1.5 rounded-md bg-black/60 text-yellow-400">
              <StarIcon size={14} />
            </span>
          )}
        </div>
      </div>

      {/* Carátula montada sobre la cabecera (como una caja apoyada en la escena) y el título al lado */}
      <div className="relative px-5 flex items-end gap-4">
        {cover && (
          <img
            src={cover}
            alt=""
            onError={() => markFailed(cover)}
            // Tamaño según su forma: alta (Master System) o apaisada (SNES PAL) sin deformarse
            className="-mt-24 w-auto h-auto max-h-44 max-w-[180px] shrink-0 rounded-sm shadow-2xl shadow-black/70 ring-1 ring-black/50"
          />
        )}
        <div className={`min-w-0 pb-1 ${cover ? "" : "-mt-14"}`}>
          <h2 className="text-xl font-bold text-white leading-tight line-clamp-3 drop-shadow">{game.name || game.path}</h2>
        </div>
      </div>

      {/* Acciones */}
      <div className="px-5 pt-3 flex gap-2">
        <button
          onClick={openEditor}
          className="flex-1 flex items-center justify-center gap-2 px-4 py-2 text-sm font-semibold bg-indigo-600 text-white rounded-md hover:bg-indigo-500 transition-colors"
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
        {/* Favorito: se guarda en el acto */}
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

      {/* Las mismas pestañas que la ficha a pantalla completa, en su propio recuadro */}
      <section className="mx-5 mt-4 p-3 rounded-lg border border-zinc-800 bg-zinc-900/60">
        <GameTabs game={game} systemPath={systemPath} media={media} shown={shown} onPick={setCurrent} onFailed={markFailed} compact />
      </section>

      {/* Información */}
      <div className="px-5 py-4">
        <h3 className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-2">{t("info.information")}</h3>
        <dl className="grid grid-cols-2 gap-x-4 gap-y-3">
          {details
            .filter(([, value]) => !!value)
            .map(([label, value]) => (
              <div key={label} className="min-w-0">
                <dt className="text-[11px] text-zinc-500">{t(label)}</dt>
                <dd className="text-sm text-zinc-200 truncate" title={value}>{value}</dd>
              </div>
            ))}
        </dl>
        {game.desc && <p className="mt-4 text-sm leading-relaxed text-zinc-400 whitespace-pre-line">{game.desc}</p>}
        <p className="mt-4 text-[11px] text-zinc-600 font-mono break-all">{game.path}</p>
      </div>
      {/* Scrapear = búsqueda manual; lo elegido se guarda en el acto */}
      {scrapeTarget && (
        <ScrapeSearchModal systemPath={systemPath} game={scrapeTarget} onClose={closeScrape} onApplied={applyScraped} />
      )}
    </div>
  );
}

type Icon = typeof CpuIcon;

// Campos de la ficha técnica, en este orden, con su icono (el de Soporte depende del dato: mediaIcon)
const SPECS: { key: keyof SystemInfoEntry; label: TranslationKey; icon: Icon }[] = [
  { key: "cpu", label: "systemInfo.cpu", icon: CpuIcon },
  { key: "memory", label: "systemInfo.memory", icon: MemoryIcon },
  { key: "resolution", label: "systemInfo.resolution", icon: MonitorIcon },
  { key: "colors", label: "systemInfo.colors", icon: PaletteIcon },
  { key: "sound", label: "systemInfo.sound", icon: SpeakerIcon },
  { key: "media", label: "systemInfo.media", icon: CartridgeIcon },
  { key: "unitsSold", label: "systemInfo.unitsSold", icon: ChartIcon },
];

// Palabras del soporte → icono. El orden importa: «Placa arcade + CD-ROM» es una placa,
// «Cartucho (Game Card)» un cartucho y «Disco Quick Disk» un disquete, no un disco óptico.
const MEDIA_ICONS: [RegExp, Icon][] = [
  [/placa|arcade|board/, ArcadeBoardIcon],
  [/cartucho|cartridge/, CartridgeIcon],
  [/hucard|card|tarjeta/, CardIcon],
  [/casete|cassette|tape/, CassetteIcon],
  [/disquete|floppy|diskette|quick disk/, FloppyIcon],
  [/cd|dvd|gd-rom|blu-ray|umd|disco|disc|optical/, DiscIcon],
];

/** Icono del soporte: se mira solo el primero («Casete / disquete» → casete). Sin coincidencia, un chip */
function mediaIcon(value: string): Icon {
  const first = value.split("/")[0].toLowerCase();
  return MEDIA_ICONS.find(([re]) => re.test(first))?.[1] ?? CpuIcon;
}

/** Un dato de la ficha: icono violeta en su casilla y, al lado, la etiqueta y el valor */
function SpecItem({ icon: IconComponent, label, value }: { icon: Icon; label: string; value: string }) {
  return (
    <div className="flex gap-2.5 min-w-0">
      <div className="shrink-0 w-7 h-7 rounded-md bg-indigo-500/15 text-indigo-300 grid place-items-center">
        <IconComponent size={15} />
      </div>
      <div className="min-w-0">
        <dt className="text-[11px] text-zinc-500">{label}</dt>
        <dd className="text-sm text-zinc-200">{value}</dd>
      </div>
    </div>
  );
}

/** Sin juego seleccionado: ficha del sistema (si la hay en src/data/systemInfo.json) y cifras */
function SystemSummary() {
  const { t, locale } = useTranslation();
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const games = useAppStore((s) => s.games);
  if (!selectedSystem) return null;

  const info = getSystemInfo(selectedSystem.name);
  // «Sega · 1985 · Consola · 3.ª generación»
  const headline = info
    ? [
        localize(info.manufacturer, locale),
        info.year ?? undefined,
        info.type ? t(`systemInfo.type_${info.type}` as TranslationKey) : undefined,
        info.generation ? t("systemInfo.generation", { n: info.generation }) : undefined,
      ]
        .filter(Boolean)
        .join(" · ")
    : "";
  const specs = info
    ? SPECS.map((s) => {
        const value = localize(info[s.key] as never, locale);
        return { label: s.label, value, icon: s.key === "media" && value ? mediaIcon(value) : s.icon };
      }).filter((s) => !!s.value)
    : [];

  const stats: [TranslationKey, number, Icon][] = [
    ["info.games", games.length, GamepadIcon],
    ["info.withCover", games.filter((g) => g.image).length, ImageIcon],
    ["info.withDescription", games.filter((g) => g.desc).length, TextIcon],
    ["info.withVideo", games.filter((g) => g.video).length, VideoIcon],
    ["info.favorites", games.filter((g) => g.favorite === "true").length, StarOutlineIcon],
  ];

  return (
    <div className="pb-6">
      {/* Cabecera: el logo en grande sobre el degradado de colores del sistema, fundido a oscuro por abajo */}
      <div className="relative overflow-hidden bg-zinc-900" style={{ backgroundImage: systemGradient(selectedSystem.name) }}>
        <div className="absolute inset-0 bg-gradient-to-t from-zinc-950 via-transparent to-transparent" />
        <div className="relative px-8 pt-12 pb-5 flex flex-col items-center">
          <div className="h-[110px] flex items-center justify-center">
            <SystemLogo
              name={selectedSystem.name}
              width={300}
              maxHeight={110}
              className="drop-shadow-[0_2px_8px_rgba(0,0,0,0.5)]"
            />
          </div>
          <h2 className="mt-5 text-xl font-bold text-zinc-100 text-center">{selectedSystem.full_name}</h2>
          {headline && <p className="mt-1 text-xs text-indigo-300 text-center">{headline}</p>}
          <p className="mt-1 text-[11px] text-zinc-500 text-center">{t("info.pickGame")}</p>
        </div>
      </div>

      <div className="px-6">
        {info?.description && (
          <p className="mt-4 text-sm leading-relaxed text-zinc-400">{locale === "es" ? info.description.es : info.description.en}</p>
        )}

        {specs.length > 0 && (
          <>
            <h3 className="mt-6 text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-2">{t("systemInfo.specs")}</h3>
            <dl className="grid grid-cols-2 gap-x-4 gap-y-3">
              {specs.map((s) => (
                <SpecItem key={s.label} icon={s.icon} label={t(s.label)} value={s.value!} />
              ))}
            </dl>
          </>
        )}

        <BiosLine system={selectedSystem.name} />

        <ActivityBlock games={games} />

        {/* Cifras de la biblioteca: compactas, una fila de casillas */}
        <h3 className="mt-6 text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-2">{t("systemInfo.library")}</h3>
        <dl className="grid grid-cols-5 gap-1.5">
          {stats.map(([label, value, StatIcon]) => (
            <div key={label} className="flex flex-col-reverse items-center justify-end gap-0.5 px-1 py-2 rounded-md bg-zinc-900 text-center">
              <dt className="text-[10px] leading-tight text-zinc-500">{t(label)}</dt>
              <dd className="text-base font-semibold text-zinc-100">{value}</dd>
              <StatIcon size={13} className="text-zinc-500 mb-0.5" />
            </div>
          ))}
        </dl>

        <SystemLocationsBlock systemPath={selectedSystem.path} />
      </div>
    </div>
  );
}

/** Actividad del sistema: tiempo total, cuántos se han jugado y los más jugados / recientes */
function ActivityBlock({ games }: { games: Game[] }) {
  const { t } = useTranslation();
  const selectGame = useAppStore((s) => s.selectGame);
  const played = games.filter(isPlayed);
  if (played.length === 0) return null;
  const totalSeconds = played.reduce((sum, g) => sum + (Number(g.gametime ?? 0) || 0), 0);
  const top = [...played]
    .filter((g) => Number(g.playcount ?? 0) > 0)
    .sort((a, b) => Number(b.playcount) - Number(a.playcount))
    .slice(0, 5);
  const recent = [...played]
    .filter((g) => g.lastplayed)
    .sort((a, b) => (b.lastplayed ?? "").localeCompare(a.lastplayed ?? ""))
    .slice(0, 5);
  const list = (title: TranslationKey, TitleIcon: Icon, items: Game[], value: (g: Game) => string) =>
    items.length > 0 && (
      <div className="min-w-0">
        <p className="flex items-center gap-1.5 text-[11px] text-zinc-500 mb-1">
          <TitleIcon size={12} />
          {t(title)}
        </p>
        {items.map((g) => (
          <button key={g.path} onClick={() => selectGame(g)} className="w-full flex items-center gap-2 text-left text-xs py-0.5 hover:text-indigo-300">
            <span className="flex-1 truncate text-zinc-300">{g.name || g.path}</span>
            <span className="shrink-0 text-zinc-500">{value(g)}</span>
          </button>
        ))}
      </div>
    );
  return (
    <>
      <h3 className="mt-6 text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-2">{t("activity.title")}</h3>
      {/* Las dos cifras grandes: cuántos se han jugado y el tiempo total (si se sabe) */}
      <div className="grid grid-cols-2 gap-2 mb-4">
        <ActivityTile icon={GamepadIcon} value={`${played.length} / ${games.length}`} label={t("activity.playedLabel")} />
        {totalSeconds > 0 && (
          <ActivityTile icon={ClockIcon} value={formatPlayTime(String(totalSeconds))} label={t("activity.totalLabel")} />
        )}
      </div>
      <div className="grid grid-cols-2 gap-4">
        {list("activity.mostPlayed", TrophyIcon, top, (g) => t("activity.times", { count: g.playcount ?? 0 }))}
        {list("activity.recent", HistoryIcon, recent, (g) => formatReleaseDate(g.lastplayed))}
      </div>
    </>
  );
}

/** Casilla destacada de Actividad: icono violeta, cifra grande y qué es debajo */
function ActivityTile({ icon: IconComponent, value, label }: { icon: Icon; value: string; label: string }) {
  return (
    <div className="flex items-center gap-2.5 px-3 py-2 rounded-md bg-zinc-900 min-w-0">
      <IconComponent size={18} className="shrink-0 text-indigo-300" />
      <div className="min-w-0">
        <p className="text-base font-semibold text-zinc-100 leading-tight truncate">{value}</p>
        <p className="text-[10px] text-zinc-500">{label}</p>
      </div>
    </div>
  );
}

/** «BIOS: N de M» del sistema (solo si la lista de BIOS está descargada y el sistema tiene alguna) */
function BiosLine({ system }: { system: string }) {
  const { t } = useTranslation();
  const [count, setCount] = useState<{ ok: number; total: number } | null>(null);
  useEffect(() => {
    let cancelled = false;
    checkBios([system])
      .then((r) => !cancelled && setCount(r?.systems[0] ? { ok: r.systems[0].ok, total: r.systems[0].total } : null))
      .catch(() => !cancelled && setCount(null));
    return () => {
      cancelled = true;
    };
  }, [system]);
  if (!count) return null;
  const color = count.ok === count.total ? "text-emerald-400" : count.ok > 0 ? "text-amber-300" : "text-zinc-400";
  return (
    <p className={`mt-4 flex items-center gap-1.5 text-xs ${color}`} title={t("bios.lineHelp")}>
      <KeyIcon size={13} />
      {t("bios.line", { ok: count.ok, total: count.total })}
    </p>
  );
}

/** Dónde lee y escribe la app los datos de este sistema con el perfil activo (solo informativo) */
function SystemLocationsBlock({ systemPath }: { systemPath: string }) {
  const { t } = useTranslation();
  const games = useAppStore((s) => s.games);
  const [loc, setLoc] = useState<SystemLocations | null>(null);

  // Se vuelve a pedir al cambiar los juegos (p. ej. tras generar la gamelist o scrapear)
  useEffect(() => {
    let cancelled = false;
    systemLocations(systemPath)
      .then((l) => !cancelled && setLoc(l))
      .catch(() => !cancelled && setLoc(null));
    return () => {
      cancelled = true;
    };
  }, [systemPath, games]);

  if (!loc) return null;

  // Una ruta: etiqueta y la ruta seleccionable (para copiarla)
  const row = (label: string, path: string, note?: string) => (
    <div className="min-w-0">
      <dt className="text-[11px] text-zinc-500">
        {label}
        {note && <span className="ml-1 text-amber-400">· {note}</span>}
      </dt>
      <dd className="text-[11px] leading-snug text-zinc-300 font-mono break-all select-text">{path}</dd>
    </div>
  );

  return (
    // Agrupado en una tarjeta, como una sección aparte de la ficha
    <section className="mt-6 rounded-lg border border-zinc-800 bg-zinc-900/60 overflow-hidden">
      <h3 className="px-4 py-2.5 flex items-center justify-between border-b border-zinc-800 text-[11px] uppercase tracking-wider text-zinc-500 font-semibold">
        {t("systemInfo.locations")}
        <span className="px-1.5 py-0.5 rounded bg-zinc-800 normal-case tracking-normal text-zinc-300">
          {loc.structure === "esde" ? "ES-DE" : t("profiles.kindClassic")}
        </span>
      </h3>
      <dl className="px-4 py-3 space-y-3">
        {row(t("systemInfo.romFolder"), loc.rom_dir)}
        {row(t("systemInfo.gamelist"), loc.gamelist_path, loc.gamelist_exists ? undefined : t("systemInfo.notCreated"))}
        {row(t("systemInfo.mediaFolder"), loc.media_dir)}
        {loc.media_folders.length > 0 && (
          <div>
            <dt className="text-[11px] text-zinc-500">{t("systemInfo.mediaFolders")}</dt>
            <dd className="mt-1 flex flex-wrap gap-1.5">
              {loc.media_folders.map((f) => (
                <span key={f.folder} title={f.path} className="px-2 py-0.5 rounded bg-zinc-800 text-xs text-zinc-300">
                  {f.folder} <span className="text-zinc-500">{f.files}</span>
                </span>
              ))}
            </dd>
          </div>
        )}
        {row(t("systemInfo.backups"), loc.backups_dir)}
        {row(t("systemInfo.collections"), loc.collections_dir)}
      </dl>
    </section>
  );
}
