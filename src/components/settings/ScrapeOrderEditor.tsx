import { useTranslation } from "../../i18n";

// Códigos de región de ScreenScraper con su nombre en cada idioma de la app
const REGIONS: { code: string; es: string; en: string }[] = [
  { code: "wor", es: "Mundo", en: "World" },
  { code: "eu", es: "Europa", en: "Europe" },
  { code: "us", es: "EE. UU.", en: "USA" },
  { code: "jp", es: "Japón", en: "Japan" },
  { code: "sp", es: "España", en: "Spain" },
  { code: "fr", es: "Francia", en: "France" },
  { code: "de", es: "Alemania", en: "Germany" },
  { code: "it", es: "Italia", en: "Italy" },
  { code: "uk", es: "Reino Unido", en: "United Kingdom" },
  { code: "br", es: "Brasil", en: "Brazil" },
  { code: "kr", es: "Corea", en: "Korea" },
  { code: "cn", es: "China", en: "China" },
  { code: "au", es: "Australia", en: "Australia" },
  { code: "ss", es: "ScreenScraper (genérica)", en: "ScreenScraper (generic)" },
];

// Idiomas que usa ScreenScraper (el nombre lo pone el propio navegador en el idioma de la app)
const LANGUAGES = ["es", "en", "fr", "de", "it", "pt", "ja", "nl", "sv", "da", "no", "fi", "pl", "cs", "hu", "ru", "zh", "ko"];

interface OrderListProps {
  items: string[];
  allCodes: string[];
  label: (code: string) => string;
  onChange: (items: string[]) => void;
}

/** Lista ordenable: subir/bajar/quitar cada elemento y un desplegable para añadir */
function OrderList({ items, allCodes, label, onChange }: OrderListProps) {
  const { t } = useTranslation();
  const move = (i: number, d: number) => {
    const next = [...items];
    [next[i], next[i + d]] = [next[i + d], next[i]];
    onChange(next);
  };
  const missing = allCodes.filter((c) => !items.includes(c));
  const btn = "w-6 h-6 rounded text-xs text-zinc-400 hover:text-zinc-100 hover:bg-zinc-700 disabled:opacity-20";

  return (
    <div className="space-y-1">
      {items.map((code, i) => (
        <div key={code} className="flex items-center gap-1 px-2 py-1 rounded bg-zinc-800">
          <span className="w-5 text-[11px] text-zinc-500">{i + 1}.</span>
          <span className="flex-1 text-xs text-zinc-200">{label(code)}</span>
          <button className={btn} disabled={i === 0} onClick={() => move(i, -1)} title={t("scrapeOrder.up")}>↑</button>
          <button className={btn} disabled={i === items.length - 1} onClick={() => move(i, 1)} title={t("scrapeOrder.down")}>↓</button>
          <button
            className={btn}
            disabled={items.length === 1}
            onClick={() => onChange(items.filter((c) => c !== code))}
            title={t("scrapeOrder.remove")}
          >
            ✕
          </button>
        </div>
      ))}
      {missing.length > 0 && (
        <select
          value=""
          onChange={(e) => e.target.value && onChange([...items, e.target.value])}
          className="w-full bg-zinc-900 border border-zinc-700 rounded px-2 py-1 text-xs text-zinc-400 focus:outline-none focus:border-indigo-500"
        >
          <option value="">{t("scrapeOrder.add")}</option>
          {missing.map((c) => (
            <option key={c} value={c}>
              {label(c)}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}

interface ScrapeOrderEditorProps {
  regions: string[];
  languages: string[];
  onChange: (regions: string[], languages: string[]) => void;
}

/**
 * Orden de regiones (nombre, fecha y medios) y de idiomas (descripción y género) que usa el
 * scraper. Si nada coincide, el scraper coge lo primero que haya.
 */
export function ScrapeOrderEditor({ regions, languages, onChange }: ScrapeOrderEditorProps) {
  const { t, locale } = useTranslation();
  const regionLabel = (code: string) => {
    const r = REGIONS.find((x) => x.code === code);
    return r ? (locale === "es" ? r.es : r.en) : code;
  };
  const languageNames = (() => {
    try {
      return new Intl.DisplayNames([locale], { type: "language" });
    } catch {
      return null;
    }
  })();
  const languageLabel = (code: string) => {
    const name = languageNames?.of(code);
    return name ? name.charAt(0).toUpperCase() + name.slice(1) : code;
  };

  return (
    <div className="grid grid-cols-2 gap-6">
      <div>
        <p className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-1">{t("scrapeOrder.regions")}</p>
        <p className="text-[11px] text-zinc-500 mb-2">{t("scrapeOrder.regionsHelp")}</p>
        <OrderList items={regions} allCodes={REGIONS.map((r) => r.code)} label={regionLabel} onChange={(r) => onChange(r, languages)} />
      </div>
      <div>
        <p className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-1">{t("scrapeOrder.languages")}</p>
        <p className="text-[11px] text-zinc-500 mb-2">{t("scrapeOrder.languagesHelp")}</p>
        <OrderList items={languages} allCodes={LANGUAGES} label={languageLabel} onChange={(l) => onChange(regions, l)} />
      </div>
    </div>
  );
}
