import { useTranslation } from "../../i18n";

// Códigos de región de ScreenScraper con su nombre en español e inglés; en los demás idiomas el
// nombre lo pone el navegador a partir de `iso` (país o región de la ONU: 001 mundo, 150 Europa)
const REGIONS: { code: string; iso?: string; es: string; en: string }[] = [
  { code: "wor", iso: "001", es: "Mundo", en: "World" },
  { code: "eu", iso: "150", es: "Europa", en: "Europe" },
  { code: "us", iso: "US", es: "EE. UU.", en: "USA" },
  { code: "jp", iso: "JP", es: "Japón", en: "Japan" },
  { code: "sp", iso: "ES", es: "España", en: "Spain" },
  { code: "fr", iso: "FR", es: "Francia", en: "France" },
  { code: "de", iso: "DE", es: "Alemania", en: "Germany" },
  { code: "it", iso: "IT", es: "Italia", en: "Italy" },
  { code: "uk", iso: "GB", es: "Reino Unido", en: "United Kingdom" },
  { code: "br", iso: "BR", es: "Brasil", en: "Brazil" },
  { code: "kr", iso: "KR", es: "Corea", en: "Korea" },
  { code: "cn", iso: "CN", es: "China", en: "China" },
  { code: "au", iso: "AU", es: "Australia", en: "Australia" },
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
  const regionNames = (() => {
    try {
      return new Intl.DisplayNames([locale], { type: "region" });
    } catch {
      return null;
    }
  })();
  const regionLabel = (code: string) => {
    const r = REGIONS.find((x) => x.code === code);
    if (!r) return code;
    if (locale === "es") return r.es;
    if (locale === "en" || !r.iso) return r.en;
    return regionNames?.of(r.iso) ?? r.en;
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
