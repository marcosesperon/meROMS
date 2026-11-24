import type { CollectionRules, Game, RenameOptions } from "../types";
import {
  detectSwappedMedia,
  findDamagedMedia,
  findDuplicates,
  findPlaceholderMedia,
  findRepeatedMedia,
  findUnusedMedia,
  planMediaNormalization,
  renamePreview,
  type ScraperStatus,
} from "../commands";
import { collectionRulesFrom } from "./collectionRules";
import { hasMedia, MEDIA_FILTER_FIELDS } from "./mediaFilter";
import { isBadReleaseDate } from "./format";
import { activeSources, planRename, renameOptionsFrom } from "./renameSources";

/**
 * Estado del sistema: qué no cumple lo que pide «Mi colección» (Ajustes). Lo caro (versiones,
 * medios dañados, placeholders, sobrantes, cambiados, con nombre antiguo, nombres fuera de
 * formato) son las comprobaciones de cada herramienta, pedidas a la vez: cada número sale en
 * cuanto llega su parte. El backend guarda el resultado completo de cada una (memoria y disco,
 * `system_status.rs` `remember`), así que «Ver» abre la herramienta al momento y, si nada ha
 * cambiado, entrar en el sistema también es inmediato. Lo barato (datos y medios obligatorios)
 * se calcula aquí con lo que ya está en memoria.
 */

/** Cada línea del estado; también es el valor del filtro «Con problemas de…» de la lista */
export type IssueKind =
  | "newRoms"
  | "missingRoms"
  | "versions"
  | "names"
  | "arrmSort"
  | "data"
  | "invalid"
  | "media"
  | "damaged"
  | "placeholders"
  | "unused"
  | "swapped"
  | "legacy";

/** Nombres fuera de formato: juegos cuyo fichero o cuyo título cambiarían al renombrar */
export interface NamesStatus {
  files: string[];
  titles: string[];
}

/** Cuántos ficheros y de qué juegos */
export interface StatusItems {
  files: number;
  games: string[];
}

/** Lo caro del estado, parte por parte (una parte que no está es que aún no ha llegado) */
export interface StatusParts {
  /** Grupos de versiones del mismo juego (rutas), sin los de discos de un mismo juego */
  versions?: string[][];
  damaged?: StatusItems;
  placeholders?: StatusItems;
  /** Medios que no usa ningún juego (sin los que «Normalizar» recupera) */
  unused?: StatusItems;
  swapped?: StatusItems;
  /** Todo el sistema parece al revés (mejor «Intercambiar tipos») */
  swappedWhole?: boolean;
  legacy?: StatusItems;
}

/** Lo que el store guarda del estado del sistema abierto */
export interface CollectionStatusState {
  system: string;
  parts: StatusParts;
  names: NamesStatus | null;
  /** Comprobándose ahora (los números van llegando) */
  checking: boolean;
  /** Comprobaciones que fallaron (el resto vale igual) */
  errors: string[];
  /** Cuándo terminó la última comprobación (ms) */
  checkedAt: number | null;
}

/** Cuenta ficheros y juegos distintos */
function items(gamePaths: string[]): StatusItems {
  return { files: gamePaths.length, games: [...new Set(gamePaths.filter(Boolean))] };
}

/**
 * Las comprobaciones caras, cada una con lo que aporta al estado. Se lanzan a la vez y cada
 * una se enseña al llegar. «Medios repetidos» no sale en el estado: se pide para que la pestaña
 * «Sobrantes» de la ventana Medios también abra al momento.
 */
