import { askChoice } from "../components/common/ChoiceDialog";
import { translate } from "../i18n";
import type { TranslationKeys } from "../i18n/en";
import { useAppStore } from "../store/useAppStore";
import type { RenameEntry } from "../types";
import { formatReleaseDate } from "./format";
import { resolveImageSrc } from "./images";
import { nameSourceText, type RenamePlan } from "./renameSources";

/** Antes → después: lo de antes tachado y gris, lo nuevo en verde */
function Change({ label, from, to }: { label: string; from: string; to: string }) {
  return (
    <div className="min-w-0">
      <p className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold">{label}</p>
      <p className="text-xs text-zinc-500 line-through break-words">{from || "—"}</p>
      <p className="text-sm text-emerald-300 break-words">{to}</p>
    </div>
  );
}

/**
 * Tras scrapear un juego, la propuesta de renombrarlo en una ventana de meROMS (no el aviso del
 * sistema): la carátula, el fichero y el título de antes y de después, de dónde sale el nombre
 * nuevo y lo que acaba de traer el scraper (desarrollador, año, género, jugadores, descripción).
 * Devuelve si se ha pulsado Renombrar.
 */
export async function askRename(systemPath: string, gamePath: string, entry: RenameEntry, plan: RenamePlan): Promise<boolean> {
  const game = useAppStore.getState().games.find((g) => g.path === gamePath);
  const cover = resolveImageSrc(game?.image || game?.boxart || game?.thumbnail, systemPath);
  const fileChange = plan.fileChanges && !plan.blocked;
  const year = formatReleaseDate(game?.releasedate).slice(0, 4);
  const facts = [game?.developer, year, game?.genre, game?.players && translate("rename.offerPlayers", { count: game.players })].filter(Boolean).join(" · ");
  const source = entry.source === "current" ? null : translate(nameSourceText(entry.source).label as TranslationKeys);

  const choice = await askChoice({
    title: translate("rename.offerHeading"),
    body: (
      <div className="flex gap-4">
        <div className="w-28 shrink-0">
          {cover ? (
            <img src={cover} alt="" className="w-full max-h-40 object-contain rounded shadow-lg shadow-black/50" />
          ) : (
            <div className="w-full h-36 rounded bg-zinc-900" />
          )}
        </div>
        <div className="flex-1 min-w-0 space-y-3">
          <p className="text-xs text-zinc-400">{translate("rename.offerTitle")}</p>
          {fileChange && <Change label={translate("rename.offerFileLabel")} from={entry.current_name} to={plan.file} />}
          {plan.titleChanges && <Change label={translate("rename.offerTitleLabel")} from={entry.game_title ?? ""} to={plan.title} />}
          {source && <p className="text-[11px] text-zinc-500">{translate("rename.offerSource", { source })}</p>}
          {plan.fileChanges && plan.blocked && <p className="text-xs text-amber-300">{translate("rename.offerClash", { name: plan.file })}</p>}
          {/* Lo que acaba de traer el scraper */}
          {(facts || game?.desc) && (
            <div className="pt-2 border-t border-zinc-700">
              <p className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold">{translate("rename.offerScraped")}</p>
              {facts && <p className="text-xs text-zinc-300 mt-0.5">{facts}</p>}
              {game?.desc && <p className="text-xs text-zinc-400 mt-1 line-clamp-3">{game.desc}</p>}
            </div>
          )}
        </div>
      </div>
    ),
    options: [
      { id: "no", label: translate("rename.offerNo") },
      { id: "yes", label: translate("rename.offerYes"), primary: true },
    ],
  });
  return choice === "yes";
}
