import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import { getActiveProfile } from "../../utils/profiles";

/**
 * Aviso al entrar en un perfil: hay sistemas con datos en la otra estructura (y nada en
 * la del perfil). «Migrar» abre la ventana de migración ya rellena; «Ahora no» lo deja
 * para luego (cada sistema sigue teniendo su franja con «Migrar…»).
 */
export function StructurePromptModal() {
  const { t } = useTranslation();
  const open = useAppStore((s) => s.structurePromptOpen);
  const pending = useAppStore((s) => s.pendingStructure);
  const settings = useAppStore((s) => s.settings);
  const closeStructurePrompt = useAppStore((s) => s.closeStructurePrompt);
  const openPresetMigration = useAppStore((s) => s.openPresetMigration);
  const [dontAskAgain, setDontAskAgain] = useState(false);

  useEffect(() => {
    if (open) setDontAskAgain(false);
  }, [open]);

  if (!open || !pending) return null;

  const target = getActiveProfile(settings);
  const kindLabel = (kind: string | undefined) => (kind === "esde" ? "ES-DE" : t("profiles.kindClassic"));

  async function handleMigrate() {
    await closeStructurePrompt(dontAskAgain);
    openPresetMigration();
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-[60]">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg p-6 max-w-md w-full mx-4">
        <h3 className="text-lg font-semibold text-zinc-100 mb-2">{t("structure.promptTitle")}</h3>
        <p className="text-zinc-400 text-sm">
          {t("structure.promptMessage", {
            count: pending.systems.length,
            kind: kindLabel(pending.source.kind),
            target: kindLabel(target?.kind),
          })}
        </p>
        <p className="text-xs text-zinc-500 mt-2 break-words">{pending.systems.join(", ")}</p>
        {/* Si viene de otra carpeta de ES-DE, que se vea de dónde */}
        {pending.source.esde_home_path && (
          <p className="text-xs text-zinc-500 mt-1 break-all">
            {t("structure.promptSource", { path: pending.source.esde_home_path })}
          </p>
        )}

        <label className="flex items-center gap-2 mt-4 text-sm text-zinc-400 cursor-pointer">
          <input
            type="checkbox"
            checked={dontAskAgain}
            onChange={(e) => setDontAskAgain(e.target.checked)}
            className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
          />
          {t("structure.dontAsk")}
        </label>

        <div className="flex justify-end gap-3 mt-6">
          <button
            onClick={() => closeStructurePrompt(dontAskAgain)}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("structure.notNow")}
          </button>
          <button
            onClick={handleMigrate}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
          >
            {t("structure.migrate")}
          </button>
        </div>
      </div>
    </div>
  );
}
