import { useTranslation } from "../../i18n";
import {
  AVAILABLE_MEDIA_TYPES,
  DEFAULT_MEDIA_PATHS,
  LEGACY_MEDIA_PATHS,
  type MediaPathConfig,
} from "../../types";

interface MediaConfigTabProps {
  mediaPaths: MediaPathConfig[];
  onChange: (paths: MediaPathConfig[]) => void;
}

type Preset = "esde" | "legacy" | "custom";

function detectPreset(paths: MediaPathConfig[]): Preset {
  const isEsde = DEFAULT_MEDIA_PATHS.every((def) => {
    const cur = paths.find((p) => p.mediaType === def.mediaType);
    return cur && cur.subfolder === def.subfolder && cur.suffix === def.suffix;
  });
  if (isEsde) return "esde";

  const isLegacy = LEGACY_MEDIA_PATHS.every((def) => {
    const cur = paths.find((p) => p.mediaType === def.mediaType);
    return cur && cur.subfolder === def.subfolder && cur.suffix === def.suffix;
  });
  if (isLegacy) return "legacy";

  return "custom";
}

export function MediaConfigTab({ mediaPaths, onChange }: MediaConfigTabProps) {
  const { t } = useTranslation();
  const preset = detectPreset(mediaPaths);

  function applyPreset(p: Preset) {
    if (p === "esde") onChange([...DEFAULT_MEDIA_PATHS]);
    else if (p === "legacy") onChange([...LEGACY_MEDIA_PATHS]);
  }

  function updateField(
    mediaType: string,
    field: keyof MediaPathConfig,
    value: string
  ) {
    const updated = mediaPaths.map((p) =>
      p.mediaType === mediaType ? { ...p, [field]: value } : p
    );
    onChange(updated);
  }

  const mediaTypeLabel = (id: string) =>
    AVAILABLE_MEDIA_TYPES.find((m) => m.id === id)?.label || id;

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-sm font-medium text-zinc-300 mb-1">
          {t("media.title")}
        </h3>
        <p className="text-xs text-zinc-500 mb-4">{t("media.description")}</p>
      </div>

      {/* Preset selector */}
      <div>
        <label className="text-xs text-zinc-400 mb-2 block">
          {t("media.preset")}
        </label>
        <div className="flex gap-2">
          {(
            [
              { key: "esde", label: t("media.presetEsde") },
              { key: "legacy", label: t("media.presetLegacy") },
              { key: "custom", label: t("media.presetCustom") },
            ] as const
          ).map(({ key, label }) => (
            <button
              key={key}
              onClick={() => key !== "custom" && applyPreset(key)}
              className={`px-3 py-1.5 text-sm rounded transition-colors ${
                preset === key
                  ? "bg-indigo-600 text-white"
                  : "bg-zinc-800 text-zinc-400 border border-zinc-600 hover:bg-zinc-700"
              }`}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Suffix help */}
      <p className="text-xs text-zinc-500">{t("media.suffixHelp")}</p>

      {/* Table */}
      <div className="overflow-auto border border-zinc-700 rounded">
        <table className="w-full text-sm">
          <thead className="bg-zinc-800">
            <tr>
              <th className="text-left px-3 py-2 text-zinc-400 font-medium">
                {t("media.mediaType")}
              </th>
              <th className="text-left px-3 py-2 text-zinc-400 font-medium">
                {t("media.subfolder")}
              </th>
              <th className="text-left px-3 py-2 text-zinc-400 font-medium">
                {t("media.suffix")}
              </th>
              <th className="text-left px-3 py-2 text-zinc-400 font-medium">
                {t("media.gameField")}
              </th>
            </tr>
          </thead>
          <tbody>
            {mediaPaths.map((mp) => (
              <tr
                key={mp.mediaType}
                className="border-t border-zinc-700/50"
              >
                <td className="px-3 py-2 text-zinc-300">
                  {mediaTypeLabel(mp.mediaType)}
                </td>
                <td className="px-3 py-1.5">
                  <input
                    type="text"
                    value={mp.subfolder}
                    onChange={(e) =>
                      updateField(mp.mediaType, "subfolder", e.target.value)
                    }
                    className="w-full bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
                  />
                </td>
                <td className="px-3 py-1.5">
                  <input
                    type="text"
                    value={mp.suffix}
                    onChange={(e) =>
                      updateField(mp.mediaType, "suffix", e.target.value)
                    }
                    placeholder="(none)"
                    className="w-full bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-indigo-500"
                  />
                </td>
                <td className="px-3 py-1.5">
                  <select
                    value={mp.gameField}
                    onChange={(e) =>
                      updateField(mp.mediaType, "gameField", e.target.value)
                    }
                    className="w-full bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
                  >
                    <option value="image">image</option>
                    <option value="thumbnail">thumbnail</option>
                    <option value="marquee">marquee</option>
                    <option value="video">video</option>
                  </select>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
