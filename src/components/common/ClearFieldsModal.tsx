import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { backupLabeled, saveGamelist } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import type { Game } from "../../types";
import { mediaAction, removeMedia } from "../../utils/collectionActions";
import { collectionRulesFrom } from "../../utils/collectionRules";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { absoluteMediaPath, bumpMediaVersion } from "../../utils/images";
import { hasMedia, MEDIA_FILTER_FIELDS } from "../../utils/mediaFilter";
import { getActiveProfile } from "../../utils/profiles";
import { reportWithUndo } from "../../utils/undo";
import { useBlockClose } from "../../hooks/useBlockClose";
import { CloseButton } from "./CloseButton";
import { ProcessingLabel } from "./ProcessingLabel";

interface ClearFieldsModalProps {
  open: boolean;
  systemPath: string;
  /** Lo que la lista enseña ahora y lo marcado en la selección múltiple */
  visiblePaths: string[];
  markedPaths: string[];
  /** Ámbito con el que se abre (desde el panel de selección múltiple: los marcados) */
  initialScope?: Scope;
  onClose: () => void;
}

type Scope = "all" | "visible" | "marked";

/** Un dato que se puede vaciar: sus campos de la gamelist (el género va con su id) y su texto */
interface DataItem {
  id: string;
  fields: (keyof Game)[];
  label: TranslationKeys;
}

const DATA_ITEMS: DataItem[] = [
  { id: "name", fields: ["name"], label: "form.name" },
  { id: "sortname", fields: ["sortname"], label: "form.sortname" },
  { id: "desc", fields: ["desc"], label: "form.description" },
  { id: "genre", fields: ["genre", "genreid"], label: "form.genre" },
  { id: "developer", fields: ["developer"], label: "form.developer" },
  { id: "publisher", fields: ["publisher"], label: "form.publisher" },
  { id: "releasedate", fields: ["releasedate"], label: "form.releaseDate" },
  { id: "players", fields: ["players"], label: "form.players" },
  { id: "rating", fields: ["rating"], label: "form.rating" },
  { id: "region", fields: ["region"], label: "form.region" },
  { id: "lang", fields: ["lang"], label: "form.lang" },
  { id: "family", fields: ["family"], label: "form.family" },
  { id: "arcadesystemname", fields: ["arcadesystemname"], label: "form.arcadesystemname" },
];

const FLAG_ITEMS: DataItem[] = [
  { id: "favorite", fields: ["favorite"], label: "form.favorite" },
  { id: "hidden", fields: ["hidden"], label: "form.hidden" },
  { id: "kidgame", fields: ["kidgame"], label: "form.kidgame" },
  // Partidas, última vez y tiempo jugado van juntos
  { id: "stats", fields: ["playcount", "lastplayed", "gametime"], label: "clear.stats" },
];

const filled = (g: Game, fields: (keyof Game)[]) => fields.some((f) => String(g[f] ?? "").trim() !== "");

/**
 * «Vaciar campos»: quitar un dato (título, descripción, género…), una marca, las estadísticas,
 * el estado de juego o un medio (carátula, vídeo, manual…) de todo el sistema, de los juegos que
 * se ven o de los marcados. Cada casilla dice a cuántos juegos afecta. Los medios van a la
 * papelera (o a la carpeta de apartados, según «Mi colección»); en la estructura clásica se puede
 * quitar solo la ruta y dejar el fichero. Antes se guarda una copia de la gamelist con etiqueta.
 */
