import { useEffect } from "react";
import { useTranslation } from "../../i18n";
import { AUTHOR, DONATION_URL, WEBSITE_URL, openExternal } from "../../utils/links";
import { CoffeeIcon } from "./Icons";
import appIcon from "../../assets/app-icon.png";

interface DonationModalProps {
  open: boolean;
  onClose: () => void;
}

/** Ventana que pide apoyo al proyecto (la abre App como mucho una vez a la semana) */
export function DonationModal({ open, onClose }: DonationModalProps) {
  const { t } = useTranslation();

  // Esc = «Ahora no»
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onClick={onClose}>
      <div
        className="bg-zinc-850 border border-zinc-700 rounded-xl p-7 max-w-sm w-full mx-4 text-center shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <img src={appIcon} alt="" className="w-16 h-16 mx-auto mb-4 rounded-2xl" />
        <h3 className="text-lg font-semibold text-zinc-100 mb-3">{t("donation.title")}</h3>
        <p className="text-sm text-zinc-400 mb-2">{t("donation.text1")}</p>
        <p className="text-sm text-zinc-400 mb-4">{t("donation.text2")}</p>
        <p className="text-sm font-semibold text-zinc-200">{t("donation.thanks")}</p>
        <p className="text-xs text-zinc-500">— {AUTHOR}</p>
        <button onClick={() => openExternal(WEBSITE_URL)} className="text-xs text-indigo-400 hover:text-indigo-300 mb-6">
          {WEBSITE_URL.replace("https://", "").replace(/\/$/, "")}
        </button>
        <div className="flex justify-center gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("donation.later")}
          </button>
          <button
            autoFocus
            onClick={() => {
              openExternal(DONATION_URL);
              onClose();
            }}
            className="inline-flex items-center gap-2 px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
          >
            <CoffeeIcon size={15} />
            {t("donation.donate")}
          </button>
        </div>
      </div>
    </div>
  );
}
