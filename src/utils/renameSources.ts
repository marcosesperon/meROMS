import type { RenameSource } from "../types";
import type { TranslationKeys } from "../i18n/en";

/** Fuentes del nombre propuesto en Renombrar, con sus textos (botón, ayuda y «no hay nada que cambiar») */
export const RENAME_SOURCES: { id: RenameSource; label: TranslationKeys; help: TranslationKeys; empty: TranslationKeys }[] = [
  { id: "title", label: "rename.sourceTitle", help: "rename.sourceTitleHelp", empty: "rename.noRenames" },
  { id: "rules", label: "rename.sourceRules", help: "rename.sourceRulesHelp", empty: "rename.noRenamesRules" },
  { id: "dat", label: "rename.sourceDat", help: "rename.sourceDatHelp", empty: "rename.noRenamesDat" },
  { id: "screenscraper", label: "rename.sourceSs", help: "rename.sourceSsHelp", empty: "rename.noRenamesSs" },
];

/** Las fuentes que se pueden usar: la de ScreenScraper solo si está configurado */
export const usableRenameSources = (screenscraper: boolean) =>
  RENAME_SOURCES.filter((s) => s.id !== "screenscraper" || screenscraper);

export const renameSourceText = (id: RenameSource) => RENAME_SOURCES.find((s) => s.id === id) ?? RENAME_SOURCES[0];