export function ClearFieldsModal({ open, systemPath, visiblePaths, markedPaths, initialScope, onClose }: ClearFieldsModalProps) {
  const { t } = useTranslation();
  const games = useAppStore((s) => s.games);
  const broken = useAppStore((s) => s.brokenMedia);
  const statuses = useAppStore((s) => s.statuses);
  const settings = useAppStore((s) => s.settings);
  const isEsde = getActiveProfile(settings)?.kind === "esde";
  const [scope, setScope] = useState<Scope>("all");
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [keepFiles, setKeepFiles] = useState(false);
  const [busy, setBusy] = useState(false);
  // Mientras se vacía no se puede salir de la ventana
  useBlockClose(busy);

  useEffect(() => {
    if (!open) return;
    setChecked(new Set());
    setKeepFiles(false);
    setScope(initialScope === "marked" && markedPaths.length > 0 ? "marked" : "all");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  // Los juegos del ámbito
  const target = useMemo(() => {
    if (scope === "all") return games;
    const paths = new Set(scope === "visible" ? visiblePaths : markedPaths);
    return games.filter((g) => paths.has(g.path));
  }, [games, scope, visiblePaths, markedPaths]);

  const mediaItems = MEDIA_FILTER_FIELDS.filter((m) => !(isEsde && m.classicOnly));
  // A cuántos juegos del ámbito afecta cada casilla
  const counts = useMemo(() => {
    const out: Record<string, number> = {};
    for (const item of [...DATA_ITEMS, ...FLAG_ITEMS]) out[item.id] = target.filter((g) => filled(g, item.fields)).length;
    for (const m of mediaItems) out[`media:${m.field}`] = target.filter((g) => hasMedia(g, m.field, broken) || filled(g, [m.field])).length;
    out.status = target.filter((g) => statuses.has(g.path)).length;
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [target, broken, statuses, isEsde]);

  if (!open) return null;

  const canVisible = visiblePaths.length > 0 && visiblePaths.length < games.length;
  const canMarked = markedPaths.length > 0;
  const chosenMedia = mediaItems.filter((m) => checked.has(`media:${m.field}`));
  const toClear = [...checked].filter((id) => (counts[id] ?? 0) > 0);
  const action = mediaAction(collectionRulesFrom(settings.collection_rules));

  function toggle(id: string) {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  async function apply() {
    const labels = toClear.map((id) => {
      const item = [...DATA_ITEMS, ...FLAG_ITEMS].find((d) => d.id === id);
      if (item) return t(item.label);
      if (id === "status") return t("clear.status");
      const media = mediaItems.find((m) => `media:${m.field}` === id);
      return media ? t(media.label) : id;
    });
    const lines = [t("clear.confirm", { count: target.length, what: labels.join(", ") })];
    if (chosenMedia.length && !(keepFiles && !isEsde)) lines.push(t(action === "set_aside" ? "clear.confirmMediaSetAside" : "clear.confirmMediaTrash"));
    if (checked.has("name")) lines.push(t("clear.confirmName"));
    if (!(await askConfirm(lines.join("\n\n")))) return;

    setBusy(true);
    const summary: string[] = [];
    const failed: string[] = [];
    const { loadGames, setStatuses } = useAppStore.getState();
    const scopePaths = new Set(target.map((g) => g.path));
    let setAside = false;
    try {
      await backupLabeled(systemPath, "clear");

      // 1. Los ficheros de medios (salvo «dejar los ficheros» en clásica). En clásica, la papelera
      //    ya vacía los campos que apuntaban a ellos.
      if (chosenMedia.length && !(keepFiles && !isEsde)) {
        const paths = [
          ...new Set(
            target.flatMap((g) =>
              chosenMedia.filter((m) => hasMedia(g, m.field, broken)).map((m) => absoluteMediaPath(String(g[m.field]), systemPath))
            )
          ),
        ];
        if (paths.length) {
          setAside = action === "set_aside";
          const r = await removeMedia(systemPath, paths, action);
          if (r.line) summary.push(r.line);
          failed.push(...r.failed);
        }
        await loadGames(systemPath, { skipStatus: true });
      }

      // 2. Los campos en la gamelist: una sola escritura (con su copia de seguridad)
      const fields = [
        ...[...DATA_ITEMS, ...FLAG_ITEMS].filter((d) => checked.has(d.id)).flatMap((d) => d.fields),
        ...chosenMedia.map((m) => m.field),
      ];
      if (fields.length) {
        const { games: current, folders, provider } = useAppStore.getState();
        let changed = 0;
        const updated = current.map((g) => {
          if (!scopePaths.has(g.path) || !filled(g, fields)) return g;
          changed += 1;
          const next = { ...g } as Record<string, unknown>;
          for (const f of fields) delete next[f];
          return next as unknown as Game;
        });
        if (changed) {
          await saveGamelist(systemPath, { games: updated, folders: folders || [], provider });
          summary.push(t("clear.doneFields", { count: changed }));
        }
      }

      // 3. El estado de juego (lo guarda meROMS aparte de la gamelist)
      if (checked.has("status")) {
        const withStatus = target.filter((g) => statuses.has(g.path)).map((g) => g.path);
        if (withStatus.length) {
          await setStatuses(withStatus, { status: null, playing: false, backlog: false });
          summary.push(t("clear.doneStatus", { count: withStatus.length }));
        }
      }
    } catch (e) {
      failed.push(String(e));
    } finally {
      setBusy(false);
      bumpMediaVersion();
      await loadGames(systemPath, { mediaChanged: true }).catch(() => {});
    }
    onClose();
    const message = [t("clear.doneTitle"), "", ...(summary.length ? summary : [t("order.nothingDone")]), "", t("clear.backupNote")];
    if (failed.length) message.push("", t("order.failed", { count: failed.length }), ...failed.slice(0, 20));
    // Solo apartar medios se deshace (lo demás, desde la copia de la gamelist o la papelera):
    // sin eso no se ofrece Deshacer, que sería de otra operación anterior
    if (setAside) await reportWithUndo(message.join("\n"));
    else await showMessage(message.join("\n"), failed.length ? "error" : "info");
  }

  const row = (id: string, label: string) => {
    const count = counts[id] ?? 0;
    return (
      <label key={id} className={`flex items-center gap-2 text-sm ${count ? "text-zinc-300 cursor-pointer" : "text-zinc-600"}`}>
        <input
          type="checkbox"
          checked={checked.has(id)}
          disabled={count === 0 || busy}
          onChange={() => toggle(id)}
          className="rounded border-zinc-600 bg-zinc-700 text-red-600 disabled:opacity-30"
        />
        <span className="flex-1 truncate">{label}</span>
        <span className="text-xs text-zinc-500 tabular-nums">{t("missing.games", { count })}</span>
      </label>
    );
  };
  const section = (title: TranslationKeys, children: ReactNode) => (
    <section>
      <h4 className="text-xs font-semibold text-zinc-300 uppercase tracking-wide mb-2">{t(title)}</h4>
      <div className="grid grid-cols-2 gap-x-6 gap-y-1.5">{children}</div>
    </section>
  );

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[720px] max-w-[calc(100vw-2rem)] max-h-[88vh] flex flex-col mx-4" data-shot="clear-modal">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("clear.title")}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("clear.help")}</p>
          <div className="flex flex-wrap gap-x-5 gap-y-1 mt-3 text-sm text-zinc-300">
            <label className="flex items-center gap-2 cursor-pointer">
              <input type="radio" checked={scope === "all"} onChange={() => setScope("all")} className="text-indigo-600" />
              {t("order.scopeAll", { count: games.length })}
            </label>
            {canVisible && (
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="radio" checked={scope === "visible"} onChange={() => setScope("visible")} className="text-indigo-600" />
                {t("order.scopeVisible", { count: visiblePaths.length })}
              </label>
            )}
            {canMarked && (
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="radio" checked={scope === "marked"} onChange={() => setScope("marked")} className="text-indigo-600" />
                {t("order.scopeMarked", { count: markedPaths.length })}
              </label>
            )}
          </div>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-6">
          {section("clear.sectionData", DATA_ITEMS.map((d) => row(d.id, t(d.label))))}
          {section("clear.sectionFlags", [...FLAG_ITEMS.map((d) => row(d.id, t(d.label))), row("status", t("clear.status"))])}
          <div>
            {section("clear.sectionMedia", mediaItems.map((m) => row(`media:${m.field}`, t(m.label))))}
            <p className="text-xs text-zinc-500 mt-2">
              {t(action === "set_aside" ? "clear.mediaHelpSetAside" : "clear.mediaHelpTrash")}
            </p>
            {/* Clásica: la ruta está en la gamelist y se puede quitar dejando el fichero */}
            {!isEsde && (
              <label className="flex items-center gap-2 mt-2 text-sm text-zinc-300 cursor-pointer">
                <input type="checkbox" checked={keepFiles} onChange={() => setKeepFiles(!keepFiles)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
                {t("clear.keepFiles")}
              </label>
            )}
          </div>
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex items-center justify-between gap-3">
          <CloseButton onClick={onClose}>{t("rename.close")}</CloseButton>
          <span className="text-[11px] text-zinc-500 flex-1 text-right">{t("clear.backupHint")}</span>
          <button
            onClick={apply}
            disabled={busy || toClear.length === 0}
            data-shot="clear-apply"
            className="px-4 py-2 text-sm bg-red-600 text-white rounded hover:bg-red-500 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {busy ? <ProcessingLabel /> : t("clear.apply", { count: toClear.length })}
          </button>
        </div>
      </div>
    </div>
  );
}
