import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "../store/useAppStore";
import type { TranslationKeys as TranslationKey } from "../i18n/en";
import type { Game } from "../types";
import { resolveImageSrc } from "../utils/images";
import { formatRating, formatReleaseDate } from "../utils/format";
import { formatPlayTime } from "../utils/playStats";
import { getFlag } from "../utils/flags";
import { statusText } from "../utils/gameStatus";
import { useTranslation } from "../i18n";
import { getSystemView } from "../utils/systemView";

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

export type GalleryItem = { field: keyof Game; label: TranslationKey; url: string };

/**
 * Lo que enseñan las fichas de lectura de un juego (el panel de la derecha y la ficha a pantalla
 * completa): medios que de verdad se pueden ver, carátula, imagen de fondo y datos.
 */
export function useGameDetails(game: Game) {
  const selectedSystem = useAppStore((s) => s.selectedSystem);
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
  const media: GalleryItem[] = useMemo(
    () =>
      GALLERY.flatMap((m) => {
        const url = src(game[m.field] as string | undefined);
        return url ? [{ ...m, url }] : [];
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [game, systemPath, failed]
  );
  const [current, setCurrent] = useState(0);
  // Al cambiar de juego, la galería vuelve al principio
  useEffect(() => setCurrent(0), [game.path]);
  const shown = media[current] ?? media[0];
  // Lo que ya se ve arriba (la carátula y la imagen de fondo de la cabecera): si no hay nada
  // más, el visor solo repetiría lo mismo y no se enseña
  const firstShown = (fields: (keyof Game)[]) => fields.find((f) => !!src(game[f] as string | undefined)) ?? null;
  // Portada y fondo: primero el medio elegido para el sistema en el perfil (su ventanita de
  // ajustes) y, si el juego no lo tiene, el orden de siempre
  // (se leen los dos textos sueltos: un objeto nuevo en cada lectura del store repintaría sin fin)
  const chosenCover = useAppStore((s) => getSystemView(s.settings, s.selectedSystem?.name).cover);
  const chosenHero = useAppStore((s) => getSystemView(s.settings, s.selectedSystem?.name).hero);
  const chosen = (field: string | undefined) => (field ? [field as keyof Game] : []);
  const coverField = firstShown([...chosen(chosenCover), "image", "boxart"]);
  const sceneField = firstShown([...chosen(chosenHero), "fanart", "thumbnail", "titleshot"]);
  const onlyShownAbove = media.every((m) => m.field === coverField || m.field === sceneField);

  // Cabecera: fanart; si no hay, una captura (la portada ya va aparte, delante).
  // Sin ninguna de las dos, la propia carátula muy desenfocada: deja un fondo con sus colores.
  const cover = coverField ? src(game[coverField] as string) : undefined;
  const scene = sceneField ? src(game[sceneField] as string) : undefined;
  const hero = scene ?? cover;
  // Capturas y pantallas de título, con píxeles nítidos; sin fondo, la portada difuminada
  const heroStyle = !scene ? "blur-2xl saturate-150 scale-125 opacity-70" : sceneField === "thumbnail" || sceneField === "titleshot" ? "[image-rendering:pixelated]" : "";

  // Estado de juego (meROMS): «Terminado · Jugando»
  const { t } = useTranslation();
  const status = useAppStore((s) => s.statuses.get(game.path));

  const details: [TranslationKey, string | undefined][] = [
    ["status.column", statusText(status, t) || undefined],
    ["form.releaseDate", formatReleaseDate(game.releasedate)],
    ["form.developer", game.developer],
    ["form.publisher", game.publisher],
    ["form.genre", game.genre],
    ["form.players", game.players],
    ["form.rating", formatRating(game.rating)],
    ["form.region", [getFlag(game.region || game.lang), game.region || game.lang].filter(Boolean).join(" ")],
    // Estadísticas de juego (las escribe el frontend al jugar); vacías no salen
    ["exportDevice.playcount", Number(game.playcount ?? 0) > 0 ? game.playcount : undefined],
    ["exportDevice.gametime", formatPlayTime(game.gametime) || undefined],
    ["exportDevice.lastplayed", formatReleaseDate(game.lastplayed) || undefined],
  ];

  return { systemPath, markFailed, media, shown, setCurrent, onlyShownAbove, cover, hero, heroStyle, details };
}
