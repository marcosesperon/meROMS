import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { useAppStore, type ToolRequest } from "../../store/useAppStore";
import { collectionRulesFrom } from "../../utils/collectionRules";
import { hasArrmSortname, hasInvalidData, lacksData, missingMedia, type IssueKind, type StatusItems } from "../../utils/collectionStatus";
import { formatAgo } from "../../utils/format";
import { getActiveProfile } from "../../utils/profiles";
import type { OrderStep } from "../../utils/orderSteps";
import { CheckIcon } from "../common/Icons";

/** Una línea del estado: sus números, cómo se dicen, cómo verlo y qué pasos lo arreglan */
interface StatusLine {
  kind: IssueKind;
  /** null = aún no se sabe (comprobándose). Varios números (ficheros y títulos) se suman para saber si sale */
  values: number[] | null;
  /** El texto con los números (ya animados) */
  text: (values: number[]) => string;
  /** Al pulsar el número, la lista se filtra a esos juegos */
  filter: boolean;
  view?: ToolRequest;
  /** Pasos que lo arreglan (los medios que faltan: antes colocar lo que ya hay, después scrapear).
   *  Sin pasos, la línea solo informa (no hay «Arreglar») */
  fix?: OrderStep | OrderStep[];
}

/** Duración de la animación de los números al cambiar */
const COUNT_MS = 700;

/**
 * Números que cuentan hasta su valor nuevo (de donde estaban; la primera vez, de 0), con una
 * curva que frena al final. Una sola animación para todos los números de una línea.
 */
