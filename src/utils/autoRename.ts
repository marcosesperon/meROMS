import { findDuplicates, renamePreview, renameRoms } from "../commands";
import { translate } from "../i18n";
import { useAppStore } from "../store/useAppStore";
import { activeSources, planRename, renameOptionsFrom, renameRequestOf } from "./renameSources";
import { showMessage } from "./dialogs";
import { resolveRenameClash } from "./renameClash";
import { askRename } from "./renameOffer";
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

/**
 * Tras scrapear un juego (botón Scrapear de la ficha): si con las opciones de Ajustes su fichero
 * o su título quedarían distintos, se propone renombrarlo con un aviso que enseña el cambio
 * («Fichero: a → b», «Título: x → y») y los botones Renombrar / Ahora no. Si no cambiaría
 * nada, no sale nada. Solo con la caché del scraper: el scrapeo acaba de guardar el nombre de
 * ScreenScraper de esa ROM. Devuelve la ruta nueva del juego (o la misma) para volver a elegirlo.
 */
export async function offerRename(systemPath: string, gamePath: string): Promise<string> {
  const { settings, scraperStatus, loadGames } = useAppStore.getState();
  const options = renameOptionsFrom(settings.rename_options);
  const request = { sources: activeSources(options, scraperStatus), scraper: options.scraper, rules: options.rules, keep: options.keep };
  try {
    const [entry] = await renamePreview(systemPath, request, [gamePath], false);
    if (!entry) return gamePath;
    let plan = planRename(entry, options);
    const folder = gamePath.slice(0, gamePath.lastIndexOf("/") + 1);
    const extension = gamePath.slice(folder.length + entry.current_name.length);
    const nameClash = plan.fileChanges && plan.blocked && !plan.invalid && !!entry.conflict;

    // ¿Ya estaba este juego en la colección? (mismo ID, contenido, DAT o nombre: lo de
    // Versiones y duplicados) Si el nombre correcto choca, eso se pregunta abajo
    if (!nameClash) {
      const groups = await findDuplicates(systemPath).catch(() => []);
      const group = groups.find((g) => !g.discs && g.games.some((x) => x.path === gamePath));
      const others = group?.games.filter((x) => x.path !== gamePath) ?? [];
      // Con varios, el que se recomienda quedarse (si no es este) o el primero
      const other = others.find((x) => x.path === group?.keep) ?? others[0];
      if (group && other) {
        const choice = await resolveRenameClash(systemPath, gamePath, other.path, { reason: group.reason, detail: group.detail, keep: group.keep });
        if (choice === "removeThis") return other.path;
        // Quitada la otra (o las dos se quedan): sigue con el renombrado, si hay algo que cambiar
        if (choice === "removeOther") {
          const [fresh] = await renamePreview(systemPath, request, [gamePath], false);
          if (fresh) plan = planRename(fresh, options);
        }
      }
    }
    if (!plan.changes) return gamePath;

    // El nombre correcto ya lo tiene otra ROM: se enseñan las dos y se pregunta qué hacer
    if (nameClash) {
      const otherPath = folder + plan.file + extension;
      const choice = await resolveRenameClash(systemPath, gamePath, otherPath);
      // Quitada esta: el juego que queda es el otro
      if (choice === "removeThis") return otherPath;
      if (choice === "removeOther") {
        // Ya no hay choque: se renombra sin volver a preguntar (ya lo ha decidido)
        const [fresh] = await renamePreview(systemPath, request, [gamePath], false);
        plan = fresh ? planRename(fresh, options) : plan;
        if (!plan.changes || plan.blocked) return gamePath;
        const result = await renameRoms(systemPath, [renameRequestOf(plan)]);
        await loadGames(systemPath);
        await useAppStore.getState().refreshUndo();
        return result.renamed > 0 && plan.fileChanges ? folder + plan.file + extension : gamePath;
      }
      // Mantener las dos: sigue como siempre (solo el título, si cambia)
      if (!plan.titleChanges) return gamePath;
    }

    // La propuesta, en una ventana de meROMS con la carátula y lo que ha traído el scraper
    if (!(await askRename(systemPath, gamePath, entry, plan))) return gamePath;

    const result = await renameRoms(systemPath, [renameRequestOf(plan)]);
    await loadGames(systemPath);
    if (result.skipped.length > 0) {
      await showMessage([translate("rename.skipped", { count: result.skipped.length }), ...result.skipped].join("\n"), "error");
    } else {
      await useAppStore.getState().refreshUndo();
    }
    // Con el fichero renombrado, la ruta del juego es otra: misma carpeta y extensión, nombre nuevo
    if (result.renamed === 0 || !plan.fileChanges || plan.blocked) return gamePath;
    return folder + plan.file + extension;
  } catch (e) {
    await showMessage(translate("rename.failed", { error: String(e) }), "error");
    return gamePath;
  }
}
