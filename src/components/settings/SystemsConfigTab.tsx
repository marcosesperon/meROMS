import { useMemo, useState } from "react";
import type { SystemInfo } from "../../types";
import { useTranslation } from "../../i18n";
import { getDefaultSystemRatio } from "../../utils/cartridgeSizes";

/** Standard aspect ratio options for the selector */
const ASPECT_RATIO_OPTIONS = [
  { value: "", label: "" },       // default (placeholder)
  { value: "3/4", label: "3:4" },       // Portrait box (NES, SNES, etc.)
  { value: "7/12", label: "7:12" },     // UMD case (PSP): taller than a DVD case
  { value: "27/38", label: "27:38" },   // DVD case (PS2, Wii)
  { value: "27/33", label: "27:33" },   // Blu-ray case (PS3, PS4)
  { value: "27/32", label: "27:32" },   // Handheld box (GBA, DS)
  { value: "1/1", label: "1:1" },       // Square (PC Engine, jewel case)
  { value: "4/3", label: "4:3" },       // Landscape screenshot
  { value: "16/9", label: "16:9" },     // Widescreen
  { value: "16/5", label: "16:5" },     // Ultra-wide (marquee/wheel)
  { value: "27/34", label: "27:34" },   // Switch case
];

/** For search and sorting: lowercase, no accents ("Nintendo" = "nintendo") */
const fold = (text: string) => text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();

interface SystemConfig {
  aspect_ratio?: string;
}

interface SystemsConfigTabProps {
  systems: SystemInfo[];
  systemConfigs: Record<string, SystemConfig>;
  onChange: (configs: Record<string, SystemConfig>) => void;
}

export function SystemsConfigTab({ systems, systemConfigs, onChange }: SystemsConfigTabProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");

  // Sorted by the name shown (full name, else the folder) and filtered by the search box,
  // which also looks at the folder name ("n64")
  const shown = useMemo(() => {
    const q = fold(query.trim());
    return systems
      .filter((sys) => !q || fold(sys.full_name || "").includes(q) || fold(sys.name).includes(q))
      .sort((a, b) => (a.full_name || a.name).localeCompare(b.full_name || b.name, undefined, { sensitivity: "base" }));
  }, [systems, query]);

  function updateSystem(systemKey: string, field: keyof SystemConfig, value: string) {
    const current = { ...systemConfigs };
    const cfg = { ...(current[systemKey] || {}) };

    if (value) {
      (cfg as Record<string, string>)[field] = value;
    } else {
      delete (cfg as Record<string, string | undefined>)[field];
    }

    // Clean up empty configs
    if (Object.keys(cfg).length === 0) {
      delete current[systemKey];
    } else {
      current[systemKey] = cfg;
    }

    onChange(current);
  }

  if (systems.length === 0) {
    return (
      <p className="text-zinc-500 text-sm">{t("sidebar.noSystems")}</p>
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-xs text-zinc-500">
        {t("settings.systemsHelp")}
      </p>
      <input
        type="search"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder={t("settings.systemsSearch")}
        className="bg-zinc-800 border border-zinc-600 rounded px-3 py-1.5 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500 w-64"
      />
      <div className="border border-zinc-700 rounded overflow-hidden">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-zinc-800">
              <th className="text-left px-3 py-2 text-zinc-400 font-medium">
                {t("settings.arSystem")}
              </th>
              <th className="text-left px-3 py-2 text-zinc-400 font-medium">
                {t("settings.arDefault")}
              </th>
              <th className="text-left px-3 py-2 text-zinc-400 font-medium">
                {t("settings.arCustom")}
              </th>
            </tr>
          </thead>
          <tbody>
            {shown.length === 0 && (
              <tr className="border-t border-zinc-700/50">
                <td colSpan={3} className="px-3 py-6 text-center text-zinc-500">{t("settings.systemsNoMatch")}</td>
              </tr>
            )}
            {shown.map((sys) => {
              const key = sys.name.toLowerCase().replace(/[^a-z0-9]/g, "");
              const defaultRatio = getDefaultSystemRatio(sys.name);
              const defaultStr = `${defaultRatio[0]}:${defaultRatio[1]}`;
              const cfg = systemConfigs[key] || {};
              const customAR = cfg.aspect_ratio || "";

              return (
                <tr key={sys.name} className="border-t border-zinc-700/50">
                  <td className="px-3 py-2 text-zinc-300">{sys.full_name}</td>
                  <td className="px-3 py-2 text-zinc-500">{defaultStr}</td>
                  <td className="px-3 py-2">
                    <select
                      value={customAR}
                      onChange={(e) => updateSystem(key, "aspect_ratio", e.target.value)}
                      className="bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-xs text-zinc-200 focus:outline-none focus:border-indigo-500"
                    >
                      {ASPECT_RATIO_OPTIONS.map((opt) => (
                        <option key={opt.value} value={opt.value}>
                          {opt.value === "" ? `— ${defaultStr} —` : opt.label}
                        </option>
                      ))}
                    </select>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
