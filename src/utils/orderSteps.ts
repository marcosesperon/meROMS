import {
  addNewRoms,
  applyMediaNormalization,
  detectSwappedMedia,
  findDamagedMedia,
  findDuplicates,
  findPlaceholderMedia,
  findReusableMedia,
  findUnusedMedia,
  listMissingRoms,
  listNewRoms,
  findRepeatedEntries,
  mergeRepeatedEntries,
  type RepeatedEntry,
  planMediaNormalization,
  removeGamesWithFiles,
  renamePreview,
  saveGamelist,
  renameRoms,
  scrapeSystem,
  setAsideGames,
  swapMedia,
  type ScraperStatus,
} from "../commands";
import { translate } from "../i18n";
import type { TranslationKeys } from "../i18n/en";
import { useAppStore } from "../store/useAppStore";
import type { CollectionRules, Game, RenameOptions, ScrapePolicy } from "../types";
import { DATA_FIELDS as POLICY_DATA_FIELDS } from "../components/common/ScrapePolicyEditor";
import { AVAILABLE_MEDIA_TYPES } from "../types";
import { hasSetAsideDir, mediaAction, removeMedia, removeVersions, versionsToRemove } from "./collectionActions";
import { hasArrmSortname, hasInvalidData, hasNotGameData, lacksData, missingMedia, requiredMediaFields, withoutInvalidData } from "./collectionStatus";
import { planRename, renameRequestOf, previewRequest } from "./renameSources";
import { applyReuse } from "./mediaReuse";
import { outcomeItem, type OutcomeItem } from "./outcome";

/**
 * «Ponerlo todo en orden»: los pasos, en este orden y por esto:
 *  1. ROMs: añadir las nuevas y quitar las entradas sin ROM (todo lo demás trabaja sobre la gamelist).
 *  2. Versiones: dejar una por juego (antes de scrapear, para no gastar cupo en las que se apartan).
 *  3. Medios: normalizar nombres antiguos, recolocar carátulas y capturas cambiadas y quitar
 *     dañados, placeholders y sobrantes (antes de scrapear, para que rellene los huecos).
 *  4. Scrapear solo los juegos a los que les falta algo obligatorio, «solo si está vacío».
 *  5. Nombres y títulos (el último: ya tiene el nombre de ScreenScraper y los datos, y al
 *     renombrar se mueven también los medios recién bajados).
 *
 * Cada paso sabe preparar su vista previa (`plan`) y aplicarse (`apply`). Al aplicar se vuelve a
 * mirar todo: los pasos de antes pueden haber cambiado la gamelist.
 */
export type OrderStep = "roms" | "versions" | "media" | "scrape" | "names";

export const ORDER_STEPS: OrderStep[] = ["roms", "versions", "media", "scrape", "names"];

/** Con qué se trabaja */
export interface OrderContext {
  systemPath: string;
  rules: CollectionRules;
  renameOptions: RenameOptions;
  scraperStatus: ScraperStatus;
  /** Hay alguna fuente de scrapeo para este sistema */
  canScrape: boolean;
  isEsde: boolean;
  /** Solo estos juegos (los mostrados o los marcados); null = el sistema entero */
  scope: Set<string> | null;
  /** Scrapear también los que el scraper ya no encontró (si no, se los salta) */
  retryMisses: boolean;
}

/** Vista previa de un paso */
export interface StepPlan {
  step: OrderStep;
  /** Cuántas cosas haría (0 = nada que hacer) */
  count: number;
  /** Qué va a hacer, con números */
  lines: string[];
  /** Unos ejemplos */
  examples: string[];
  /** Por qué no se puede hacer (sin carpeta de apartados, sin scraper…) */
  blocked?: string;
  /** Scrapeo: juegos con algo que buscar que se saltan por no encontrados antes */
  skipped?: number;
}

/** Lo que hace falta mientras se aplica: el id de tarea para el progreso y si se ha parado */
export interface StepRunner {
  task: () => string;
  stopped: () => boolean;
}