export function statusChecks(systemPath: string): { name: string; run: () => Promise<StatusParts> }[] {
  return [
    {
      name: "versions",
      run: () => findDuplicates(systemPath).then((groups) => ({ versions: groups.filter((g) => !g.discs).map((g) => g.games.map((x) => x.path)) })),
    },
    { name: "legacy", run: () => planMediaNormalization(systemPath).then((list) => ({ legacy: items(list.filter((i) => i.status !== "conflict").map((i) => i.game_path)) })) },
    { name: "swapped", run: () => detectSwappedMedia(systemPath).then((r) => ({ swapped: items(r.proposals.map((p) => p.game_path)), swappedWhole: r.looks_swapped })) },
    { name: "damaged", run: () => findDamagedMedia(systemPath).then((list) => ({ damaged: items(list.map((d) => d.game_path)) })) },
    { name: "placeholders", run: () => findPlaceholderMedia(systemPath).then((groups) => ({ placeholders: items(groups.flatMap((g) => g.files.map((f) => f.game_path))) })) },
    // Los que «Normalizar» recupera no sobran (cuentan como nombre antiguo)
    { name: "unused", run: () => findUnusedMedia(systemPath).then((files) => ({ unused: items(files.filter((f) => !f.recoverable).map(() => "")) })) },
    { name: "repeated", run: () => findRepeatedMedia(systemPath).then(() => ({})) },
  ];
}

/** Lo que hace falta para saber qué juegos tienen cada problema */
export interface IssueContext {
  games: Game[];
  rules: CollectionRules;
  broken: Set<string>;
  isEsde: boolean;
  status: CollectionStatusState | null;
  missingRomPaths: Set<string>;
}

/** Medios obligatorios que existen en esta estructura (ES-DE no tiene wheels) */
export function requiredMediaFields(rules: CollectionRules, isEsde: boolean): (keyof Game)[] {
  return MEDIA_FILTER_FIELDS.filter((m) => rules.required_media.includes(m.field as string) && !(isEsde && m.classicOnly)).map((m) => m.field);
}

/**
 * Datos no válidos del scraper: ScreenScraper llama «ZZZ(notgame):…» a las ROMs que conoce pero
 * que no son un juego, y versiones antiguas de meROMS (u otras herramientas) lo guardaron como
 * título. Con «#» detrás («#UNKNOWN GAME», «#NONGAME») no hay nombre que salvar; si no
 * («ZZZ(notgame):Game Boy Camera»), el nombre es bueno y solo sobra el prefijo.
 */
const NOT_GAME = /zzz\(notgame\):\s*/i;
const NOT_GAME_FIELDS: (keyof Game)[] = ["name", "sortname"];

export function hasNotGameData(game: Game): boolean {
  return NOT_GAME_FIELDS.some((f) => NOT_GAME.test(String(game[f] ?? "")));
}

/** Datos no válidos del scraper: el título «no es un juego» o la fecha en otro formato
 *  («1988-07-01», como se guardaba la de ScreenScraper; EmulationStation y ES-DE la leen mal) */
export function hasInvalidData(game: Game): boolean {
  return hasNotGameData(game) || isBadReleaseDate(game.releasedate);
}

/** El juego con esos datos arreglados: la fecha a AAAAMMDDTHHMMSS y el título sin «ZZZ(notgame)» */
export function withoutInvalidData(game: Game): Game {
  const fixed = withoutNotGameData(game);
  if (!isBadReleaseDate(fixed.releasedate)) return fixed;
  const digits = (fixed.releasedate ?? "").replace(/T.*$/, "").replace(/\D/g, "");
  const date = digits.length >= 8 ? `${digits.slice(0, 8)}T000000` : digits.length === 6 ? `${digits}01T000000` : digits.length === 4 ? `${digits}0101T000000` : undefined;
  return { ...fixed, releasedate: date };
}

/** El juego sin esos datos: el prefijo fuera o, si lo que sigue empieza por «#», el campo vacío */
export function withoutNotGameData(game: Game): Game {
  const next = { ...game } as Record<string, unknown>;
  for (const f of NOT_GAME_FIELDS) {
    const value = String(game[f] ?? "");
    const match = value.match(NOT_GAME);
    if (!match) continue;
    const rest = value.slice((match.index ?? 0) + match[0].length).trim();
    // El nombre para ordenar de ARRM («473 =-  ZZZ(notgame):X») no se salva: fuera entero
    if (rest.startsWith("#") || f === "sortname") delete next[f];
    else next[f] = rest;
  }
  return next as unknown as Game;
}

