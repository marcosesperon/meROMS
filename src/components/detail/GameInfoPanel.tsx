import { useEffect, useMemo, useState } from "react";
import { checkBios, systemLocations, type SystemLocations } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { TranslationKeys as TranslationKey } from "../../i18n/en";
import type { Game } from "../../types";
import { resolveImageSrc } from "../../utils/images";
import { formatRating, formatReleaseDate } from "../../utils/format";
import { getFlag } from "../../utils/flags";
import { useGameActions } from "../../hooks/useGameActions";
import { getSystemInfo, localize, type SystemInfoEntry } from "../../utils/systemInfo";
import { SystemLogo } from "../common/SystemLogo";
import { ScrapeSearchModal } from "../common/ScrapeSearchModal";
import { PencilIcon, StarIcon, StarOutlineIcon, TrashIcon } from "../common/Icons";

// Medios que salen en la galería, en este orden (el vídeo va primero si hay)
const GALLERY: { field: keyof Game; label: TranslationKey }[] = [
  { field: "video", label: "images.video" },
  { field: "thumbnail", label: "info.screenshot" },
  { field: "titleshot", label: "images.titleshot" },
  { field: "image", label: "images.boxArt" },
  { field: "boxart", label: "images.boxart" },
  { field: "boxback", label: "images.boxback" },
  { field: "mix", label: "images.mix" },
  { field: "marquee", label: "images.marquee" },
  { field: "cartridge", label: "images.cartridge" },
  { field: "fanart", label: "images.fanart" },
];

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
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const openEditor = useAppStore((s) => s.openEditor);
  const toggleGameFlag = useAppStore((s) => s.toggleGameFlag);
  const isFavorite = game.favorite === "true";
  const { scrape, remove, scrapeTarget, closeScrape, applyScraped } = useGameActions();
  const systemPath = selectedSystem?.path || "";
  // Medios que no se han podido cargar (la gamelist apunta a un fichero que no existe):
  // no se pintan, para no dejar el icono de imagen rota
  const [failed, setFailed] = useState<Set<string>>(new Set());
  useEffect(() => setFailed(new Set()), [game.path]);
  const markFailed = (url: string) => setFailed((prev) => new Set(prev).add(url));
  const src = (value?: string) => {
    const url = resolveImageSrc(value, systemPath);
    return url && !failed.has(url) ? url : undefined;
  };

  // Lo que hay de verdad para la galería
  const media = useMemo(
    () =>
      GALLERY.flatMap((m) => {
        const url = src(game[m.field] as string | undefined);
        return url ? [{ ...m, url }] : [];
      }),
    [game, systemPath, failed]
  );
  const [current, setCurrent] = useState(0);
  // Al cambiar de juego, la galería vuelve al principio
  useEffect(() => setCurrent(0), [game.path]);
  const shown = media[current] ?? media[0];
  // Lo que ya se ve arriba (la carátula y la imagen de fondo de la cabecera): si no hay nada
  // más, el visor solo repetiría lo mismo y no se enseña
  const firstShown = (fields: (keyof Game)[]) => fields.find((f) => !!src(game[f] as string | undefined)) ?? null;
  const coverField = firstShown(["image", "boxart"]);
  const sceneField = firstShown(["fanart", "thumbnail", "titleshot"]);
  const onlyShownAbove = media.every((m) => m.field === coverField || m.field === sceneField);

  // Cabecera: fanart; si no hay, una captura (la portada ya va aparte, delante).
  // Sin ninguna de las dos, la propia carátula muy desenfocada: deja un fondo con sus colores.
  const cover = coverField ? src(game[coverField] as string) : undefined;
  const scene = sceneField ? src(game[sceneField] as string) : undefined;
  const hero = scene ?? cover;
  const heroStyle = sceneField === "fanart" ? "" : scene ? "[image-rendering:pixelated]" : "blur-2xl saturate-150 scale-125 opacity-70";

  const details: [TranslationKey, string | undefined][] = [
    ["form.releaseDate", formatReleaseDate(game.releasedate)],
    ["form.developer", game.developer],
    ["form.publisher", game.publisher],
    ["form.genre", game.genre],
    ["form.players", game.players],
    ["form.rating", formatRating(game.rating)],
    ["form.region", [getFlag(game.region || game.lang), game.region || game.lang].filter(Boolean).join(" ")],
  ];

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
        <button
          onClick={() => remove(game)}
          title={t("detail.delete")}
          className="px-3 py-2 text-sm bg-zinc-800 text-red-400 rounded-md hover:bg-zinc-700 transition-colors"
        >
          <TrashIcon size={14} />
        </button>
      </div>

      {/* Visor + galería */}
      {shown && !onlyShownAbove && (
        <div className="px-5 pt-4">
          <div className="aspect-video bg-black rounded-lg overflow-hidden flex items-center justify-center">
            {shown.field === "video" ? (
              <video key={shown.url} src={shown.url} onError={() => markFailed(shown.url)} controls autoPlay muted loop className="w-full h-full object-contain" />
            ) : (
              <img src={shown.url} alt={t(shown.label)} onError={() => markFailed(shown.url)} className="w-full h-full object-contain" />
            )}
          </div>
          {media.length > 1 && (
            <div className="flex gap-2 mt-2 overflow-x-auto pb-1">
              {media.map((m, i) => (
                <button
                  key={m.field}
                  onClick={() => setCurrent(i)}
                  title={t(m.label)}
                  // Sin borde: la elegida se ve entera y el resto algo apagadas
                  className={`w-16 h-11 shrink-0 rounded-md overflow-hidden bg-zinc-800 transition-opacity ${
                    m === shown ? "opacity-100" : "opacity-50 hover:opacity-80"
                  }`}
                >
                  {m.field === "video" ? (
                    <span className="w-full h-full flex items-center justify-center text-zinc-300 text-lg">▶</span>
                  ) : (
                    <img src={m.url} alt="" onError={() => markFailed(m.url)} className="w-full h-full object-cover" />
                  )}
                </button>
              ))}
            </div>
          )}
        </div>
      )}

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

