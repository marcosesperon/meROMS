import type { Game } from "../../types";
import { useTranslation } from "../../i18n";
import { formatReleaseDate, toESDate } from "../../utils/format";
import type { TranslationKeys } from "../../i18n/en";

interface GameFormProps {
  game: Game;
  onChange: (field: keyof Game, value: string) => void;
  /** Shows a "Rename..." button next to the ROM path (existing games only) */
  onRenameRom?: () => void;
}

type FieldType = "text" | "textarea" | "number" | "toggle";

interface FieldDef {
  key: keyof Game;
  labelKey: TranslationKeys;
  placeholderKey?: TranslationKeys;
  type?: FieldType;
}

const TOGGLE_FIELDS: FieldDef[] = [
  { key: "favorite", labelKey: "form.favorite", type: "toggle" },
  { key: "hidden", labelKey: "form.hidden", type: "toggle" },
  { key: "kidgame", labelKey: "form.kidgame", type: "toggle" },
];

const MAIN_FIELDS: FieldDef[] = [
  { key: "path", labelKey: "form.romPath", placeholderKey: "form.romPathPlaceholder" },
  { key: "name", labelKey: "form.name", placeholderKey: "form.namePlaceholder" },
  { key: "sortname", labelKey: "form.sortname" },
  { key: "desc", labelKey: "form.description", type: "textarea" },
  { key: "genre", labelKey: "form.genre", placeholderKey: "form.genrePlaceholder" },
  { key: "developer", labelKey: "form.developer" },
  { key: "publisher", labelKey: "form.publisher" },
  { key: "players", labelKey: "form.players", type: "number", placeholderKey: "form.playersPlaceholder" },
  { key: "rating", labelKey: "form.rating", placeholderKey: "form.ratingPlaceholder" },
  { key: "releasedate", labelKey: "form.releaseDate", placeholderKey: "form.releaseDatePlaceholder" },
  { key: "region", labelKey: "form.region" },
  { key: "lang", labelKey: "form.lang" },
  { key: "family", labelKey: "form.family" },
  { key: "arcadesystemname", labelKey: "form.arcadesystemname" },
  { key: "emulator", labelKey: "form.emulator" },
  { key: "core", labelKey: "form.core" },
  { key: "gametime", labelKey: "form.gametime", type: "number" },
  { key: "hash", labelKey: "form.hash" },
];

export function GameForm({ game, onChange, onRenameRom }: GameFormProps) {
  const { t } = useTranslation();

  function getValue(field: FieldDef): string {
    const raw = game[field.key] || "";
    if (field.key === "releasedate" && raw) {
      return formatReleaseDate(raw);
    }
    return raw;
  }

  function handleChange(field: FieldDef, value: string) {
    if (field.key === "releasedate" && value) {
      onChange(field.key, toESDate(value));
    } else {
      onChange(field.key, value);
    }
  }

  function handleToggle(key: keyof Game) {
    const current = game[key] === "true";
    onChange(key, current ? "" : "true");
  }

  return (
    <div className="space-y-4">
      {/* Boolean toggles */}
      <div className="flex items-center gap-4">
        {TOGGLE_FIELDS.map((field) => {
          const isActive = game[field.key] === "true";
          return (
            <label
              key={field.key}
              className="flex items-center gap-2 cursor-pointer select-none"
            >
              <button
                type="button"
                onClick={() => handleToggle(field.key)}
                className={`w-8 h-5 rounded-full transition-colors relative ${
                  isActive ? "bg-indigo-600" : "bg-zinc-600"
                }`}
              >
                <span
                  className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
                    isActive ? "left-3.5" : "left-0.5"
                  }`}
                />
              </button>
              <span className="text-xs text-zinc-400">{t(field.labelKey)}</span>
            </label>
          );
        })}
      </div>

      {/* Main fields */}
      {MAIN_FIELDS.map((field) => (
        <div key={field.key}>
          <label className="text-xs text-zinc-400 mb-1 block">
            {t(field.labelKey)}
          </label>
          {field.type === "textarea" ? (
            <textarea
              value={getValue(field)}
              onChange={(e) => handleChange(field, e.target.value)}
              rows={4}
              className="w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500 resize-none"
            />
          ) : (
            <div className="flex gap-2">
              <input
                type={field.type || "text"}
                value={getValue(field)}
                onChange={(e) => handleChange(field, e.target.value)}
                placeholder={field.placeholderKey ? t(field.placeholderKey) : undefined}
                className="w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
              />
              {/* Renames the file on disk (the field alone only changes the gamelist) */}
              {field.key === "path" && onRenameRom && (
                <button
                  type="button"
                  onClick={onRenameRom}
                  className="shrink-0 px-2.5 text-xs bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
                >
                  {t("gameTools.renameRom")}
                </button>
              )}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