/**
 * Nombre para ordenar de ARRM («228 =-  0501 Future Warrior»): un número para fijar el orden y el
 * nombre que tenía el juego entonces. Tras renombrar ya no vale (ordena el juego como otro). Los
 * puestos a mano (sin «N =-» delante) no cuentan.
 */
const ARRM_SORTNAME = /^\s*\d+\s*=-/;

export function hasArrmSortname(game: Game): boolean {
  return ARRM_SORTNAME.test(game.sortname ?? "");
}

/** Le falta algún dato obligatorio */
export function lacksData(game: Game, rules: CollectionRules): boolean {
  return rules.required_data.some((field) => !String(game[field as keyof Game] ?? "").trim());
}

/** Datos obligatorios que le faltan (para «Revisar este juego») */
export function missingData(game: Game, rules: CollectionRules): string[] {
  return rules.required_data.filter((field) => !String(game[field as keyof Game] ?? "").trim());
}

/** Medios obligatorios que le faltan (campo vacío o que apunta a un fichero que no existe) */
export function missingMedia(game: Game, rules: CollectionRules, broken: Set<string>, isEsde: boolean): (keyof Game)[] {
  return requiredMediaFields(rules, isEsde).filter((field) => !hasMedia(game, field, broken));
}

/**
 * Juegos (rutas) con ese problema, para filtrar la lista. `null` = no se sabe todavía (el estado
 * no ha llegado) o el problema no es de juegos (ROMs nuevas, medios sobrantes).
 */
export function issueGames(kind: IssueKind, ctx: IssueContext): Set<string> | null {
  const parts = ctx.status?.parts;
  switch (kind) {
    case "data":
      return new Set(ctx.games.filter((g) => lacksData(g, ctx.rules)).map((g) => g.path));
    case "media":
      return new Set(ctx.games.filter((g) => missingMedia(g, ctx.rules, ctx.broken, ctx.isEsde).length > 0).map((g) => g.path));
    case "invalid":
      return new Set(ctx.games.filter(hasInvalidData).map((g) => g.path));
    case "arrmSort":
      return new Set(ctx.games.filter(hasArrmSortname).map((g) => g.path));
    case "missingRoms":
      return ctx.missingRomPaths;
    case "names":
      return ctx.status?.names ? new Set([...ctx.status.names.files, ...ctx.status.names.titles]) : null;
    case "versions":
      return parts?.versions ? new Set(parts.versions.flat()) : null;
    case "damaged":
    case "placeholders":
    case "swapped":
    case "legacy":
      return parts?.[kind] ? new Set(parts[kind]!.games) : null;
    default:
      return null;
  }
}

/** Contexto a partir del estado del store (lo usan el filtro de la lista y el panel) */
export function issueContextOf(s: {
  games: Game[];
  settings: { collection_rules?: Partial<CollectionRules> };
  brokenMedia: Set<string>;
  collectionStatus: CollectionStatusState | null;
  missingRomPaths: Set<string>;
  isEsde: boolean;
}): IssueContext {
  return {
    games: s.games,
    rules: collectionRulesFrom(s.settings.collection_rules),
    broken: s.brokenMedia,
    isEsde: s.isEsde,
    status: s.collectionStatus,
    missingRomPaths: s.missingRomPaths,
  };
}

/**
 * Nombres fuera de formato: la vista previa de Renombrar con las opciones de Ajustes, solo con
 * la caché del scraper (no pregunta a nadie). Un fichero que no se puede renombrar por un
 * choque no cuenta: no hay nada que aplicar.
 */
export async function checkNames(systemPath: string, saved: Partial<RenameOptions> | undefined, scraper: ScraperStatus): Promise<NamesStatus> {
  const options = renameOptionsFrom(saved);
  const request = { sources: activeSources(options, scraper), scraper: options.scraper, rules: options.rules, keep: options.keep };
  const entries = await renamePreview(systemPath, request, undefined, false);
  const plans = entries.map((e) => planRename(e, options));
  return {
    files: plans.filter((p) => p.fileChanges && !p.blocked).map((p) => p.entry.current_path),
    titles: plans.filter((p) => p.titleChanges).map((p) => p.entry.current_path),
  };
}
