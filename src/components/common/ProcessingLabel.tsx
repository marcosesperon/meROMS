import { useTranslation } from "../../i18n";

/** Lo que dice el botón de una acción mientras se aplica: un circulito girando y «Procesando…» */
export function ProcessingLabel() {
  const { t } = useTranslation();
  return (
    <span className="inline-flex items-center gap-2">
      <span className="w-3.5 h-3.5 rounded-full border-2 border-white/40 border-t-white animate-spin" aria-hidden />
      {t("common.processing")}
    </span>
  );
}
