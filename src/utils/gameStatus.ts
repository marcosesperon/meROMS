import type { GameStatus, PlayStatus } from "../commands";
import type { TranslationKeys } from "../i18n/en";
import {
  BanIcon,
  CircleHalfIcon,
  ClockIcon,
  FlagIcon,
  FlagOffIcon,
  GamepadIcon,
  TrophyIcon,
} from "../components/common/Icons";

type Icon = typeof FlagIcon;

/** Cada estado de partida: icono, texto y color (en el menú, la tarjeta y la tabla) */
export const STATUS_META: Record<PlayStatus, { icon: Icon; label: TranslationKeys; color: string }> = {
  incomplete: { icon: CircleHalfIcon, label: "status.incomplete", color: "text-sky-300" },
  finished: { icon: FlagIcon, label: "status.finished", color: "text-emerald-400" },
  completed: { icon: TrophyIcon, label: "status.completed", color: "text-yellow-400" },
  abandoned: { icon: FlagOffIcon, label: "status.abandoned", color: "text-orange-400" },
  never: { icon: BanIcon, label: "status.never", color: "text-zinc-400" },
};

/** Las dos marcas sueltas */
export const STATUS_FLAGS: { key: "playing" | "backlog"; icon: Icon; label: TranslationKeys; color: string }[] = [
  { key: "playing", icon: GamepadIcon, label: "status.playing", color: "text-indigo-300" },
  { key: "backlog", icon: ClockIcon, label: "status.backlog", color: "text-zinc-300" },
];

/**
 * El icono que representa el estado de un juego en una sola marca (tarjeta, botón): el estado
 * de partida si lo tiene; si no, «Jugando» y después «Pendiente».
 */
export function statusBadge(status?: GameStatus): { icon: Icon; label: TranslationKeys; color: string } | null {
  if (status?.status) return STATUS_META[status.status];
  return STATUS_FLAGS.find((f) => status?.[f.key]) ?? null;
}

/** Todo el estado en texto: «Terminado · Jugando» (vacío si no tiene) */
export function statusText(status: GameStatus | undefined, t: (key: TranslationKeys) => string): string {
  if (!status) return "";
  return [status.status ? t(STATUS_META[status.status].label) : "", ...STATUS_FLAGS.filter((f) => status[f.key]).map((f) => t(f.label))]
    .filter(Boolean)
    .join(" · ");
}
