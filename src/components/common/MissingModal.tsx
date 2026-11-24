import { useMemo } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { useAppStore } from "../../store/useAppStore";
import type { Game } from "../../types";
import { collectionRulesFrom, DATA_FIELDS } from "../../utils/collectionRules";
import { hasNotGameData, missingData, missingMedia } from "../../utils/collectionStatus";
import { isBadReleaseDate } from "../../utils/format";
import { MEDIA_FILTER_FIELDS } from "../../utils/mediaFilter";
import { getActiveProfile } from "../../utils/profiles";

interface MissingModalProps {
  open: boolean;
  /** Qué se mira: los datos obligatorios, los medios obligatorios o los datos no válidos del scraper */
  kind: "data" | "media" | "invalid";
  onClose: () => void;
}

/** Juegos que se enseñan en la lista de abajo (el resto se cuenta, pero no se pinta) */
const MAX_ROWS = 300;

/**
 * «Ver» de las líneas «sin datos obligatorios» y «sin medios obligatorios» del estado del
 * sistema: cuántos juegos no tienen cada campo y, debajo, qué le falta a cada juego (un clic lo
 * selecciona). Desde aquí se puede filtrar la lista o buscar lo que falta (Ponerlo todo en orden
 * con el paso de scrapear).
 */
export function MissingModal({ open, kind, onClose }: MissingModalProps) {
  const { t } = useTranslation();
  const games = useAppStore((s) => s.games);
  const settings = useAppStore((s) => s.settings);
  const broken = useAppStore((s) => s.brokenMedia);
  const setFilters = useAppStore((s) => s.setFilters);
  const openTool = useAppStore((s) => s.openTool);
  const selectGame = useAppStore((s) => s.selectGame);

  const rules = collectionRulesFrom(settings.collection_rules);
  const isEsde = getActiveProfile(settings)?.kind === "esde";
  const labelOf = (field: string) =>
    t(
      (kind === "invalid"
        ? (`missing.invalid_${field}` as TranslationKeys)
        : kind === "data"
          ? DATA_FIELDS.find((d) => d.field === field)?.label
          : MEDIA_FILTER_FIELDS.find((m) => m.field === field)?.label) ?? (field as TranslationKeys)
    );
  // Datos no válidos del scraper: el título «no es un juego» y la fecha en otro formato
  const invalidOf = (game: Game): string[] => [
    ...(hasNotGameData(game) ? ["title"] : []),
    ...(isBadReleaseDate(game.releasedate) ? ["date"] : []),
  ];
  // Con los no válidos se enseña además lo que hay guardado («ZZZ(notgame):…», «1988-07-01»)
  const shownValue = (game: Game, field: string) => (field === "title" ? game.name : field === "date" ? game.releasedate : undefined);

  // Lo que le falta a cada juego (solo los que les falta algo)
  const rows = useMemo(() => {
    if (!open) return [] as { game: Game; missing: string[] }[];
    return games
      .map((game) => ({
        game,
        missing: (kind === "invalid" ? invalidOf(game) : kind === "data" ? missingData(game, rules) : missingMedia(game, rules, broken, isEsde)) as string[],
      }))
      .filter((r) => r.missing.length > 0)
      .sort((a, b) => b.missing.length - a.missing.length || (a.game.name ?? a.game.path).localeCompare(b.game.name ?? b.game.path));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, games, kind, settings.collection_rules, broken, isEsde]);

  if (!open) return null;

  // Cuántos juegos no tienen cada campo, en el orden de las reglas
  const fields =
    kind === "invalid"
      ? ["title", "date"]
      : kind === "data"
        ? rules.required_data
        : rules.required_media.filter((f) => !(isEsde && MEDIA_FILTER_FIELDS.find((m) => m.field === f)?.classicOnly));
  const perField = fields.map((f) => ({ field: f, count: rows.filter((r) => r.missing.includes(f)).length }));

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[620px] max-w-[calc(100vw-2rem)] max-h-[85vh] flex flex-col mx-4" data-shot="missing-modal">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t(kind === "invalid" ? "missing.titleInvalid" : kind === "data" ? "missing.titleData" : "missing.titleMedia")}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t(kind === "invalid" ? "missing.helpInvalid" : "missing.help", { count: rows.length, total: games.length })}</p>
          {/* Una barra por campo: cuántos juegos no lo tienen */}
          <div className="mt-3 space-y-1.5">
            {perField.map(({ field, count }) => (
              <div key={field} className="grid grid-cols-[130px_1fr_70px] items-center gap-2 text-xs">
                <span className="text-zinc-300 truncate">{labelOf(field)}</span>
                <span className="h-2 rounded bg-zinc-900 overflow-hidden">
                  <span className="block h-full bg-amber-500/70" style={{ width: `${games.length ? (count / games.length) * 100 : 0}%` }} />
                </span>
                <span className="text-zinc-400 tabular-nums text-right">{t("missing.games", { count })}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="flex-1 overflow-y-auto divide-y divide-zinc-700/30">
          {rows.length === 0 ? (
            <p className="py-12 text-center text-sm text-emerald-400">{t("missing.none")}</p>
          ) : (
            rows.slice(0, MAX_ROWS).map(({ game, missing }) => (
              <button
                key={game.path}
                onClick={() => {
                  selectGame(game);
                  onClose();
                }}
                className="w-full text-left px-5 py-2 hover:bg-zinc-700/30"
              >
                <p className="text-sm text-zinc-200 truncate">{game.name || game.path}</p>
                <p className="text-xs text-amber-400/80 truncate">{missing.map((f) => (kind === "invalid" ? `${labelOf(f)}: ${shownValue(game, f) ?? ""}` : labelOf(f))).join(" · ")}</p>
              </button>
            ))
          )}
          {rows.length > MAX_ROWS && <p className="px-5 py-2 text-xs text-zinc-500">{t("missing.more", { count: rows.length - MAX_ROWS })}</p>}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between gap-2">
          <button onClick={onClose} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
            {t("rename.close")}
          </button>
          {rows.length > 0 && (
            <div className="flex gap-2">
              <button
                onClick={() => {
                  setFilters({ issue: kind });
                  onClose();
                }}
                className="px-4 py-2 text-sm bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600"
              >
                {t("missing.filter")}
              </button>
              <button
                // Medios: antes de scrapear, colocar los que ya están con otro nombre (Limpiar medios)
                onClick={() => openTool({ id: "order", steps: kind === "media" ? ["media", "scrape"] : ["scrape"] })}
                className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500"
              >
                {t(kind === "invalid" ? "missing.fixInvalid" : "missing.fix")}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
