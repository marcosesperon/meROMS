import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { getScrapePolicy, getScrapeResume } from "../../commands";
import type { ScrapePolicy, ScrapeResumeInfo } from "../../types";
import { ScrapePolicyEditor } from "./ScrapePolicyEditor";

interface ScrapeAllModalProps {
  open: boolean;
  gameCount: number;
  systemPath: string;
  onClose: () => void;
  /** Start scraping the system with this choice (not saved) */
  onStart: (policy: ScrapePolicy) => void;
  /** Continue the run left halfway (its games and rules) */
  onContinue: () => void;
}

/** Antes de «Scrapear Todo»: qué se actualiza esta vez (empieza con lo de Ajustes, no se guarda) */
export function ScrapeAllModal({ open, gameCount, systemPath, onClose, onStart, onContinue }: ScrapeAllModalProps) {
  const { t, locale } = useTranslation();
  const [policy, setPolicy] = useState<ScrapePolicy | null>(null);
  // Un «Scrapear todo» que se quedó a medias en este sistema (app cerrada, fallo o «Detener»)
  const [pending, setPending] = useState<ScrapeResumeInfo | null>(null);

  useEffect(() => {
    if (!open) {
      setPolicy(null);
      setPending(null);
      return;
    }
    getScrapePolicy().then(setPolicy).catch(() => setPolicy({ data: {}, media: {} }));
    getScrapeResume(systemPath).then(setPending).catch(() => setPending(null));
  }, [open, systemPath]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[720px] max-h-[85vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("scrapePolicy.runTitle")}</h3>
          <p className="text-sm text-zinc-400 mt-1">{t("scrapePolicy.runHelp", { count: gameCount })}</p>
        </div>
        {/* A medias: «Continuar» sigue con sus juegos y sus reglas; «Empezar de nuevo» usa la tabla de abajo */}
        {pending && (
          <div className="mx-5 mt-4 px-4 py-3 rounded border border-amber-600/50 bg-amber-500/10 flex items-center gap-3">
            <p className="flex-1 text-sm text-amber-200">
              {t("scrapePolicy.pending", {
                remaining: pending.remaining,
                total: pending.total,
                date: new Date(pending.started_at * 1000).toLocaleString(locale),
              })}
            </p>
            <button
              onClick={onContinue}
              className="px-4 py-2 text-sm bg-emerald-600 text-white rounded hover:bg-emerald-500 transition-colors"
            >
              {t("scrapePolicy.continue")}
            </button>
          </div>
        )}
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
            {pending ? t("scrapePolicy.startOver") : t("scrapePolicy.start")}
          </button>
        </div>
      </div>
    </div>
  );
}
