import { renamePreview, renameRoms } from "../commands";
import { translate } from "../i18n";
import { useAppStore } from "../store/useAppStore";
import { activeSources, planRename, renameOptionsFrom, renameRequestOf } from "./renameSources";
import { showMessage } from "./dialogs";
import { reportWithUndo } from "./undo";

/**
 * Renombrado sin ventana (ROMs nuevas con «Renombrar automáticamente»): con las opciones de
 * Ajustes, calcula la propuesta de esos juegos y aplica lo que no choca, igual que si se pulsara
 * Aplicar en la ventana de Renombrar con todo marcado. Al final, un resumen con Deshacer.
 * `askScraper`: preguntar a ScreenScraper por las ROMs que no estén en su caché (si no se han
 * scrapeado; el scrapeo ya guarda el nombre).
 */
export async function autoRename(systemPath: string, gamePaths: string[], askScraper: boolean): Promise<void> {
  if (gamePaths.length === 0) return;
  const { settings, scraperStatus, trackBackground, loadGames } = useAppStore.getState();
  const options = renameOptionsFrom(settings.rename_options);
  const request = { sources: activeSources(options, scraperStatus), scraper: options.scraper, rules: options.rules, keep: options.keep };
  try {
    // Mientras pregunta (puede tardar con ScreenScraper) se ve el aviso «Analizando…»
    const entries = await trackBackground(renamePreview(systemPath, request, gamePaths, askScraper && request.sources.includes("scraper")));
    const plans = entries.map((entry) => planRename(entry, options));
    const toApply = plans.filter((p) => p.changes);
    const clashes = plans.filter((p) => p.fileChanges && p.blocked).length;
    if (toApply.length === 0 && clashes === 0) return;

    const result = toApply.length > 0 ? await renameRoms(systemPath, toApply.map(renameRequestOf)) : null;
    await loadGames(systemPath);
    const lines = [translate("newRoms.autoRenamed")];
    if (result) lines.push(translate("rename.done", { count: result.renamed }), translate("rename.doneTitles", { count: result.titled }));
    if (clashes > 0) lines.push(translate("newRoms.autoRenameClashes", { count: clashes }));
    if (result && result.skipped.length > 0) {
      lines.push("", translate("rename.skipped", { count: result.skipped.length }), ...result.skipped);
    }
    // Con algo renombrado se puede deshacer; si solo hubo choques, un aviso normal
    if (result) await reportWithUndo(lines.join("\n"));
    else await showMessage(lines.join("\n"));
  } catch (e) {
    await showMessage(translate("rename.failed", { error: String(e) }), "error");
  }
}
