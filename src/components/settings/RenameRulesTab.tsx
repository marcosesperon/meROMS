import { useEffect, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "../../i18n";
import { exportRenameRules, getDefaultRenameRules, importRenameRules, testRenameRules } from "../../commands";
import type { RenameRule, RuleTestResult } from "../../types";
import { useDebounce } from "../../hooks/useDebounce";
import { TrashIcon } from "../common/Icons";

interface RenameRulesTabProps {
  /** undefined = el usuario no ha tocado nada y se usan las de serie */
  rules?: RenameRule[];
  onChange: (rules: RenameRule[] | undefined) => void;
}

const SAMPLE_NAME = "Legend of Example, The (USA) (En,Fr,De) (Rev 1) [!]";

/**
 * Reglas para «Limpiar con reglas» en Renombrar (estilo ES-DE-Roms-Renamer).
 * Se aplican en orden, de arriba abajo. Abajo hay un campo para probarlas en vivo.
 */
export function RenameRulesTab({ rules, onChange }: RenameRulesTabProps) {
  const { t } = useTranslation();
  const [defaults, setDefaults] = useState<RenameRule[]>([]);
  const [sample, setSample] = useState(SAMPLE_NAME);
  const [test, setTest] = useState<RuleTestResult | null>(null);

  useEffect(() => {
    getDefaultRenameRules().then(setDefaults).catch(console.error);
  }, []);

  const current = rules ?? defaults;
  const debouncedSample = useDebounce(sample, 250);

  // Prueba en vivo: se recalcula al cambiar el ejemplo o cualquier regla
  useEffect(() => {
    if (current.length === 0 && !rules) return;
    testRenameRules(debouncedSample, current).then(setTest).catch(console.error);
  }, [debouncedSample, current]);

  const errorFor = (index: number) => test?.errors.find((e) => e.index === index)?.message;

  function update(index: number, changes: Partial<RenameRule>) {
    onChange(current.map((r, i) => (i === index ? { ...r, ...changes } : r)));
  }

  function move(index: number, delta: number) {
    const target = index + delta;
    if (target < 0 || target >= current.length) return;
    const next = [...current];
    [next[index], next[target]] = [next[target], next[index]];
    onChange(next);
  }

  function remove(index: number) {
    onChange(current.filter((_, i) => i !== index));
  }

  function add() {
    onChange([...current, { pattern: "", replacement: "", description: "", enabled: true }]);
  }

  async function handleImport() {
    const file = await openDialog({ filters: [{ name: "JSON", extensions: ["json"] }], multiple: false });
    if (!file) return;
    try {
      onChange(await importRenameRules(String(file)));
    } catch (e) {
      alert(t("rules.importError", { error: String(e) }));
    }
  }

  async function handleExport() {
    const dest = await saveDialog({ defaultPath: "rules.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!dest) return;
    try {
      await exportRenameRules(current, dest);
      alert(t("rules.exportSuccess"));
    } catch (e) {
      alert(String(e));
    }
  }

  const inputClass =
    "w-full bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-xs font-mono text-zinc-200 focus:outline-none focus:border-indigo-500";

  return (
    <div className="space-y-5">
      <div>
        <h3 className="text-sm font-medium text-zinc-300 mb-1">{t("rules.title")}</h3>
        <p className="text-xs text-zinc-500">{t("rules.description")}</p>
      </div>

      {/* Prueba en vivo */}
      <section className="p-3 rounded border border-zinc-700 bg-zinc-800/50 space-y-2">
        <label className="text-xs text-zinc-400 block">{t("rules.testLabel")}</label>
        <input
          type="text"
          value={sample}
          onChange={(e) => setSample(e.target.value)}
          className="w-full bg-zinc-900 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
        />
        <p className="text-sm">
          <span className="text-zinc-500">{t("rules.testResult")} </span>
          <span className="text-indigo-300 font-medium">{test?.result || "—"}</span>
        </p>
      </section>

      <div className="flex flex-wrap gap-2">
        <button onClick={add} className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
          {t("rules.add")}
        </button>
        <button onClick={handleImport} className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
          {t("rules.import")}
        </button>
        <button onClick={handleExport} className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
          {t("rules.export")}
        </button>
        <div className="flex-1" />
        <button
          onClick={() => confirm(t("rules.resetConfirm")) && onChange(undefined)}
          disabled={!rules}
          className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 disabled:opacity-40"
        >
          {t("rules.reset")}
        </button>
      </div>

      <div className="border border-zinc-700 rounded divide-y divide-zinc-700/50">
        {current.map((rule, index) => {
          const error = errorFor(index);
          return (
            <div key={index} className={`p-2 grid grid-cols-[auto_1fr_1fr_auto] gap-2 items-start ${rule.enabled ? "" : "opacity-50"}`}>
              <input
                type="checkbox"
                checked={rule.enabled}
                onChange={(e) => update(index, { enabled: e.target.checked })}
                title={t("rules.enabled")}
                className="mt-1.5 rounded border-zinc-600 bg-zinc-700 text-indigo-600"
              />
              <div className="space-y-1">
                <input
                  type="text"
                  value={rule.pattern}
                  onChange={(e) => update(index, { pattern: e.target.value })}
                  placeholder={t("rules.pattern")}
                  className={`${inputClass} ${error ? "border-red-500" : ""}`}
                />
                {error && <p className="text-[10px] text-red-400 break-all">{error}</p>}
                <input
                  type="text"
                  value={rule.description ?? ""}
                  onChange={(e) => update(index, { description: e.target.value })}
                  placeholder={t("rules.descriptionField")}
                  className="w-full bg-transparent border-0 px-2 py-0.5 text-[11px] text-zinc-500 focus:outline-none focus:text-zinc-300"
                />
              </div>
              <input
                type="text"
                value={rule.replacement}
                onChange={(e) => update(index, { replacement: e.target.value })}
                placeholder={t("rules.replacement")}
                className={inputClass}
              />
              <div className="flex gap-1">
                <button onClick={() => move(index, -1)} disabled={index === 0} className="px-1.5 text-zinc-400 hover:text-zinc-200 disabled:opacity-30" title={t("rules.up")}>↑</button>
                <button onClick={() => move(index, 1)} disabled={index === current.length - 1} className="px-1.5 text-zinc-400 hover:text-zinc-200 disabled:opacity-30" title={t("rules.down")}>↓</button>
                <button onClick={() => remove(index)} className="px-1.5 text-red-400 hover:text-red-300" title={t("rules.remove")}>
                  <TrashIcon size={12} />
                </button>
              </div>
            </div>
          );
        })}
      </div>
      <p className="text-xs text-zinc-500">{t("rules.syntaxHelp")}</p>
    </div>
  );
}
