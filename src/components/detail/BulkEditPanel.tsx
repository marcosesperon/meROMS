import { useMemo, useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import type { Game } from "../../types";
import { saveGamelist } from "../../commands";
import { toESDate } from "../../utils/format";
import { askConfirm, showMessage } from "../../utils/dialogs";

// Campos que se pueden poner igual en varios juegos (nombre, descripción y ruta son de cada uno)
const BULK_FIELDS: { field: keyof Game; label: TranslationKeys; placeholder?: TranslationKeys }[] = [
  { field: "genre", label: "form.genre" },
  { field: "developer", label: "form.developer" },
  { field: "publisher", label: "form.publisher" },
  { field: "releasedate", label: "form.releaseDate", placeholder: "form.releaseDatePlaceholder" },
  { field: "players", label: "form.players", placeholder: "form.playersPlaceholder" },
  { field: "rating", label: "form.rating", placeholder: "form.ratingPlaceholder" },
  { field: "region", label: "form.region" },
  { field: "lang", label: "form.lang" },
  { field: "family", label: "form.family" },
];

const FLAGS: { field: "favorite" | "hidden" | "kidgame"; label: TranslationKeys }[] = [
  { field: "favorite", label: "form.favorite" },
  { field: "hidden", label: "form.hidden" },
  { field: "kidgame", label: "form.kidgame" },
];

// Campos de texto donde se puede buscar y reemplazar
const TEXT_FIELDS: { field: keyof Game; label: TranslationKeys }[] = [
  { field: "name", label: "form.name" },
  { field: "sortname", label: "form.sortname" },
  { field: "desc", label: "form.description" },
  { field: "genre", label: "form.genre" },
  { field: "developer", label: "form.developer" },
  { field: "publisher", label: "form.publisher" },
  { field: "family", label: "form.family" },
  { field: "region", label: "form.region" },
  { field: "lang", label: "form.lang" },
];

type FlagChoice = "keep" | "yes" | "no";

/** Guarda la gamelist con los juegos cambiados (una sola escritura, con copia de seguridad) */
async function saveGames(updated: Game[]) {
  const { selectedSystem, folders, provider, setGames } = useAppStore.getState();
  if (!selectedSystem) return;
  await saveGamelist(selectedSystem.path, { games: updated, folders: folders || [], provider });
  setGames(updated);
}

/**
 * Panel derecho en modo «Selección múltiple»: cambiar campos comunes y marcas en todos los
 * juegos seleccionados, y buscar y reemplazar texto en ellos.
 */
export function BulkEditPanel() {
  const { t } = useTranslation();
  const games = useAppStore((s) => s.games);
  const selectedPaths = useAppStore((s) => s.selectedPaths);
  const selected = useMemo(() => games.filter((g) => selectedPaths.has(g.path)), [games, selectedPaths]);

  return (
    <aside className="w-[420px] shrink-0 border-l border-zinc-800 bg-zinc-950 flex flex-col overflow-hidden">
      <div className="px-5 py-4 border-b border-zinc-800">
        <h2 className="text-lg font-bold text-zinc-100">{t("bulk.title", { count: selected.length })}</h2>
        <p className="text-xs text-zinc-500 mt-0.5">{t("bulk.help")}</p>
      </div>
      <div className="flex-1 overflow-y-auto px-5 py-4 space-y-8">
        <FieldsSection selected={selected} />
        <ReplaceSection selected={selected} />
      </div>
    </aside>
  );
}

function FieldsSection({ selected }: { selected: Game[] }) {
  const { t } = useTranslation();
  // Solo se aplica lo marcado con «cambiar»; un valor vacío borra el campo
  const [enabled, setEnabled] = useState<Set<keyof Game>>(new Set());
  const [values, setValues] = useState<Partial<Record<keyof Game, string>>>({});
  const [flags, setFlags] = useState<Record<string, FlagChoice>>({ favorite: "keep", hidden: "keep", kidgame: "keep" });
  const [saving, setSaving] = useState(false);

  const changes = enabled.size + Object.values(flags).filter((f) => f !== "keep").length;

  async function apply() {
    if (!(await askConfirm(t("bulk.applyConfirm", { count: selected.length })))) return;
    setSaving(true);
    try {
      const paths = new Set(selected.map((g) => g.path));
      const updated = useAppStore.getState().games.map((g) => {
        if (!paths.has(g.path)) return g;
        const next: Game = { ...g };
        for (const field of enabled) {
          let value = (values[field] ?? "").trim();
          if (field === "releasedate" && value) value = toESDate(value);
          (next as unknown as Record<string, string | undefined>)[field] = value || undefined;
        }
        for (const { field } of FLAGS) {
          if (flags[field] === "yes") next[field] = "true";
          if (flags[field] === "no") next[field] = undefined;
        }
        return next;
      });
      await saveGames(updated);
      setEnabled(new Set());
      setValues({});
      setFlags({ favorite: "keep", hidden: "keep", kidgame: "keep" });
    } catch (e) {
      await showMessage(t("detail.saveFailed", { error: String(e) }), "error");
    } finally {
      setSaving(false);
    }
  }

  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-3">{t("bulk.fields")}</h3>
      <div className="space-y-2">
        {BULK_FIELDS.map(({ field, label, placeholder }) => {
          const on = enabled.has(field);
          return (
            <div key={field} className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={on}
                onChange={() =>
                  setEnabled((prev) => {
                    const next = new Set(prev);
                    if (next.has(field)) next.delete(field);
                    else next.add(field);
                    return next;
                  })
                }
                title={t("bulk.change")}
                className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
              />
              <label className="w-28 shrink-0 text-xs text-zinc-400">{t(label)}</label>
              <input
                type="text"
                value={values[field] ?? ""}
                disabled={!on}
                placeholder={on ? t(placeholder ?? "bulk.emptyClears") : ""}
                onChange={(e) => setValues((prev) => ({ ...prev, [field]: e.target.value }))}
                className="flex-1 min-w-0 bg-zinc-900 border border-zinc-700 rounded px-2 py-1 text-sm text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-indigo-500 disabled:opacity-40"
              />
            </div>
          );
        })}
      </div>

      <div className="mt-4 space-y-2">
        {FLAGS.map(({ field, label }) => (
          <div key={field} className="flex items-center gap-2">
            <span className="w-[136px] shrink-0 text-xs text-zinc-400">{t(label)}</span>
            {(["keep", "yes", "no"] as FlagChoice[]).map((choice) => (
              <button
                key={choice}
                onClick={() => setFlags((prev) => ({ ...prev, [field]: choice }))}
                className={`px-2.5 py-1 text-xs rounded transition-colors ${
                  flags[field] === choice ? "bg-indigo-600 text-white" : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
                }`}
              >
                {t(`bulk.flag_${choice}` as TranslationKeys)}
              </button>
            ))}
          </div>
        ))}
      </div>

      <button
        onClick={apply}
        disabled={selected.length === 0 || changes === 0 || saving}
        className="mt-4 w-full px-4 py-2 text-sm font-semibold bg-indigo-600 text-white rounded-md hover:bg-indigo-500 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
      >
        {saving ? "..." : t("bulk.apply", { count: selected.length })}
      </button>
    </section>
  );
}

