import { applyMediaReuse } from "../commands";
import { translate } from "../i18n";
import type { ReusableMedia } from "../types";
import { removeMedia } from "./collectionActions";
import { outcomeItem, type OutcomeItem } from "./outcome";

/** Lo que dice cada motivo (cómo se ha sabido de qué juego es) */
export const REUSE_REASON_KEYS = {
  name: "reuse.reasonName",
  old_name: "reuse.reasonOldName",
  dat: "reuse.reasonDat",
  mame: "reuse.reasonMame",
  broken_ref: "reuse.reasonBroken",
  manual: "reuse.reasonManual",
} as const;

/**
 * Incorpora los sobrantes elegidos. `keepNew(item)`: quedarse con el sobrante (true) o con lo que
 * el juego ya tiene (false). Primero se quita lo que no se queda (papelera o apartados, según Mi
 * colección: así el nombre bueno queda libre) y después se mueven los que se quedan a su nombre
 * unificado. Devuelve las líneas del resumen (también como piezas) y lo que no se pudo.
 */
export async function applyReuse(
  systemPath: string,
  items: ReusableMedia[],
  keepNew: (item: ReusableMedia) => boolean,
  action: "trash" | "set_aside",
  task?: string
): Promise<{ lines: string[]; items: OutcomeItem[]; failed: string[] }> {
  const lines: string[] = [];
  const pieces: OutcomeItem[] = [];
  const failed: string[] = [];
  const losers = items.flatMap((i) => (keepNew(i) ? (i.current ? [i.current.path] : []) : [i.path]));
  if (losers.length > 0) {
    const r = await removeMedia(systemPath, [...new Set(losers)], action, [], task);
    if (r.line) lines.push(r.line);
    if (r.item) pieces.push(r.item);
    failed.push(...r.failed);
  }
  const winners = items.filter(keepNew).map((i) => ({ path: i.path, game_path: i.game_path, field: i.field }));
  if (winners.length > 0) {
    const r = await applyMediaReuse(systemPath, winners, task);
    lines.push(translate("reuse.done", { count: r.moved + r.linked }));
    pieces.push(outcomeItem("reuse.done", r.moved + r.linked));
    failed.push(...r.skipped);
  }
  return { lines, items: pieces, failed };
}
