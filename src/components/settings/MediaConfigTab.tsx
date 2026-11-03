import { useTranslation } from "../../i18n";
import {
  AVAILABLE_MEDIA_TYPES,
  DEFAULT_MEDIA_PATHS,
  MEDIA_GAME_FIELDS,
  MEDIA_TYPE_FOLDERS,
  type MediaGameField,
  type MediaPathConfig,
} from "../../types";

interface MediaConfigTabProps {
  mediaPaths: MediaPathConfig[];
  onChange: (paths: MediaPathConfig[]) => void;
}

/**
 * Media files always follow the unified naming `<folder>/<ROM name>.<ext>` (ES-DE folder
 * names), so the folder is shown read-only. The only choice left is which gamelist field
 * each media type fills, and that only matters for the Classic structure.
 */
export function MediaConfigTab({ mediaPaths, onChange }: MediaConfigTabProps) {
  const { t } = useTranslation();

  // Configured entry or its default, so every media type always has a row
  const rows = DEFAULT_MEDIA_PATHS.map(
    (def) => mediaPaths.find((p) => p.mediaType === def.mediaType) ?? def
  );

  function updateField(mediaType: string, gameField: MediaGameField) {
    onChange(rows.map((p) => (p.mediaType === mediaType ? { ...p, gameField } : p)));
  }

  const mediaTypeLabel = (id: string) =>
    AVAILABLE_MEDIA_TYPES.find((m) => m.id === id)?.label || id;

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-sm font-medium text-zinc-300 mb-1">{t("media.title")}</h3>
        <p className="text-xs text-zinc-500">{t("media.description")}</p>
      </div>

      <div className="overflow-auto border border-zinc-700 rounded">
        <table className="w-full text-sm">
          <thead className="bg-zinc-800">
            <tr>
              <th className="text-left px-3 py-2 text-zinc-400 font-medium">{t("media.mediaType")}</th>
              <th className="text-left px-3 py-2 text-zinc-400 font-medium">{t("media.subfolder")}</th>
              <th className="text-left px-3 py-2 text-zinc-400 font-medium">{t("media.gameField")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((mp) => (
              <tr key={mp.mediaType} className="border-t border-zinc-700/50">
                <td className="px-3 py-2 text-zinc-300">{mediaTypeLabel(mp.mediaType)}</td>
                <td className="px-3 py-2 text-zinc-400 font-mono text-xs">
                  {MEDIA_TYPE_FOLDERS[mp.mediaType] ?? "—"}/
                </td>
                <td className="px-3 py-1.5">
                  <select
                    value={mp.gameField}
                    onChange={(e) => updateField(mp.mediaType, e.target.value as MediaGameField)}
                    className="w-full bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
                  >
                    {MEDIA_GAME_FIELDS.map((field) => (
                      <option key={field} value={field}>{field}</option>
                    ))}
                  </select>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="text-xs text-zinc-500">{t("media.fieldHelp")}</p>
    </div>
  );
}
