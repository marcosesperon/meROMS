import type { ArticleMode, NameScraper, NameSource, RenameEntry, RenameOptions, RenameRequest } from "../types";
import type { ScraperStatus } from "../commands";
import type { TranslationKeys } from "../i18n/en";
import { articleToEnd, articleToFront } from "./articles";

/** Fuentes del nombre de fichero, con sus textos (nombre y ayuda) */
export const NAME_SOURCES: { id: NameSource; label: TranslationKeys; help: TranslationKeys }[] = [
  { id: "dat", label: "rename.sourceDat", help: "rename.sourceDatHelp" },
  { id: "scraper", label: "rename.sourceScraper", help: "rename.sourceScraperHelp" },
  { id: "goodtools", label: "rename.sourceGoodtools", help: "rename.sourceGoodtoolsHelp" },
  { id: "title", label: "rename.sourceTitle", help: "rename.sourceTitleHelp" },
];

export const nameSourceText = (id: NameSource) => NAME_SOURCES.find((s) => s.id === id) ?? NAME_SOURCES[0];

/**
 * Scrapers que saben el nombre de fichero de una ROM (reconocida por su contenido). Solo
 * ScreenScraper: LaunchBox y TheGamesDB buscan por nombre y solo dan el título del juego.
 */
const NAME_SCRAPERS: { id: NameScraper; label: string; usable: (status: ScraperStatus) => boolean }[] = [
  { id: "screenscraper", label: "ScreenScraper", usable: (status) => status.screenscraper },
];

/** Los scrapers de nombres que se pueden usar ahora (configurados) */
export const usableNameScrapers = (status: ScraperStatus) => NAME_SCRAPERS.filter((s) => s.usable(status));

/** Lo de serie: fuentes «oficiales» primero, reglas puestas, todas las etiquetas, ficheros y títulos */
export const DEFAULT_RENAME_OPTIONS: RenameOptions = {
  sources: ["dat", "scraper", "goodtools"],
  disabled: ["title"],
  scraper: "screenscraper",
  rules: true,
  keep: { region: true, language: true, revision: true, status: true, dump: true, other: true },
  article: "keep",
  renameFiles: true,
  changeTitles: true,
};

/** Las opciones guardadas, completadas con las de serie (y sin fuentes repetidas ni perdidas) */
export function renameOptionsFrom(saved: Partial<RenameOptions> | undefined): RenameOptions {
  const o = { ...DEFAULT_RENAME_OPTIONS, ...saved, keep: { ...DEFAULT_RENAME_OPTIONS.keep, ...saved?.keep } };
  const known = (list: NameSource[] | undefined) => (list ?? []).filter((s) => NAME_SOURCES.some((n) => n.id === s));
  const sources = [...new Set(known(o.sources))];
  const disabled = [...new Set(known(o.disabled))].filter((s) => !sources.includes(s));
  // Una fuente que no está en ninguna lista (versión nueva) entra apagada al final
  const missing = NAME_SOURCES.map((s) => s.id).filter((s) => !sources.includes(s) && !disabled.includes(s));
  return { ...o, sources, disabled: [...disabled, ...missing] };
}

/** La fuente «Scraper» solo cuenta si hay un scraper de nombres configurado */
export const activeSources = (o: RenameOptions, status: ScraperStatus) =>
  o.sources.filter((s) => s !== "scraper" || usableNameScrapers(status).some((n) => n.id === o.scraper));

/**
 * Título limpio a partir del nombre de fichero: sin etiquetas («Sonic (USA) (Rev 1)» → «Sonic»;
 * una etiqueta al principio, como «[BIOS] X», se queda) y con el artículo donde se pida.
 */
export function titleFromName(name: string, article: ArticleMode): string {
  const match = name.match(/^(.*?\S.*?)\s*[([].*$/);
  const title = (match ? match[1] : name).trim();
  if (article === "end") return articleToEnd(title) ?? title;
  if (article === "front") return articleToFront(title) ?? title;
  return title;
}

/** Para comparar títulos sin mirar mayúsculas, acentos ni signos («Contra III: The…» = «Contra III - The…») */
const comparable = (s: string) =>
  s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]/g, "");

/**
 * Título propuesto: el sacado del nombre de fichero, salvo que diga lo mismo que el actual (solo
 * cambian signos o mayúsculas: el actual suele venir del scraper y está mejor escrito). Con el
 * artículo cambiado de sitio sí se propone.
 * Si ninguna fuente dio nombre (`fromSource` false), el fichero es el de siempre, solo limpiado:
 * el título solo se cambia si era una copia del nombre de fichero de antes («0004 Green Corps»).
 */
export function proposedTitle(fileName: string, current: string | undefined, article: ArticleMode, fromSource = true, oldFileName = ""): string {
  const fromFile = titleFromName(fileName, article);
  if (!current) return fromFile;
  const moved = article === "end" ? articleToEnd(current) : article === "front" ? articleToFront(current) : null;
  const base = moved ?? current;
  const copiedFromFile = comparable(titleFromName(current, "keep")) === comparable(titleFromName(oldFileName, "keep"));
  if (!fromSource && !copiedFromFile) return base;
  return comparable(base) === comparable(fromFile) ? base : fromFile;
}

/** Caracteres que no valen en un nombre de fichero (los mismos que mira el backend) */
const FORBIDDEN = /[/\\:*?"<>|]/;

/** Una carpeta como ROM no tiene extensión: su nombre no se toca (el título sí) */
export const hasExtension = (e: RenameEntry) => e.current_path.slice(e.current_path.lastIndexOf("/") + 1) !== e.current_name;

/** Lo que se haría con un juego según las opciones (y lo escrito a mano, si lo hay) */
export interface RenamePlan {
  entry: RenameEntry;
  /** Nombre de fichero (sin extensión) y título que se aplicarían */
  file: string;
  title: string;
  /** Vacío, con espacios en los extremos o con caracteres prohibidos */
  invalid: boolean;
  /** El fichero no se puede renombrar así: nombre no válido o un conflicto sin resolver */
  blocked: boolean;
  fileChanges: boolean;
  titleChanges: boolean;
  /** Hay algo que aplicar (fichero que se puede renombrar o título distinto) */
  changes: boolean;
}

/** Decide fichero y título de un juego: lo comparten la ventana de Renombrar y el renombrado automático */
export function planRename(entry: RenameEntry, options: RenameOptions, typedFile?: string, typedTitle?: string): RenamePlan {
  const file = typedFile ?? entry.proposed_name;
  const invalid = !file.trim() || file.trim() !== file || FORBIDDEN.test(file);
  // Un conflicto se salva escribiendo otro nombre
  const blocked = invalid || (!!entry.conflict && file === entry.proposed_name);
  const fileChanges = options.renameFiles && hasExtension(entry) && file !== entry.current_name;
  const title = typedTitle ?? proposedTitle(file, entry.game_title, options.article, entry.source !== "current", entry.current_name);
  const titleChanges = options.changeTitles && !!title.trim() && title !== (entry.game_title ?? "");
  return { entry, file, title, invalid, blocked, fileChanges, titleChanges, changes: (fileChanges && !blocked) || titleChanges };
}

/** La petición para `rename_roms`: el fichero solo si se puede renombrar, el título solo si cambia */
export const renameRequestOf = (p: RenamePlan): RenameRequest => ({
  current_path: p.entry.current_path,
  new_name: p.fileChanges && !p.blocked ? p.file : p.entry.current_name,
  new_title: p.titleChanges ? p.title.trim() : undefined,
});