function useCountUp(targets: number[]): number[] {
  const [shown, setShown] = useState<number[]>(() => targets.map(() => 0));
  const from = useRef<number[]>(shown);
  const key = targets.join(",");
  useEffect(() => {
    const start = performance.now();
    const origin = from.current;
    let frame = 0;
    const tick = (now: number) => {
      const p = Math.min(1, (now - start) / COUNT_MS);
      const ease = 1 - Math.pow(1 - p, 3);
      const next = targets.map((to, i) => Math.round((origin[i] ?? 0) + (to - (origin[i] ?? 0)) * ease));
      from.current = next;
      setShown(next);
      if (p < 1) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return shown;
}

/**
 * Estado del sistema frente a «Mi colección» (en el resumen del sistema del panel derecho): una
 * línea por cada cosa que no se cumple, con su número. Las comprobaciones van llegando una a una
 * y cada número cuenta hasta su valor. El número filtra la lista; «Ver» abre la herramienta (al
 * momento: el backend guarda lo comprobado) y «Arreglar» abre «Ponerlo todo en orden» con solo
 * ese paso marcado. Lo que está en orden no sale: si no queda nada, un ✓.
 */
export function CollectionStatus() {
  const { t, locale } = useTranslation();
  const status = useAppStore((s) => s.collectionStatus);
  const games = useAppStore((s) => s.games);
  const settings = useAppStore((s) => s.settings);
  const broken = useAppStore((s) => s.brokenMedia);
  const newRoms = useAppStore((s) => s.newRomsCount);
  const missingRoms = useAppStore((s) => s.missingRomPaths.size);
  const scrapeMisses = useAppStore((s) => s.scrapeMisses.size);
  const issue = useAppStore((s) => s.filters.issue);
  const setFilters = useAppStore((s) => s.setFilters);
  const openTool = useAppStore((s) => s.openTool);
  const check = useAppStore((s) => s.checkCollectionStatus);

  const rules = collectionRulesFrom(settings.collection_rules);
  const isEsde = getActiveProfile(settings)?.kind === "esde";
  const parts = status?.parts ?? {};
  const names = status?.names ?? null;
  const noData = games.filter((g) => lacksData(g, rules)).length;
  const noMedia = games.filter((g) => missingMedia(g, rules, broken, isEsde).length > 0).length;
  const invalid = games.filter(hasInvalidData).length;
  const arrmSort = games.filter(hasArrmSortname).length;

  // Línea de medios: se ve en su pestaña de la ventana Medios y la arregla «Limpiar medios»
  const mediaLine = (kind: IssueKind, part: StatusItems | undefined, count: (p: StatusItems) => number, key: TranslationKeys, tab: string): StatusLine => ({
    kind,
    values: part ? [count(part)] : null,
    text: ([n]) => t(key, { count: n }),
    filter: true,
    view: { id: "media", tab },
    fix: "media",
  });

  const lines: { group: TranslationKeys; items: StatusLine[] }[] = [
    {
      group: "colstatus.groupRoms",
      items: [
        { kind: "newRoms", values: [newRoms], text: ([n]) => t("colstatus.newRoms", { count: n }), filter: false, view: { id: "newRoms" }, fix: "roms" },
        { kind: "missingRoms", values: [missingRoms], text: ([n]) => t("colstatus.missingRoms", { count: n }), filter: true, view: { id: "missingRoms" }, fix: "roms" },
      ],
    },
    {
      group: "colstatus.groupVersions",
      items: [
        {
          kind: "versions",
          values: parts.versions ? [parts.versions.length] : null,
          text: ([n]) => t("colstatus.versions", { count: n }),
          filter: true,
          view: { id: "versions" },
          fix: "versions",
        },
      ],
    },
    {
      group: "colstatus.groupNames",
      items: [
        {
          kind: "names",
          values: names ? [names.files.length, names.titles.length] : null,
          text: ([files, titles]) => t("colstatus.names", { files, titles }),
          filter: true,
          view: { id: "rename" },
          fix: "names",
        },
        // Nombre para ordenar de ARRM, desfasado tras renombrar (lo quita el mismo paso)
        { kind: "arrmSort", values: [arrmSort], text: ([n]) => t("colstatus.arrmSort", { count: n }), filter: true, fix: "names" },
      ],
    },
    {
      group: "colstatus.groupData",
      items: [
        // Primero lo falso que dejó el scraper (lo arregla el mismo paso, antes de scrapear)
        { kind: "invalid", values: [invalid], text: ([n]) => t("colstatus.invalid", { count: n }), filter: true, view: { id: "missing", tab: "invalid" }, fix: "scrape" },
        { kind: "data", values: [noData], text: ([n]) => t("colstatus.data", { count: n }), filter: true, view: { id: "missing", tab: "data" }, fix: "scrape" },
        // Ya intentados sin éxito: el paso de scrapeo se los salta (solo se dice cuántos son)
        { kind: "scrapeMiss", values: [scrapeMisses], text: ([n]) => t("colstatus.scrapeMiss", { count: n }), filter: true },
      ],
    },
    {
      group: "colstatus.groupMedia",
      // En el orden en que conviene arreglarlo: primero colocar lo que ya hay (nombres antiguos,
      // cambiados), después quitar lo que sobra y al final buscar lo que falta
      items: [
        mediaLine("legacy", parts.legacy, (p) => p.files, "colstatus.legacy", "legacy"),
        mediaLine("swapped", parts.swapped, (p) => p.games.length, "colstatus.swapped", "swapped"),
        mediaLine("damaged", parts.damaged, (p) => p.files, "colstatus.damaged", "damaged"),
        mediaLine("placeholders", parts.placeholders, (p) => p.files, "colstatus.placeholders", "damaged"),
        // Primero lo que se puede aprovechar; los sobrantes de verdad son los demás
        mediaLine("reuse", parts.reuse, (p) => p.files, "colstatus.reuse", "unused"),
        {
          ...mediaLine("unused", parts.unused, (p) => Math.max(0, p.files - (parts.reuse?.files ?? 0)), "colstatus.unused", "unused"),
          filter: false,
        },
        {
          kind: "media",
          values: [noMedia],
          text: ([n]) => t("colstatus.media", { count: n }),
          filter: true,
          view: { id: "missing", tab: "media" },
          fix: ["media", "scrape"],
        },
      ],
    },
  ];

  const total = (l: StatusLine) => (l.values ?? []).reduce((a, b) => a + b, 0);
  // Solo lo que falla (o lo que aún no se sabe); los grupos sin nada desaparecen
  const visible = lines.map((g) => ({ ...g, items: g.items.filter((l) => l.values === null || total(l) > 0) })).filter((g) => g.items.length > 0);
  const pending = visible.some((g) => g.items.some((l) => l.values === null));
  // Algo que arreglar (las líneas que solo informan no cuentan)
  const anything = visible.some((g) => g.items.some((l) => l.fix && total(l) > 0));
  const checking = status?.checking ?? true;
  const errors = status?.errors ?? [];

  return (
    <section className="mt-6 rounded-lg border border-zinc-800 bg-zinc-900/60 overflow-hidden" data-shot="collection-status">
      <div className="px-4 py-2.5 flex items-center justify-between gap-2 border-b border-zinc-800">
        <h3 className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold">{t("colstatus.title")}</h3>
        <button
          onClick={() => openTool({ id: "order" })}
          disabled={!anything}
          data-shot="order-open"
          className="px-2.5 py-1 text-xs bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {t("order.button")}
        </button>
      </div>

      <div className="px-4 py-3 space-y-3">
        {visible.map((g) => (
          <div key={g.group} className="grid grid-cols-[72px_1fr] gap-x-2">
            <p className="text-[11px] text-zinc-500 pt-0.5">{t(g.group)}</p>
            <div className="space-y-1.5">
              {g.items.map((line) => (
                <Line
                  key={line.kind}
                  line={line}
                  active={issue === line.kind}
                  onFilter={() => setFilters({ issue: issue === line.kind ? undefined : line.kind })}
                  onView={line.view ? () => openTool(line.view!) : line.filter ? () => setFilters({ issue: line.kind }) : undefined}
                  onFix={line.fix ? () => openTool({ id: "order", steps: Array.isArray(line.fix) ? line.fix : [line.fix!] }) : undefined}
                />
              ))}
            </div>
          </div>
        ))}
        {!anything && !pending && (
          <p className="flex items-center gap-1.5 text-sm text-emerald-400">
            <CheckIcon size={14} />
            {t("colstatus.allGood")}
          </p>
        )}
        {errors.length > 0 && (
          <p className="text-[11px] text-amber-400" title={errors.join("\n")}>
            {t("colstatus.someFailed", { count: errors.length })}
          </p>
        )}
      </div>

      <div className="px-4 py-2 border-t border-zinc-800 flex items-center justify-between text-[11px] text-zinc-500">
        <span>
          {checking ? t("colstatus.checking") : status?.checkedAt ? t("colstatus.checkedAgo", { ago: formatAgo(status.checkedAt, locale) }) : ""}
        </span>
        <button onClick={() => check(true)} disabled={checking} className="text-indigo-400 hover:text-indigo-300 disabled:opacity-40">
          {t("colstatus.recheck")}
        </button>
      </div>
    </section>
  );
}

function Line({ line, active, onFilter, onView, onFix }: { line: StatusLine; active: boolean; onFilter: () => void; onView?: () => void; onFix?: () => void }) {
  const { t } = useTranslation();
  const known = line.values !== null;
  // Los números cuentan hasta su valor (los de una línea que aún no se sabe, desde 0)
  const shown = useCountUp(line.values ?? [0, 0]);
  const text = known ? line.text(shown) : t("colstatus.pending");
  const link = (label: string, onClick: () => void) => (
    <button onClick={onClick} className="text-indigo-400 hover:text-indigo-300">
      {label}
    </button>
  );
  return (
    <div className="flex items-start gap-2 text-sm">
      {known && line.filter ? (
        <button
          onClick={onFilter}
          title={t("colstatus.filterHint")}
          className={`flex-1 text-left leading-snug tabular-nums ${active ? "text-indigo-200 underline" : "text-zinc-300 hover:text-zinc-100"}`}
        >
          {text}
        </button>
      ) : (
        <span className={`flex-1 leading-snug tabular-nums ${known ? "text-zinc-300" : "text-zinc-600"}`}>{text}</span>
      )}
      {known && (
        <span className="shrink-0 flex gap-2 text-xs pt-0.5">
          {onView && link(t("colstatus.view"), onView)}
          {onFix && link(t("colstatus.fix"), onFix)}
        </span>
      )}
    </div>
  );
}
