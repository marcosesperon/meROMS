import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import { scrapeMissText } from "../../utils/scrapeMiss";

/**
 * La marca de «no encontrado por el scraper» de un juego, en su ficha:
 * - marcado (`show` "note" o "both"): aviso ámbar «No encontrado por el scraper el …» con «Quitar marca»;
 * - sin marcar (`show` "mark" o "both"): un enlace discreto para ponerla a mano (juego dado por
 *   imposible), así Scrapear todo y Poner en orden se lo saltan.
 * La marca la ponen y quitan solos los scrapeos; esto es para cambiarla a mano.
 */
export function ScrapeMissNote({ gamePath, show = "both", className = "" }: { gamePath: string; show?: "note" | "mark" | "both"; className?: string }) {
  const { t, locale } = useTranslation();
  const miss = useAppStore((s) => s.scrapeMisses.get(gamePath));
  const setScrapeMisses = useAppStore((s) => s.setScrapeMisses);

  if (miss) {
    if (show === "mark") return null;
    return (
      <div className={`flex items-center gap-2 px-3 py-2 rounded-md border border-amber-600/40 bg-amber-500/10 text-xs ${className}`} data-shot="scrape-miss">
        <span className="shrink-0 font-bold text-amber-400">?</span>
        <span className="flex-1 min-w-0 text-amber-200">{scrapeMissText(miss, locale)}</span>
        <button onClick={() => setScrapeMisses([gamePath], false)} className="shrink-0 text-indigo-400 hover:text-indigo-300">
          {t("scrapeMiss.unmark")}
        </button>
      </div>
    );
  }
  if (show === "note") return null;
  return (
    <button onClick={() => setScrapeMisses([gamePath], true)} className={`text-[11px] text-zinc-500 hover:text-indigo-300 ${className}`} title={t("scrapeMiss.markHint")}>
      {t("scrapeMiss.mark")}
    </button>
  );
}
