import type { NameScraper, RenameSource } from "../types";
import type { ScraperStatus } from "../commands";
import type { TranslationKeys } from "../i18n/en";

/** Fuentes del nombre propuesto en Renombrar, con sus textos (botón, ayuda y «no hay nada que cambiar») */
export const RENAME_SOURCES: { id: RenameSource; label: TranslationKeys; help: TranslationKeys; empty: TranslationKeys }[] = [
  { id: "title", label: "rename.sourceTitle", help: "rename.sourceTitleHelp", empty: "rename.noRenames" },
  { id: "rules", label: "rename.sourceRules", help: "rename.sourceRulesHelp", empty: "rename.noRenamesRules" },
  { id: "dat", label: "rename.sourceDat", help: "rename.sourceDatHelp", empty: "rename.noRenamesDat" },
  { id: "goodtools", label: "rename.sourceGoodtools", help: "rename.sourceGoodtoolsHelp", empty: "rename.noRenamesGoodtools" },
  { id: "scraper", label: "rename.sourceScraper", help: "rename.sourceScraperHelp", empty: "rename.noRenamesScraper" },
];

export const renameSourceText = (id: RenameSource) => RENAME_SOURCES.find((s) => s.id === id) ?? RENAME_SOURCES[0];

/**
 * Scrapers que saben el nombre de fichero de una ROM (reconocida por su contenido). Solo
 * ScreenScraper: LaunchBox y TheGamesDB buscan por nombre y solo dan el título del juego.
 */
const NAME_SCRAPERS: { id: NameScraper; label: string; usable: (status: ScraperStatus) => boolean }[] = [
  { id: "screenscraper", label: "ScreenScraper", usable: (status) => status.screenscraper },
];

/** Los scrapers de nombres que se pueden usar ahora (configurados) */
export const usableNameScrapers = (status: ScraperStatus) => NAME_SCRAPERS.filter((s) => s.usable(status));

/** Las fuentes que se pueden usar: «Scraper» solo si hay algún scraper de nombres configurado */
export const usableRenameSources = (status: ScraperStatus) =>
  RENAME_SOURCES.filter((s) => s.id !== "scraper" || usableNameScrapers(status).length > 0);