function ReplaceSection({ selected }: { selected: Game[] }) {
  const { t } = useTranslation();
  const [field, setField] = useState<keyof Game | "all">("all");
  const [find, setFind] = useState("");
  const [replace, setReplace] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [useRegex, setUseRegex] = useState(false);
  const [saving, setSaving] = useState(false);

  // La expresión de búsqueda (o el error si la expresión regular no es válida)
  const pattern = useMemo(() => {
    if (!find) return null;
    try {
      const source = useRegex ? find : find.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      return new RegExp(source, caseSensitive ? "g" : "gi");
    } catch (e) {
      return e as Error;
    }
  }, [find, useRegex, caseSensitive]);

  const fields = field === "all" ? TEXT_FIELDS.map((f) => f.field) : [field];

  // Vista previa: cuántos campos y juegos cambiarían
  const preview = useMemo(() => {
    if (!(pattern instanceof RegExp)) return { fields: 0, games: 0 };
    let changedFields = 0;
    let changedGames = 0;
    for (const g of selected) {
      let touched = false;
      for (const f of fields) {
        const value = g[f] as string | undefined;
        if (value && value.replace(pattern, replace) !== value) {
          changedFields++;
          touched = true;
        }
      }
      if (touched) changedGames++;
    }
    return { fields: changedFields, games: changedGames };
  }, [selected, pattern, replace, field]);

  async function apply() {
    if (!(pattern instanceof RegExp)) return;
    setSaving(true);
    try {
      const paths = new Set(selected.map((g) => g.path));
      const updated = useAppStore.getState().games.map((g) => {
        if (!paths.has(g.path)) return g;
        const next: Game = { ...g };
        for (const f of fields) {
          const value = next[f] as string | undefined;
          if (value) {
            const replaced = value.replace(pattern, replace).trim();
            (next as unknown as Record<string, string | undefined>)[f] = replaced || undefined;
          }
        }
        return next;
      });
      await saveGames(updated);
    } catch (e) {
      await showMessage(t("detail.saveFailed", { error: String(e) }), "error");
    } finally {
      setSaving(false);
    }
  }

  const inputClass =
    "w-full bg-zinc-900 border border-zinc-700 rounded px-2 py-1.5 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500";

  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold mb-3">{t("bulk.replaceTitle")}</h3>
      <div className="space-y-2">
        <select value={field} onChange={(e) => setField(e.target.value as keyof Game | "all")} className={inputClass}>
          <option value="all">{t("bulk.allTextFields")}</option>
          {TEXT_FIELDS.map((f) => (
            <option key={f.field} value={f.field}>
              {t(f.label)}
            </option>
          ))}
        </select>
        <input type="text" value={find} onChange={(e) => setFind(e.target.value)} placeholder={t("bulk.find")} className={inputClass} />
        <input type="text" value={replace} onChange={(e) => setReplace(e.target.value)} placeholder={t("bulk.replaceWith")} className={inputClass} />
        <div className="flex items-center gap-4 text-xs text-zinc-400">
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input type="checkbox" checked={caseSensitive} onChange={(e) => setCaseSensitive(e.target.checked)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
            {t("bulk.caseSensitive")}
          </label>
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input type="checkbox" checked={useRegex} onChange={(e) => setUseRegex(e.target.checked)} className="rounded border-zinc-600 bg-zinc-700 text-indigo-600" />
            {t("bulk.regex")}
          </label>
        </div>
        {pattern instanceof Error ? (
          <p className="text-xs text-red-400">{t("bulk.badRegex", { error: pattern.message })}</p>
        ) : find ? (
          <p className="text-xs text-zinc-500">{t("bulk.preview", { fields: preview.fields, games: preview.games })}</p>
        ) : null}
      </div>
      <button
        onClick={apply}
        disabled={preview.fields === 0 || saving}
        className="mt-3 w-full px-4 py-2 text-sm bg-zinc-800 text-zinc-200 rounded-md hover:bg-zinc-700 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
      >
        {saving ? "..." : t("bulk.replaceApply", { count: preview.fields })}
      </button>
    </section>
  );
}
