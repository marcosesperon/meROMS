import { useTranslation } from "../../i18n";
import { useUpdateStore } from "../../store/useUpdateStore";
import { CloseIcon } from "./Icons";

/** Banda de aviso arriba de la lista cuando hay una versión nueva; la × la oculta hasta el siguiente arranque */
export function UpdateBanner() {
  const { t } = useTranslation();
  const update = useUpdateStore((s) => s.update);
  const dismissed = useUpdateStore((s) => s.bannerDismissed);
  const openModal = useUpdateStore((s) => s.openModal);
  const dismissBanner = useUpdateStore((s) => s.dismissBanner);

  if (!update || dismissed) return null;

  return (
    <div className="px-6 py-2 shrink-0 border-b border-indigo-700/50 bg-indigo-600/20 flex items-center gap-3">
      <p className="flex-1 text-sm text-indigo-200">{t("update.banner", { version: update.version })}</p>
      <button
        onClick={openModal}
        className="shrink-0 px-3 py-1 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
      >
        {t("update.view")}
      </button>
      <button onClick={dismissBanner} title={t("update.dismiss")} className="shrink-0 p-1 text-indigo-300 hover:text-white">
        <CloseIcon size={16} />
      </button>
    </div>
  );
}