/** Resumen de lo hecho en un paso: piezas con su número (el resumen las enseña destacadas) */
export interface StepOutcome {
  items: OutcomeItem[];
  failed: string[];
}

const inScope = (ctx: OrderContext) => (path: string) => !ctx.scope || ctx.scope.has(path);
const nameOf = (path: string) => path.replace(/^.*\//, "");

// ---------- 1. ROMs ----------

async function gatherRoms(ctx: OrderContext) {
  const [news, missing, repeated] = await Promise.all([
    ctx.scope ? Promise.resolve([]) : listNewRoms(ctx.systemPath),
    listMissingRoms(ctx.systemPath),
    findRepeatedEntries(ctx.systemPath),
  ]);
  return {
    // La misma ROM más de una vez: se deja una entrada (lo primero, para que lo demás vea una sola)
    repeated: repeated.filter((r) => inScope(ctx)(r.path)),
    // Las de extensión rara salen desmarcadas en ROMs nuevas: aquí no se añaden solas
    newRoms: news.filter((r) => r.known_extension),
    // Si faltan todas, la carpeta o el disco no está conectado: no se quita nada
    missing: missing.all_missing ? [] : missing.games.filter((g) => inScope(ctx)(g.path)),
    disconnected: missing.all_missing && missing.games.length > 0,
  };
}

async function planRoms(ctx: OrderContext): Promise<StepPlan> {
  const { repeated, newRoms, missing, disconnected } = await gatherRoms(ctx);
  const lines: string[] = [];
  if (repeated.length) lines.push(translate("order.romsMerge", { count: repeated.length }));
  if (newRoms.length) lines.push(translate("order.romsAdd", { count: newRoms.length }));
  if (missing.length) lines.push(translate(mediaAction(ctx.rules) === "set_aside" ? "order.romsRemoveSetAside" : "order.romsRemove", { count: missing.length }));
  if (disconnected) lines.push(translate("order.romsDisconnected"));
  const examples = [...repeated.slice(0, 2).map(mergeExample), ...newRoms.slice(0, 2).map((r) => `+ ${r.proposed_name}`), ...missing.slice(0, 2).map((g) => `− ${g.name || nameOf(g.path)}`)];
  return { step: "roms", count: repeated.length + newRoms.length + missing.length, lines, examples };
}

/** «Spider-Man: se queda la de 1991 · SEGA» */
function mergeExample(r: RepeatedEntry): string {
  const keep = r.entries[r.keep];
  const what = [keep.year, keep.publisher || keep.developer].filter(Boolean).join(" · ") || keep.name || "";
  return `= ${translate("order.romsMergeKeep", { name: keep.name || nameOf(r.path), what })}`;
}

async function applyRoms(ctx: OrderContext, run: StepRunner): Promise<StepOutcome> {
  const { repeated, newRoms, missing } = await gatherRoms(ctx);
  const out: StepOutcome = { items: [], failed: [] };
  if (repeated.length) {
    const r = await mergeRepeatedEntries(ctx.systemPath, repeated.map((x) => x.path));
    out.items.push(outcomeItem("order.romsMerged", r.merged));
  }
  if (newRoms.length) {
    const added = await addNewRoms(ctx.systemPath, newRoms.map((r) => ({ path: r.path, name: r.proposed_name })), run.task());
    out.items.push(outcomeItem("order.romsAdded", added));
  }
  if (missing.length && !run.stopped()) {
    const paths = missing.map((g) => g.path);
    if (mediaAction(ctx.rules) === "set_aside") {
      const r = await setAsideGames(ctx.systemPath, paths, run.task());
      out.items.push(outcomeItem("order.romsRemoved", r.moved));
      out.failed.push(...r.skipped);
    } else {
      const r = await removeGamesWithFiles(ctx.systemPath, paths, false, run.task());
      out.items.push(outcomeItem("order.romsRemoved", r.removed));
      out.failed.push(...r.failed);
    }
  }
  return out;
}

// ---------- 2. Versiones ----------

async function gatherVersions(ctx: OrderContext) {
  const groups = await findDuplicates(ctx.systemPath);
  const remove = versionsToRemove(groups, ctx.rules.protect_played).filter(inScope(ctx));
  return { groups, remove };
}

function versionsBlocked(ctx: OrderContext): string | undefined {
  return ctx.rules.versions_action === "set_aside" && !hasSetAsideDir() ? translate("order.needFolder") : undefined;
}

async function planVersions(ctx: OrderContext): Promise<StepPlan> {
  const { groups, remove } = await gatherVersions(ctx);
  const removing = new Set(remove);
  const examples = groups
    .filter((g) => g.games.some((x) => removing.has(x.path)))
    .slice(0, 3)
    .map((g) => {
      const keeper = g.games.find((x) => x.path === g.keep);
      const gone = g.games.find((x) => removing.has(x.path));
      return translate("order.versionsExample", { gone: nameOf(gone?.path ?? ""), keep: nameOf(keeper?.path ?? "") });
    });
  const lines = remove.length ? [translate(`order.versions_${ctx.rules.versions_action}` as TranslationKeys, { count: remove.length })] : [];
  return { step: "versions", count: remove.length, lines, examples, blocked: remove.length ? versionsBlocked(ctx) : undefined };
}

async function applyVersions(ctx: OrderContext, run: StepRunner): Promise<StepOutcome> {
  if (versionsBlocked(ctx)) return { items: [], failed: [] };
  const { remove } = await gatherVersions(ctx);
  const r = await removeVersions(ctx.systemPath, remove, ctx.rules.versions_action, run.task());
  return { items: r.item ? [r.item] : [], failed: r.failed };
}

// ---------- 3. Medios ----------

async function gatherRelocate(ctx: OrderContext) {
  const [legacy, swapped] = await Promise.all([planMediaNormalization(ctx.systemPath), detectSwappedMedia(ctx.systemPath)]);
  return {
    legacy: legacy.filter((i) => i.status !== "conflict" && inScope(ctx)(i.game_path)),
    swapped: swapped.proposals.filter((p) => inScope(ctx)(p.game_path)),
    // Todo el sistema al revés: se intercambian carátula y captura de todos los juegos que tengan alguna
    whole: swapped.looks_swapped ? wholeSwapGames(ctx) : [],
    profiles: { cover: swapped.cover_profile, shot: swapped.screenshot_profile },
  };
}

/** Juegos del ámbito con carátula o captura (los que entran en el intercambio de todo el sistema) */
function wholeSwapGames(ctx: OrderContext): Game[] {
  return useAppStore.getState().games.filter((g) => inScope(ctx)(g.path) && (g.image || g.thumbnail));
}

/** Sobrantes que son de un juego (y ficheros de referencias rotas): se incorporan con la propuesta */
async function gatherReuse(ctx: OrderContext) {
  const list = await findReusableMedia(ctx.systemPath);
  return list.filter((i) => inScope(ctx)(i.game_path));
}

async function gatherLeftovers(ctx: OrderContext) {
  const [damaged, fillers, unused] = await Promise.all([
    findDamagedMedia(ctx.systemPath),
    findPlaceholderMedia(ctx.systemPath),
    // Los sobrantes no son de ningún juego: solo con el sistema entero
    ctx.scope ? Promise.resolve([]) : findUnusedMedia(ctx.systemPath),
  ]);
  const placeholderFiles = fillers.flatMap((g) => g.files.filter((f) => inScope(ctx)(f.game_path)).map((f) => f.path));
  return {
    damaged: damaged.filter((d) => inScope(ctx)(d.game_path)).map((d) => d.path),
    fillers: fillers.filter((g) => g.files.some((f) => inScope(ctx)(f.game_path))),
    placeholderFiles,
    // Los que «Normalizar» recupera no sobran
    unused: unused.filter((f) => !f.recoverable).map((f) => f.path),
  };
}

async function planMedia(ctx: OrderContext): Promise<StepPlan> {
  const [{ legacy, swapped, whole, profiles }, { damaged, placeholderFiles, unused: allUnused }, reuse] = await Promise.all([gatherRelocate(ctx), gatherLeftovers(ctx), gatherReuse(ctx)]);
  const action = mediaAction(ctx.rules);
  const lines: string[] = [];
  if (whole.length) lines.push(translate("order.mediaSwapWhole", { count: whole.length }));
  if (legacy.length) lines.push(translate("order.mediaLegacy", { count: legacy.length }));
  if (swapped.length) lines.push(translate("order.mediaSwapped", { count: swapped.length }));
  // Lo que se aprovecha no cuenta como sobrante (aunque su copia peor también se vaya)
  if (reuse.length) lines.push(translate("order.mediaReuse", { count: reuse.length, filled: reuse.filter((i) => i.keep_new).length }));
  const reused = new Set(reuse.map((i) => i.path));
  const unused = allUnused.filter((p) => !reused.has(p));
  const gone = damaged.length + placeholderFiles.length + unused.length;
  if (damaged.length) lines.push(translate("order.mediaDamaged", { count: damaged.length }));
  if (placeholderFiles.length) lines.push(translate("order.mediaPlaceholders", { count: placeholderFiles.length }));
  if (unused.length) lines.push(translate("order.mediaUnused", { count: unused.length }));
  if (gone) lines.push(translate(action === "set_aside" ? "order.mediaToSetAside" : "order.mediaToTrash"));
  const size = (p: { width: number; height: number } | null) => (p ? `${p.width}×${p.height}` : "?");
  const examples = [
    ...(whole.length ? [translate("order.mediaSwapWholeExample", { cover: size(profiles.cover), shot: size(profiles.shot) })] : []),
    ...legacy.slice(0, 2).map((i) => `${i.from_relative} → ${i.to_relative}`),
    ...swapped.slice(0, 1).map((p) => translate("order.mediaSwappedExample", { name: p.game_name })),
    ...reuse.filter((i) => i.keep_new).slice(0, 2).map((i) => `${i.relative} → ${i.game_name}`),
  ];
  return { step: "media", count: whole.length + legacy.length + swapped.length + reuse.length + gone, lines, examples };
}

async function applyMedia(ctx: OrderContext, run: StepRunner): Promise<StepOutcome> {
  const out: StepOutcome = { items: [], failed: [] };
  // Primero lo que se recoloca (así lo que se mueve a su sitio ya no cuenta como sobrante)
  const { legacy, swapped, whole } = await gatherRelocate(ctx);
  // Lo primero, el sistema entero al revés (lo demás se compara con lo habitual, que estaba al revés)
  if (whole.length) {
    const r = await swapMedia(ctx.systemPath, whole.map((g) => g.path), "image", "thumbnail", run.task());
    out.items.push(outcomeItem("order.resSwapped", r.swapped));
    out.failed.push(...r.skipped);
  }
  if (legacy.length && !run.stopped()) {
    const r = await applyMediaNormalization(ctx.systemPath, legacy, run.task());
    out.items.push(outcomeItem("order.resMoved", r.moved), outcomeItem("order.resLinked", r.linked));
    out.failed.push(...r.skipped);
  }
  if (swapped.length && !run.stopped()) {
    const r = await swapMedia(ctx.systemPath, swapped.map((p) => p.game_path), "image", "thumbnail", run.task());
    out.items.push(outcomeItem("order.resSwapped", r.swapped));
    out.failed.push(...r.skipped);
  }
  if (run.stopped()) return out;
  // Lo que es de un juego, a su sitio (con la propuesta: el de más resolución) antes de tirar nada
  const reuse = await gatherReuse(ctx);
  if (reuse.length) {
    const r = await applyReuse(ctx.systemPath, reuse, (i) => i.keep_new, mediaAction(ctx.rules), run.task());
    out.items.push(...r.items);
    out.failed.push(...r.failed);
  }
  if (run.stopped()) return out;
  // Después, lo que sobra, mirado otra vez
  const { damaged, fillers, placeholderFiles, unused } = await gatherLeftovers(ctx);
  const paths = [...new Set([...damaged, ...placeholderFiles, ...unused])];
  if (paths.length || fillers.length) {
    const r = await removeMedia(ctx.systemPath, paths, mediaAction(ctx.rules), fillers, run.task());
    if (r.item) out.items.push(r.item);
    out.failed.push(...r.failed);
  }
  return out;
}

// ---------- 4. Datos y medios que faltan ----------

/** Tipos del scraper que rellenan cada campo de medios (como `media::kind_for_scraper_type`) */
const FIELD_SCRAPER_TYPES: Record<string, string[]> = {
  image: ["box-2D"],
  thumbnail: ["ss"],
  video: ["video"],
  marquee: ["marquee", "wheel"],
  titleshot: ["sstitle"],
  boxback: ["box-2D-back"],
  boxart: ["box-3D"],
  cartridge: ["support-2D"],
  fanart: ["fanart"],
  mix: ["mixrbv2"],
  manual: ["manuel"],
  wheel: ["wheel"],
};

/** Scrapear solo lo obligatorio y solo si está vacío: lo que ya hay no se toca */
export function requiredScrapePolicy(rules: CollectionRules, isEsde: boolean): ScrapePolicy {
  const media = new Set(requiredMediaFields(rules, isEsde).flatMap((f) => FIELD_SCRAPER_TYPES[f as string] ?? []));
  return {
    // El título, la región y los idiomas, solo si están vacíos (los que tenían un título no válido
    // se quedan sin él; región e idiomas dan las etiquetas del nombre de fichero al renombrar)
    data: Object.fromEntries(
      POLICY_DATA_FIELDS.map(({ field }) => [field, ["name", "region", "lang"].includes(field) || rules.required_data.includes(field) ? "if_empty" : "keep"])
    ),
    media: Object.fromEntries(AVAILABLE_MEDIA_TYPES.map(({ id }) => [id, media.has(id) ? "if_empty" : "keep"])),
  };
}

/** Juegos con algo que buscar; `skipped` = los que se dejan por no encontrados antes */
function gatherScrape(ctx: OrderContext): { games: Game[]; skipped: number } {
  const { games, brokenMedia, scrapeMisses } = useAppStore.getState();
  const wanted = games.filter(
    (g) => inScope(ctx)(g.path) && (hasNotGameData(g) || !g.name?.trim() || lacksData(g, ctx.rules) || missingMedia(g, ctx.rules, brokenMedia, ctx.isEsde).length > 0)
  );
  if (ctx.retryMisses) return { games: wanted, skipped: 0 };
  const kept = wanted.filter((g) => !scrapeMisses.has(g.path));
  return { games: kept, skipped: wanted.length - kept.length };
}

/** Juegos del ámbito con datos no válidos del scraper («ZZZ(notgame):…») */
// (título «no es un juego» o fecha en otro formato; solo los del título hay que volver a scrapearlos)
const invalidGames = (ctx: OrderContext) => useAppStore.getState().games.filter((g) => inScope(ctx)(g.path) && hasInvalidData(g));

async function planScrape(ctx: OrderContext): Promise<StepPlan> {
  const { games, skipped } = gatherScrape(ctx);
  const invalid = invalidGames(ctx).length;
  return {
    step: "scrape",
    count: games.length,
    lines: [
      ...(invalid ? [translate("order.scrapeInvalid", { count: invalid })] : []),
      ...(games.length ? [translate("order.scrapeLine", { count: games.length })] : []),
    ],
    skipped,
    examples: games.slice(0, 3).map((g) => g.name || nameOf(g.path)),
    // Sin scraper, si hay datos no válidos que quitar el paso sigue sirviendo (solo para eso)
    blocked: games.length && !ctx.canScrape && !invalid ? translate("order.noScraper") : undefined,
  };
}

async function applyScrape(ctx: OrderContext): Promise<StepOutcome> {
  const items: OutcomeItem[] = [];
  // Primero, fuera los datos no válidos del scraper (así «solo si está vacío» puede rellenarlos)
  const invalid = new Set(invalidGames(ctx).map((g) => g.path));
  if (invalid.size) {
    const { games, folders, provider, loadGames } = useAppStore.getState();
    await saveGamelist(ctx.systemPath, { games: games.map((g) => (invalid.has(g.path) ? withoutInvalidData(g) : g)), folders: folders || [], provider });
    await loadGames(ctx.systemPath, { skipStatus: true });
    items.push(outcomeItem("order.scrapeInvalidDone", invalid.size));
  }
  if (!ctx.canScrape) return { items, failed: [] };
  const { games } = gatherScrape(ctx);
  if (games.length === 0) return { items, failed: [] };
  // La lista ya viene sin los no encontrados (si no se pidió reintentarlos): va tal cual
  const r = await scrapeSystem(ctx.systemPath, games.map((g) => g.path), requiredScrapePolicy(ctx.rules, ctx.isEsde), false, true);
  items.push(r.stopped ? outcomeItem("order.scrapeStopped") : outcomeItem("order.scrapeDone", games.length));
  return { items, failed: [] };
}

// ---------- 5. Nombres y títulos ----------

async function gatherNames(ctx: OrderContext) {
  const o = ctx.renameOptions;
  const request = previewRequest(o, ctx.scraperStatus);
  const entries = await renamePreview(ctx.systemPath, request, ctx.scope ? [...ctx.scope] : undefined, false);
  return entries.map((e) => planRename(e, o)).filter((p) => p.changes);
}

/** Juegos del ámbito con nombre para ordenar de ARRM («228 =-  …»), que se quita */
const arrmSortGames = (ctx: OrderContext) => useAppStore.getState().games.filter((g) => inScope(ctx)(g.path) && hasArrmSortname(g));

async function planNames(ctx: OrderContext): Promise<StepPlan> {
  const plans = await gatherNames(ctx);
  const files = plans.filter((p) => p.fileChanges && !p.blocked);
  const titles = plans.filter((p) => p.titleChanges);
  const sortnames = arrmSortGames(ctx).length;
  const lines: string[] = [];
  if (sortnames) lines.push(translate("order.namesArrmSort", { count: sortnames }));
  if (files.length) lines.push(translate("order.namesFiles", { count: files.length }));
  if (titles.length) lines.push(translate("order.namesTitles", { count: titles.length }));
  const examples = plans.slice(0, 3).map((p) => (p.fileChanges && !p.blocked ? `${p.entry.current_name} → ${p.file}` : `${p.entry.game_title ?? ""} → ${p.title}`));
  return { step: "names", count: plans.length + sortnames, lines, examples };
}

async function applyNames(ctx: OrderContext, run: StepRunner): Promise<StepOutcome> {
  const items: OutcomeItem[] = [];
  // Primero, fuera el nombre para ordenar de ARRM (una escritura de la gamelist)
  const stale = new Set(arrmSortGames(ctx).map((g) => g.path));
  if (stale.size) {
    const { games, folders, provider, loadGames } = useAppStore.getState();
    const updated = games.map((g) => {
      if (!stale.has(g.path)) return g;
      const next = { ...g };
      delete next.sortname;
      return next;
    });
    await saveGamelist(ctx.systemPath, { games: updated, folders: folders || [], provider });
    await loadGames(ctx.systemPath, { skipStatus: true });
    items.push(outcomeItem("order.namesArrmSortDone", stale.size));
  }
  if (run.stopped()) return { items, failed: [] };
  const plans = await gatherNames(ctx);
  if (plans.length === 0) return { items, failed: [] };
  const r = await renameRoms(ctx.systemPath, plans.map(renameRequestOf), run.task());
  items.push(outcomeItem("rename.done", r.renamed), outcomeItem("rename.doneTitles", r.titled));
  return { items, failed: r.skipped };
}

// ---------- Todos ----------

export const STEP_PLANNERS: Record<OrderStep, (ctx: OrderContext) => Promise<StepPlan>> = {
  roms: planRoms,
  versions: planVersions,
  media: planMedia,
  scrape: planScrape,
  names: planNames,
};

export const STEP_APPLIERS: Record<OrderStep, (ctx: OrderContext, run: StepRunner) => Promise<StepOutcome>> = {
  roms: applyRoms,
  versions: applyVersions,
  media: applyMedia,
  scrape: applyScrape,
  names: applyNames,
};
