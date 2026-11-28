import { useEffect, useState } from "react";
import { datProgress, type DatProgress } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";

/**
 * «Colección» del resumen del sistema: cuántos juegos del DAT tienes (cada juego una vez, sea
 * cual sea la versión; sin betas, demos ni hacks), con su barra. Solo si el sistema tiene DAT.
 * Un clic abre la ventana DAT, que empieza en la pestaña «Faltan».
 */
export function DatProgressBlock({ systemPath }: { systemPath: string }) {
  const { t } = useTranslation();
  const games = useAppStore((s) => s.games);
  // Al cerrar una herramienta (la del DAT, sobre todo) se vuelve a pedir: el backend lo guarda,
  // así que si no ha cambiado nada la respuesta es inmediata
  const toolOpen = useAppStore((s) => s.tool !== null);
  const openTool = useAppStore((s) => s.openTool);
  const [progress, setProgress] = useState<DatProgress | null>(null);

  useEffect(() => {
    if (toolOpen) return;
    let cancelled = false;
    datProgress(systemPath)
      .then((p) => !cancelled && setProgress(p))
      .catch(() => !cancelled && setProgress(null));
    return () => {
      cancelled = true;
    };
  }, [systemPath, games, toolOpen]);

  if (!progress || progress.total === 0) return null;
  const missing = progress.total - progress.have;
  const percent = Math.floor((progress.have / progress.total) * 100);
  const complete = missing === 0;
  // Sin ninguna ROM reconocida por contenido, el cruce ha sido por nombre de fichero
  const byName = progress.has_crcs && progress.content_matched === 0;

  return (
    <button
      onClick={() => openTool("dat")}
      title={t("dat.progressHelp", { name: progress.dat })}
      className="group mt-6 block w-full text-left"
    >
      <div className="flex items-baseline justify-between mb-2">
        <h3 className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold">{t("dat.progressTitle")}</h3>
        <span className={`text-sm font-semibold ${complete ? "text-emerald-400" : "text-zinc-100"}`}>{percent} %</span>
      </div>
      <div className="h-2 rounded-full bg-zinc-800 overflow-hidden">
        <div
          className={`h-full rounded-full ${complete ? "bg-emerald-500" : "bg-indigo-500"}`}
          style={{ width: `${(progress.have / progress.total) * 100}%` }}
        />
      </div>
      <p className="mt-1.5 text-xs text-zinc-400 group-hover:text-zinc-200">
        {t("dat.progressLine", { have: progress.have, total: progress.total })}
        {!complete && <span className="text-amber-300"> · {t("dat.progressMissing", { count: missing })}</span>}
        <span className="text-zinc-500 group-hover:text-indigo-300"> ›</span>
      </p>
      {byName && <p className="mt-1 text-[11px] text-zinc-500">{t("dat.progressByName")}</p>}
    </button>
  );
}
