import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";

/** Etiqueta «Clon» / «BIOS» de un juego según el DAT del sistema (nada si no hay DAT o es original) */
export function DatTag({ path, className = "" }: { path: string; className?: string }) {
  const { t } = useTranslation();
  const mark = useAppStore((s) => s.datMarks?.get(path));
  if (!mark) return null;
  const label = mark.kind === "clone" ? t("dat.clone") : mark.kind === "bios" ? "BIOS" : t("dat.device");
  return (
    <span
      title={mark.parent ? t("dat.cloneOf", { parent: mark.parent }) : undefined}
      className={`px-1.5 py-0.5 rounded text-[10px] font-semibold bg-sky-600/80 text-white ${className}`}
    >
      {label}
    </span>
  );
}
