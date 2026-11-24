import {
  learnPlaceholders,
  removeGamesWithFiles,
  saveGamelist,
  setAsideGames,
  setAsideMedia,
  trashMedia,
  type DuplicateGroup,
  type PlaceholderGroup,
} from "../commands";
import { translate } from "../i18n";
import { useAppStore } from "../store/useAppStore";
import type { CollectionRules, VersionsAction } from "../types";
import { getActiveProfile } from "./profiles";
import { collectionRulesFrom } from "./collectionRules";
import { askConfirm, showMessage } from "./dialogs";
import { reportWithUndo } from "./undo";

/**
 * Lo que hacen las reglas de «Mi colección» con lo que sobra: las versiones que no se quedan
 * (apartar, ocultar o papelera) y los medios sobrantes o dañados (papelera o apartar). Lo usan
 * la ventana de Versiones y duplicados, la de Medios y «Ponerlo todo en orden».
 */

/** Hay carpeta de apartados en el perfil activo (sin ella, «apartar» no se puede usar) */
export function hasSetAsideDir(): boolean {
  return !!getActiveProfile(useAppStore.getState().settings)?.set_aside_dir?.trim();
}

/**
 * Las que sobran en cada grupo: todas menos la que se queda, sin las protegidas (favoritas o
 * jugadas) si la regla lo pide. Los grupos de discos de un mismo juego no se tocan.
 */
export function versionsToRemove(groups: DuplicateGroup[], protectPlayed: boolean): string[] {
  return groups.flatMap((group) =>
    group.discs ? [] : group.games.filter((game, i) => game.path !== group.keep && !(protectPlayed && group.protected[i])).map((g) => g.path)
  );
}

/** Resultado de quitar versiones: cuántas y lo que falló, para el resumen */
export interface ActionOutcome {
  done: number;
  failed: string[];
  /** Línea del resumen («12 versiones apartadas en …») */
  line: string;
}

/**
 * Quita versiones con la acción elegida. `task`: para el progreso y Cancelar (apartar y
 * papelera). Ocultar solo marca `hidden` en la gamelist (sin tocar ficheros).
 */
export async function removeVersions(systemPath: string, paths: string[], action: VersionsAction, task?: string): Promise<ActionOutcome> {
  if (paths.length === 0) return { done: 0, failed: [], line: "" };
  if (action === "hide") {
    const { games, folders, provider } = useAppStore.getState();
    const hide = new Set(paths);
    await saveGamelist(systemPath, { games: games.map((g) => (hide.has(g.path) ? { ...g, hidden: "true" } : g)), folders: folders || [], provider });
    return { done: hide.size, failed: [], line: translate("versions.doneHide", { count: hide.size }) };
  }
  if (action === "set_aside") {
    const r = await setAsideGames(systemPath, paths, task);
    return { done: r.moved, failed: r.skipped, line: translate("versions.doneSetAside", { count: r.moved, folder: r.folder }) };
  }
  const r = await removeGamesWithFiles(systemPath, paths, false, task);
  return { done: r.removed, failed: r.failed, line: translate("versions.doneTrash", { count: r.removed }) };
}

/** Qué se hace con los medios sobrantes o dañados: apartar solo si se pidió y hay carpeta */
export function mediaAction(rules: CollectionRules): "trash" | "set_aside" {
  return rules.leftover_media_action === "set_aside" && hasSetAsideDir() ? "set_aside" : "trash";
}

/** Manda ficheros de medios a la papelera o los aparta. Los placeholders se aprenden (el scraper ya no los guarda). */
export async function removeMedia(
  systemPath: string,
  paths: string[],
  action: "trash" | "set_aside",
  fillers: PlaceholderGroup[] = [],
  task?: string
): Promise<ActionOutcome> {
  let outcome: ActionOutcome = { done: 0, failed: [], line: "" };
  if (paths.length > 0) {
    if (action === "set_aside") {
      const r = await setAsideMedia(systemPath, paths, task);
      outcome = { done: r.moved, failed: r.skipped, line: translate("mediaTool.doneSetAside", { count: r.moved, folder: r.folder }) };
    } else {
      const r = await trashMedia(systemPath, paths, task);
      outcome = { done: r.trashed, failed: r.failed, line: translate("cleanup.deleted", { count: r.trashed }) };
    }
  }
  if (fillers.length > 0) {
    await learnPlaceholders(fillers.map((g) => ({ size: g.size, md5: g.md5, field: g.fields[0] ?? "" })));
  }
  return outcome;
}

/**
 * Quitar juegos de la colección (los marcados o uno desde el menú del botón derecho): papelera
 * o apartar si así lo dice «Mi colección» (ocultar no es eliminar: papelera). Pregunta antes con
 * lo que se va a quitar y luego recarga y da el resumen (apartar, con Deshacer).
 * Devuelve si se quitó algo.
 */
export async function removeGames(paths: string[]): Promise<boolean> {
  const { selectedSystem, settings, loadGames, loadSystems, setSelectedPaths, selectedGame, selectGame } = useAppStore.getState();
  if (!selectedSystem || paths.length === 0) return false;
  const rule = collectionRulesFrom(settings.collection_rules).versions_action;
  const action = rule === "set_aside" ? "set_aside" : "trash";
  try {
    const plan = await removeGamesWithFiles(selectedSystem.path, paths, true);
    const question =
      action === "set_aside"
        ? translate("bulk.removeConfirmSetAside", { count: plan.removed })
        : translate("duplicates.removeConfirm", { games: plan.removed, files: plan.files.length });
    if (!(await askConfirm(question))) return false;
    const outcome = await removeVersions(selectedSystem.path, paths, action);
    setSelectedPaths(new Set());
    if (selectedGame && paths.includes(selectedGame.path)) selectGame(null);
    await loadGames(selectedSystem.path);
    loadSystems();
    const lines = [outcome.line];
    if (outcome.failed.length) lines.push("", translate("duplicates.removeFailed", { count: outcome.failed.length }), ...outcome.failed);
    if (action === "set_aside") await reportWithUndo(lines.join("\n"));
    else await showMessage(lines.join("\n"), outcome.failed.length ? "error" : "info");
    return true;
  } catch (e) {
    // Sin carpeta de apartados elegida
    await showMessage(String(e) === "no_set_aside_dir" ? translate("versions.noFolder") : String(e), "error");
    return false;
  }
}
