import type { Game } from "../types";
import type { TranslationKeys } from "../i18n/en";

/** Tipos de media del filtro «Le falta»: campo de la gamelist y su texto (mismo orden que media.rs) */
export const MEDIA_FILTER_FIELDS: { field: keyof Game; label: TranslationKeys; classicOnly?: boolean }[] = [
  { field: "image", label: "filter.media.image" },
  { field: "thumbnail", label: "filter.media.thumbnail" },
  { field: "video", label: "filter.media.video" },
  { field: "marquee", label: "filter.media.marquee" },
  { field: "titleshot", label: "filter.media.titleshot" },
  { field: "boxback", label: "filter.media.boxback" },
  { field: "boxart", label: "filter.media.boxart" },
  { field: "cartridge", label: "filter.media.cartridge" },
  { field: "fanart", label: "filter.media.fanart" },
  { field: "mix", label: "filter.media.mix" },
  { field: "manual", label: "filter.media.manual" },
  // ES-DE no tiene carpeta de wheels
  { field: "wheel", label: "filter.media.wheel", classicOnly: true },
];

/** Valor del filtro para «juegos sin ninguna media» */
export const NO_MEDIA = "any";

/** Clave de una ruta rota en el conjunto `brokenMedia` del store (con la ruta: si cambia, ya no cuenta) */
export const brokenKey = (gamePath: string, field: string, value: string) => `${gamePath}\n${field}\n${value}`;

/** El juego tiene ese tipo de media: el campo está relleno y no apunta a un fichero que ya no existe */
export function hasMedia(game: Game, field: keyof Game, broken: Set<string>): boolean {
  const value = game[field];
  if (typeof value !== "string" || !value.trim()) return false;
  return !broken.has(brokenKey(game.path, field, value));
}

/** El juego cumple el filtro «Le falta»: no tiene ese tipo (o, con NO_MEDIA, no tiene ninguno) */
export function lacksMedia(game: Game, missing: string, broken: Set<string>): boolean {
  if (missing === NO_MEDIA) return MEDIA_FILTER_FIELDS.every(({ field }) => !hasMedia(game, field, broken));
  return !hasMedia(game, missing as keyof Game, broken);
}
