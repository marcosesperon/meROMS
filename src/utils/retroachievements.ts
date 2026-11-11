import type { RaMark } from "../commands";

/**
 * Cómo se enseña la marca de RetroAchievements de un juego (tarjeta y tabla): dorado si la ROM es
 * compatible, gris si el juego tiene logros pero esta versión no; con el progreso si hay.
 * `null` = nada que enseñar (sin logros, sin comprobar o sistema de disco).
 */
export function raBadge(mark?: RaMark): { color: string; percent: number | null; compatible: boolean } | null {
  if (!mark || (mark.status !== "compatible" && mark.status !== "other")) return null;
  const compatible = mark.status === "compatible";
  const percent = mark.achievements > 0 && mark.awarded > 0 ? Math.round((mark.awarded / mark.achievements) * 100) : null;
  return { color: compatible ? "text-amber-300" : "text-zinc-400", percent, compatible };
}
