import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { getScrapePolicy } from "../../commands";
import type { ScrapePolicy } from "../../types";
import { ScrapePolicyEditor } from "./ScrapePolicyEditor";

interface ScrapeAllModalProps {
  open: boolean;
  gameCount: number;
  onClose: () => void;
  /** Start scraping the system with this choice (not saved) */
  onStart: (policy: ScrapePolicy) => void;
}

/** Antes de «Scrapear Todo»: qué se actualiza esta vez (empieza con lo de Ajustes, no se guarda) */
export function ScrapeAllModal({ open, gameCount, onClose, onStart }: ScrapeAllModalProps) {
  const { t } = useTranslation();
  const [policy, setPolicy] = useState<ScrapePolicy | null>(null);

  useEffect(() => {
    if (!open) {
      setPolicy(null);
      return;
    }
    getScrapePolicy().then(setPolicy).catch(() => setPolicy({ data: {}, media: {} }));
  }, [open]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[720px] max-h-[85vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("scrapePolicy.runTitle")}</h3>
          <p className="text-sm text-zinc-400 mt-1">{t("scrapePolicy.runHelp", { count: gameCount })}</p>
        </div>
        <div className="flex-1 overflow-y-auto px-5 py-4">
          {policy && <ScrapePolicyEditor policy={policy} onChange={setPolicy} />}
        </div>
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("rename.close")}
          </button>
          <button
            onClick={() => policy && onStart(policy)}
            disabled={!policy}
            className="px-4 py-2 text-sm bg-emerald-600 text-white rounded hover:bg-emerald-500 transition-colors disabled:opacity-50"
          >
            {t("scrapePolicy.start")}
          </button>
        </div>
      </div>
    </div>
  );
}
