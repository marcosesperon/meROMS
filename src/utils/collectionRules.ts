import type { CollectionRules, Game } from "../types";
import type { TranslationKeys } from "../i18n/en";
import { MEDIA_FILTER_FIELDS } from "./mediaFilter";

/**
 * Lo que debe cumplir la colección (Ajustes → Mi colección): datos y medios que todo juego
 * debería tener y qué hacer con las versiones que sobran y con los medios sobrantes o dañados.
 * Lo usan el estado del sistema y «Ponerlo todo en orden».
 */

/** Datos que se pueden pedir a cada juego, con su texto del formulario */
export const DATA_FIELDS: { field: keyof Game; label: TranslationKeys }[] = [
  { field: "desc", label: "form.description" },
  { field: "releasedate", label: "form.releaseDate" },
  { field: "genre", label: "form.genre" },
  { field: "developer", label: "form.developer" },
  { field: "publisher", label: "form.publisher" },
  { field: "players", label: "form.players" },
  { field: "rating", label: "form.rating" },
];

/** Medios que se pueden pedir: los mismos que el filtro «Le falta…» */
export const MEDIA_FIELDS = MEDIA_FILTER_FIELDS;

/** De serie: descripción, fecha, género y desarrollador/editor; carátula, captura y vídeo; apartar */
export const DEFAULT_COLLECTION_RULES: CollectionRules = {
  required_data: ["desc", "releasedate", "genre", "developer", "publisher"],
  required_media: ["image", "thumbnail", "video"],
  versions_action: "set_aside",
  protect_played: true,
  leftover_media_action: "trash",
};

/** Las reglas guardadas, completadas con las de serie (y sin campos que ya no existan) */
export function collectionRulesFrom(saved: Partial<CollectionRules> | undefined): CollectionRules {
  const rules = { ...DEFAULT_COLLECTION_RULES, ...saved };
  const known = <T extends { field: keyof Game }>(list: T[]) => new Set(list.map((f) => f.field as string));
  const data = known(DATA_FIELDS);
  const media = known(MEDIA_FIELDS);
  return {
    ...rules,
    required_data: rules.required_data.filter((f) => data.has(f)),
    required_media: rules.required_media.filter((f) => media.has(f)),
  };
}
