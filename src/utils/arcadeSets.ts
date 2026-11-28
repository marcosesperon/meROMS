import { arcadeCheck, type ArcadeCoreId, type ArcadeGame, type SetCheck } from "../commands";
import type { TranslationKeys } from "../i18n/en";

type T = (key: TranslationKeys, vars?: Record<string, string | number>) => string;

/** Nombre de cada núcleo (son nombres propios: no se traducen) */
export const CORE_NAMES: Record<ArcadeCoreId, string> = {
  fbneo: "FinalBurn Neo",
  mame: "MAME - Current",
  mame2010: "MAME 2010",
  mame2003plus: "MAME 2003-Plus",
};

export const CORE_IDS: ArcadeCoreId[] = ["fbneo", "mame", "mame2010", "mame2003plus"];

/** Icono corto de cada estado, para las celdas de la tabla */
export const STATE_MARK: Record<SetCheck["state"], string> = {
  ok: "✓",
  needs: "!",
  wrong: "✗",
  absent: "–",
  not_a_game: "·",
};

/** Color de cada estado */
export const STATE_COLOR: Record<SetCheck["state"], string> = {
  ok: "text-emerald-400",
  needs: "text-amber-300",
  wrong: "text-red-400",
  absent: "text-zinc-600",
  not_a_game: "text-zinc-500",
};

/** Qué le falta a un set para un núcleo, una línea por cosa (vacío si está completo) */
export function checkProblems(check: SetCheck, t: T): string[] {
  const lines: string[] = [];
  if (check.bios) lines.push(t("arcade.needBios", { name: `${check.bios}.zip` }));
  if (check.parent) lines.push(t("arcade.needParent", { name: `${check.parent}.zip` }));
  if (check.devices?.length) lines.push(t("arcade.needDevices", { names: check.devices.map((d) => `${d}.zip`).join(", ") }));
  if (check.chd?.length) lines.push(t("arcade.needChd", { names: check.chd.map((d) => `${d}.chd`).join(", ") }));
  if (check.bad_chips > 0) {
    const names = check.missing_chips?.length ? ` (${check.missing_chips.join(", ")}${check.bad_chips > check.missing_chips.length ? "…" : ""})` : "";
    lines.push(t("arcade.badChips", { bad: check.bad_chips, total: check.chips }) + names);
  }
  return lines;
}

/** Texto completo de un set en un núcleo (para el título de una celda o la ficha) */
export function describeCheck(check: SetCheck, t: T): string {
  const lines = [t(`arcade.state_${check.state}` as TranslationKeys), ...checkProblems(check, t)];
  if (check.driver && check.state !== "absent") lines.push(t(`arcade.driver_${check.driver}` as TranslationKeys));
  return lines.join("\n");
}

/**
 * Antes de Jugar (arcade): el diagnóstico del juego si su set no está completo para el emulador
 * que se va a usar. null si está bien, si no se puede saber (sin DAT de ese núcleo, comando
 * propio…) o si falla la comprobación: entonces se juega sin más.
 */
export async function arcadeProblem(systemPath: string, gamePath: string): Promise<ArcadeGame | null> {
  try {
    const report = await arcadeCheck(systemPath, [gamePath]);
    const game = report.games[0];
    const current = game?.current_core ? game.checks[game.current_core] : undefined;
    if (!current || current.state === "ok" || current.state === "not_a_game") return null;
    return game;
  } catch {
    return null;
  }
}
