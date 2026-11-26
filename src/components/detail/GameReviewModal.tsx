import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { findDamagedMedia, findGameLeftoverMedia, planMediaNormalization, renamePreview } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import type { Game } from "../../types";
import { collectionRulesFrom, DATA_FIELDS } from "../../utils/collectionRules";
import { hasNotGameData, missingData, missingMedia } from "../../utils/collectionStatus";
import { isBadReleaseDate } from "../../utils/format";
import { MEDIA_FILTER_FIELDS } from "../../utils/mediaFilter";
import { getActiveProfile } from "../../utils/profiles";
import { planRename, previewRequest, renameOptionsFrom } from "../../utils/renameSources";
import { CheckIcon } from "../common/Icons";

/** Lo que «Arreglar» abre en el editor: las herramientas de un juego de siempre */
export type ReviewFix = "scrape" | "rename" | "normalize" | "clean" | "versions";

interface Problem {
  key: string;
  text: string;
  detail?: string;
  fix: ReviewFix;
}

interface GameReviewModalProps {
  systemPath: string;
  game: Game;
  onClose: () => void;
  onFix: (fix: ReviewFix) => void;
}

/**
 * «Revisar este juego» (editor): el estado del sistema, pero de un solo juego. Qué le falta según
 * «Mi colección» (datos, medios), si su fichero o su título están fuera de formato, si tiene otras
 * versiones, medios dañados o sobrantes, o con nombre antiguo. Cada cosa con su «Arreglar», que
 * abre la herramienta de un juego de siempre (Scrapear, Renombrar, Normalizar, Limpiar…).
 */
export function GameReviewModal({ systemPath, game, onClose, onFix }: GameReviewModalProps) {
  const { t } = useTranslation();
  const settings = useAppStore((s) => s.settings);
  const broken = useAppStore((s) => s.brokenMedia);
  const versions = useAppStore((s) => s.collectionStatus?.parts.versions);
  const missingRom = useAppStore((s) => s.missingRomPaths.has(game.path));
  const scraperStatus = useAppStore((s) => s.scraperStatus);
  const [slow, setSlow] = useState<Problem[] | null>(null);

  const rules = collectionRulesFrom(settings.collection_rules);
  const isEsde = getActiveProfile(settings)?.kind === "esde";
  const labelOfData = (f: string) => t(DATA_FIELDS.find((d) => d.field === f)?.label ?? ("form.description" as TranslationKeys));
  const labelOfMedia = (f: string) => t(MEDIA_FILTER_FIELDS.find((m) => m.field === f)?.label ?? ("filter.media.image" as TranslationKeys));

  // Lo inmediato: datos y medios que faltan, versiones (del estado del sistema) y sin ROM
  const quick: Problem[] = [];
  if (hasNotGameData(game)) quick.push({ key: "invalid", text: t("review.invalid"), detail: game.name, fix: "scrape" });
  // La fecha en otro formato: se arregla al guardar el editor (el campo ya la enseña bien)
  if (isBadReleaseDate(game.releasedate)) quick.push({ key: "date", text: t("review.badDate"), detail: game.releasedate, fix: "scrape" });
  const data = missingData(game, rules);
  if (data.length) quick.push({ key: "data", text: t("review.missingData"), detail: data.map(labelOfData).join(", "), fix: "scrape" });
  const media = missingMedia(game, rules, broken, isEsde);
  if (media.length) quick.push({ key: "media", text: t("review.missingMedia"), detail: media.map((f) => labelOfMedia(f as string)).join(", "), fix: "scrape" });
  const group = versions?.find((g) => g.includes(game.path));
  if (group) quick.push({ key: "versions", text: t("review.versions", { count: group.length - 1 }), fix: "versions" });

  // Lo que hay que preguntar al backend: nombre, medios dañados, sobrantes y nombres antiguos
  useEffect(() => {
    let cancelled = false;
    const options = renameOptionsFrom(settings.rename_options);
    const request = previewRequest(options, scraperStatus);
    Promise.all([
      renamePreview(systemPath, request, [game.path], false).catch(() => []),
      findDamagedMedia(systemPath, game.path).catch(() => []),
      findGameLeftoverMedia(systemPath, game.path).catch(() => []),
      planMediaNormalization(systemPath, game.path).catch(() => []),
    ]).then(([entries, damaged, leftovers, legacy]) => {
      if (cancelled) return;
      const found: Problem[] = [];
      const plan = entries[0] ? planRename(entries[0], options) : null;
      if (plan?.fileChanges && !plan.blocked) found.push({ key: "file", text: t("review.fileName"), detail: `${plan.entry.current_name} → ${plan.file}`, fix: "rename" });
      if (plan?.titleChanges) found.push({ key: "title", text: t("review.title"), detail: `${plan.entry.game_title ?? ""} → ${plan.title}`, fix: "rename" });
      if (damaged.length) found.push({ key: "damaged", text: t("review.damaged", { count: damaged.length }), detail: damaged.map((d) => labelOfMedia(d.field)).join(", "), fix: "clean" });
      if (leftovers.length) found.push({ key: "leftovers", text: t("review.leftovers", { count: leftovers.length }), fix: "clean" });
      const movable = legacy.filter((i) => i.status !== "conflict");
      if (movable.length) found.push({ key: "legacy", text: t("review.legacy", { count: movable.length }), fix: "normalize" });
      setSlow(found);
    });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [systemPath, game.path]);

  const problems = [...quick, ...(slow ?? [])];

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-[60]" data-shot="game-review">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[560px] max-w-[calc(100vw-2rem)] max-h-[80vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("review.heading", { name: game.name || game.path })}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("review.help")}</p>
        </div>
        <div className="flex-1 overflow-y-auto px-5 py-3 space-y-2">
          {missingRom && <p className="text-sm text-red-300">{t("review.noRom")}</p>}
          {problems.map((p) => (
            <div key={p.key} className="flex items-start gap-3 rounded bg-zinc-900/50 px-3 py-2">
              <div className="flex-1 min-w-0">
                <p className="text-sm text-zinc-200">{p.text}</p>
                {p.detail && (
                  <p className="text-xs text-zinc-500 truncate" title={p.detail}>
                    {p.detail}
                  </p>
                )}
              </div>
              <button onClick={() => onFix(p.fix)} className="shrink-0 px-2.5 py-1 text-xs bg-indigo-600 text-white rounded hover:bg-indigo-500">
                {t(`review.fix_${p.fix}` as TranslationKeys)}
              </button>
            </div>
          ))}
          {slow === null && <p className="text-xs text-zinc-500">{t("colstatus.checking")}</p>}
          {slow !== null && problems.length === 0 && !missingRom && (
            <p className="flex items-center gap-1.5 text-sm text-emerald-400 py-4">
              <CheckIcon size={14} />
              {t("review.allGood")}
            </p>
          )}
        </div>
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-end">
          <button onClick={onClose} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
            {t("rename.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
