import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { AVAILABLE_MEDIA_TYPES, type FieldPolicy, type ScrapePolicy } from "../../types";

// Datos que rellena el scraper (mismo orden y nombres que SCRAPE_DATA_FIELDS en Rust)
export const DATA_FIELDS: { field: string; label: TranslationKeys }[] = [
  { field: "name", label: "form.name" },
  { field: "desc", label: "form.description" },
  { field: "genre", label: "form.genre" },
  { field: "developer", label: "form.developer" },
  { field: "publisher", label: "form.publisher" },
  { field: "players", label: "form.players" },
  { field: "rating", label: "form.rating" },
  { field: "releasedate", label: "form.releaseDate" },
];

const CHOICES: FieldPolicy[] = ["overwrite", "if_empty", "keep"];

interface ScrapePolicyEditorProps {
  policy: ScrapePolicy;
  onChange: (policy: ScrapePolicy) => void;
}

/**
 * Qué puede actualizar el scraper: por cada dato y por cada tipo de medio, «Sobrescribir»,
 * «Solo si está vacío» (medios: «Solo si falta») o «No tocar». Se usa en Ajustes y al lanzar
 * «Scrapear Todo».
 */
export function ScrapePolicyEditor({ policy, onChange }: ScrapePolicyEditorProps) {
  const { t } = useTranslation();
  const dataOf = (f: string): FieldPolicy => policy.data[f] ?? "if_empty";
  const mediaOf = (m: string): FieldPolicy => policy.media[m] ?? "keep";

  // Botones rápidos: todo a la vez (los medios que estaban en «No tocar» se quedan así)
  function setAll(choice: FieldPolicy) {
    onChange({
      data: Object.fromEntries(DATA_FIELDS.map(({ field }) => [field, choice])),
      media: Object.fromEntries(
        AVAILABLE_MEDIA_TYPES.map(({ id }) => [id, mediaOf(id) === "keep" ? "keep" : choice])
      ),
    });
  }

  const row = (label: string, value: FieldPolicy, isMedia: boolean, set: (v: FieldPolicy) => void) => (
    <div key={label} className="flex items-center gap-2">
      <span className="flex-1 min-w-0 text-xs text-zinc-300 truncate">{label}</span>
      <div className="flex rounded-md overflow-hidden border border-zinc-700 shrink-0">
        {CHOICES.map((choice) => (
          <button
            key={choice}
            onClick={() => set(choice)}
            className={`px-2 py-1 text-[11px] transition-colors ${
              value === choice ? "bg-indigo-600 text-white" : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
            }`}
          >
            {t(choice === "if_empty" && isMedia ? "scrapePolicy.ifMissing" : (`scrapePolicy.${choice}` as TranslationKeys))}
          </button>
        ))}
      </div>
    </div>
  );

  return (
    <div className="space-y-4" data-shot="scrape-policy">
      <div className="flex gap-2">
        <button onClick={() => setAll("if_empty")} className="px-2.5 py-1 text-xs bg-zinc-800 text-zinc-300 rounded hover:bg-zinc-700">
          {t("scrapePolicy.allIfEmpty")}
        </button>
        <button onClick={() => setAll("overwrite")} className="px-2.5 py-1 text-xs bg-zinc-800 text-zinc-300 rounded hover:bg-zinc-700">
          {t("scrapePolicy.allOverwrite")}
        </button>
      </div>
      <div className="grid grid-cols-2 gap-x-6">
        <div className="space-y-1.5">
          <p className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-1">{t("scrapePolicy.data")}</p>
          {DATA_FIELDS.map(({ field, label }) =>
            row(t(label), dataOf(field), false, (v) => onChange({ ...policy, data: { ...policy.data, [field]: v } }))
          )}
        </div>
        <div className="space-y-1.5">
          <p className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-1">{t("scrapePolicy.media")}</p>
          {AVAILABLE_MEDIA_TYPES.map(({ id, label }) =>
            row(label, mediaOf(id), true, (v) => onChange({ ...policy, media: { ...policy.media, [id]: v } }))
          )}
        </div>
      </div>
    </div>
  );
}
