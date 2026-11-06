import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import { PLAY_STATUSES, type StatusChange } from "../../commands";
import type { Game } from "../../types";
import { STATUS_FLAGS, STATUS_META, statusBadge } from "../../utils/gameStatus";
import { showMessage } from "../../utils/dialogs";
import { CheckIcon, CircleDashedIcon, EyeSlashIcon } from "./Icons";

interface StatusMenuProps {
  game: Game;
  /** Clases del botón (para que case con los de al lado) */
  buttonClassName?: string;
}

/**
 * Botón con menú de estados del juego (como el de RomM): estado de partida (uno, o ninguno),
 * las marcas «Jugando» y «Pendiente», y «Oculto» (la marca de la gamelist de siempre).
 * Se cierra al hacer clic fuera o con Esc; los cambios se guardan en el acto.
 */
export function StatusMenu({ game, buttonClassName = "" }: StatusMenuProps) {
  const { t } = useTranslation();
  const status = useAppStore((s) => s.statuses.get(game.path));
  const setStatuses = useAppStore((s) => s.setStatuses);
  const toggleGameFlag = useAppStore((s) => s.toggleGameFlag);
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    // Esc cierra solo el menú (la ficha a pantalla completa cede si hay un [data-popup-open])
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  async function change(c: StatusChange) {
    try {
      await setStatuses([game.path], c);
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  const badge = statusBadge(status);
  const BadgeIcon = badge?.icon ?? CircleDashedIcon;
  const hidden = game.hidden === "true";
  const item = "w-full flex items-center gap-2.5 px-3 py-1.5 text-sm text-left hover:bg-zinc-700/70";

  return (
    // flex: el botón se estira al alto de la fila, como sus vecinos (Favorito, Borrar…)
    <div ref={rootRef} className="relative flex">
      <button
        onClick={() => setOpen((v) => !v)}
        data-shot="status-menu"
        title={badge ? t(badge.label) : t("status.button")}
        className={`${buttonClassName} ${badge ? badge.color : "text-zinc-400 hover:text-zinc-200"}`}
      >
        <BadgeIcon size={14} />
      </button>
      {open && (
        <div data-popup-open className="absolute right-0 top-full mt-2 z-30 min-w-[210px] py-1 bg-zinc-800 border border-zinc-600 rounded-lg shadow-xl">
          {/* Estado de partida: uno solo; pulsar el que está puesto lo quita */}
          {PLAY_STATUSES.map((s) => {
            const meta = STATUS_META[s];
            const active = status?.status === s;
            return (
              <button key={s} onClick={() => change({ status: active ? null : s })} className={`${item} ${active ? "text-zinc-100" : "text-zinc-300"}`}>
                <meta.icon size={15} className={meta.color} />
                <span className="flex-1">{t(meta.label)}</span>
                {active && <CheckIcon size={14} className="text-indigo-300" />}
              </button>
            );
          })}
          <div className="my-1 border-t border-zinc-700" />
          {/* Marcas sueltas, compatibles con cualquier estado */}
          {STATUS_FLAGS.map((f) => {
            const active = !!status?.[f.key];
            return (
              <button key={f.key} onClick={() => change({ [f.key]: !active })} className={`${item} ${active ? "text-zinc-100" : "text-zinc-300"}`}>
                <f.icon size={15} className={f.color} />
                <span className="flex-1">{t(f.label)}</span>
                {active && <CheckIcon size={14} className="text-indigo-300" />}
              </button>
            );
          })}
          <div className="my-1 border-t border-zinc-700" />
          <button onClick={() => toggleGameFlag(game.path, "hidden")} className={`${item} ${hidden ? "text-zinc-100" : "text-zinc-300"}`}>
            <EyeSlashIcon size={15} className="text-zinc-400" />
            <span className="flex-1">{t("status.hidden")}</span>
            {hidden && <CheckIcon size={14} className="text-indigo-300" />}
          </button>
        </div>
      )}
    </div>
  );
}
