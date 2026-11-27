import type { ReactNode } from "react";
import { useActionBusy } from "../../hooks/useBlockClose";
import { useTranslation } from "../../i18n";

const CLOSE_STYLE = "px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-50 disabled:cursor-not-allowed";

/**
 * El «Cerrar» de las ventanas de herramientas: se desactiva solo mientras una acción está en
 * marcha (`hooks/useBlockClose`), para que no se pueda salir hasta que termine.
 */
export function CloseButton({ onClick, disabled, className, children }: { onClick: () => void; disabled?: boolean; className?: string; children?: ReactNode }) {
  const { t } = useTranslation();
  const busy = useActionBusy();
  return (
    <button onClick={onClick} disabled={busy || disabled} className={className ?? CLOSE_STYLE}>
      {children ?? t("cleanup.close")}
    </button>
  );
}