// Campos de la ficha técnica, en este orden
const SPECS: { key: keyof SystemInfoEntry; label: TranslationKey }[] = [
  { key: "cpu", label: "systemInfo.cpu" },
  { key: "memory", label: "systemInfo.memory" },
  { key: "resolution", label: "systemInfo.resolution" },
  { key: "colors", label: "systemInfo.colors" },
  { key: "sound", label: "systemInfo.sound" },
  { key: "media", label: "systemInfo.media" },
  { key: "unitsSold", label: "systemInfo.unitsSold" },
];

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
  const specs = info ? SPECS.map((s) => [s.label, localize(info[s.key] as never, locale)] as const).filter(([, v]) => !!v) : [];

  const stats: [TranslationKey, number][] = [
    ["info.games", games.length],
    ["info.withCover", games.filter((g) => g.image).length],
    ["info.withDescription", games.filter((g) => g.desc).length],
    ["info.withVideo", games.filter((g) => g.video).length],
    ["info.favorites", games.filter((g) => g.favorite === "true").length],
  ];

  return (
    <div className="px-6 pt-14 pb-6">
      <div className="flex justify-center">
        <SystemLogo name={selectedSystem.name} width={200} maxHeight={80} />
      </div>
      <h2 className="mt-6 text-xl font-bold text-zinc-100 text-center">{selectedSystem.full_name}</h2>
      {headline && <p className="mt-1 text-xs text-indigo-300 text-center">{headline}</p>}
      <p className="mt-1 text-[11px] text-zinc-500 text-center">{t("info.pickGame")}</p>
      {info?.description && (
        <p className="mt-4 text-sm leading-relaxed text-zinc-400">{locale === "es" ? info.description.es : info.description.en}</p>
      )}

      {specs.length > 0 && (
        <>
          <h3 className="mt-6 text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-2">{t("systemInfo.specs")}</h3>
          <dl className="grid grid-cols-2 gap-x-4 gap-y-3">
            {specs.map(([label, value]) => (
              <div key={label} className="min-w-0">
                <dt className="text-[11px] text-zinc-500">{t(label)}</dt>
                <dd className="text-sm text-zinc-200">{value}</dd>
              </div>
            ))}
          </dl>
        </>
      )}

      <BiosLine system={selectedSystem.name} />

      {/* Cifras de la biblioteca: compactas, una fila de casillas */}
      <h3 className="mt-6 text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-2">{t("systemInfo.library")}</h3>
      <dl className="grid grid-cols-5 gap-1.5">
        {stats.map(([label, value]) => (
          <div key={label} className="flex flex-col-reverse items-center justify-end gap-0.5 px-1 py-2 rounded-md bg-zinc-900 text-center">
            <dt className="text-[10px] leading-tight text-zinc-500">{t(label)}</dt>
            <dd className="text-base font-semibold text-zinc-100">{value}</dd>
          </div>
        ))}
      </dl>

      <SystemLocationsBlock systemPath={selectedSystem.path} />
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
    <p className={`mt-4 text-xs ${color}`} title={t("bios.lineHelp")}>
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
