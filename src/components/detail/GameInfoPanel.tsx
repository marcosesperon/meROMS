import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { TranslationKeys as TranslationKey } from "../../i18n/en";
import type { Game } from "../../types";
import { resolveImageSrc } from "../../utils/images";
import { formatRating, formatReleaseDate } from "../../utils/format";
import { getFlag } from "../../utils/flags";
import { useGameActions } from "../../hooks/useGameActions";
import { SystemLogo } from "../common/SystemLogo";
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
  const { scrape, remove, scraping } = useGameActions();
  const systemPath = selectedSystem?.path || "";
  const src = (value?: string) => resolveImageSrc(value, systemPath);

  // Lo que hay de verdad para la galería
  const media = useMemo(
    () => GALLERY.filter((m) => !!game[m.field]).map((m) => ({ ...m, url: src(game[m.field] as string)! })),
    [game, systemPath]
  );
  const [current, setCurrent] = useState(0);
  // Al cambiar de juego, la galería vuelve al principio
  useEffect(() => setCurrent(0), [game.path]);
  const shown = media[current] ?? media[0];

  // Cabecera: fanart; si no hay, una captura (desenfocada para que no compita con el título)
  const hero = src(game.fanart) ?? src(game.thumbnail) ?? src(game.titleshot) ?? src(game.image);
  const heroIsFanart = !!game.fanart;

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
            className={`absolute inset-0 w-full h-full object-cover ${heroIsFanart ? "" : "blur-sm scale-110 opacity-60"}`}
          />
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/40 to-transparent" />
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
        <div className="absolute bottom-3 left-5 right-5">
          <p className="text-[11px] uppercase tracking-wider text-indigo-300 font-semibold">{selectedSystem?.full_name}</p>
          <h2 className="text-2xl font-bold text-white leading-tight line-clamp-2 drop-shadow">{game.name || game.path}</h2>
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
          disabled={scraping}
          className="px-4 py-2 text-sm bg-zinc-800 text-emerald-400 rounded-md hover:bg-zinc-700 transition-colors disabled:opacity-50"
        >
          {scraping ? t("detail.scraping") : t("detail.scrape")}
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
      {shown && (
        <div className="px-5 pt-4">
          <div className="aspect-video bg-black rounded-lg overflow-hidden flex items-center justify-center">
            {shown.field === "video" ? (
              <video key={shown.url} src={shown.url} controls autoPlay muted loop className="w-full h-full object-contain" />
            ) : (
              <img src={shown.url} alt={t(shown.label)} className="w-full h-full object-contain" />
            )}
          </div>
          {media.length > 1 && (
            <div className="flex gap-2 mt-2 overflow-x-auto pb-1">
              {media.map((m, i) => (
                <button
                  key={m.field}
                  onClick={() => setCurrent(i)}
                  title={t(m.label)}
                  className={`w-16 h-11 shrink-0 rounded-md overflow-hidden bg-zinc-800 ring-2 transition-colors ${
                    m === shown ? "ring-indigo-500" : "ring-transparent hover:ring-zinc-600"
                  }`}
                >
                  {m.field === "video" ? (
                    <span className="w-full h-full flex items-center justify-center text-zinc-300 text-lg">▶</span>
                  ) : (
                    <img src={m.url} alt="" className="w-full h-full object-cover" />
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
    </div>
  );
}

/** Sin juego seleccionado: unos datos del sistema */
function SystemSummary() {
  const { t } = useTranslation();
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const games = useAppStore((s) => s.games);
  if (!selectedSystem) return null;

  const stats: [TranslationKey, number][] = [
    ["info.games", games.length],
    ["info.withCover", games.filter((g) => g.image).length],
    ["info.withDescription", games.filter((g) => g.desc).length],
    ["info.withVideo", games.filter((g) => g.video).length],
    ["info.favorites", games.filter((g) => g.favorite === "true").length],
  ];

  return (
    <div className="px-6 pt-16 pb-6">
      <div className="flex justify-center">
        <SystemLogo name={selectedSystem.name} width={200} maxHeight={80} />
      </div>
      <h2 className="mt-6 text-xl font-bold text-zinc-100 text-center">{selectedSystem.full_name}</h2>
      <p className="mt-1 text-xs text-zinc-500 text-center">{t("info.pickGame")}</p>
      <dl className="mt-8 space-y-2">
        {stats.map(([label, value]) => (
          <div key={label} className="flex items-center justify-between px-4 py-2.5 rounded-md bg-zinc-900">
            <dt className="text-sm text-zinc-400">{t(label)}</dt>
            <dd className="text-sm font-semibold text-zinc-100">{value}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
