import { useTranslation } from "../../i18n";
import type { RenameOptions, RenameRule } from "../../types";
import { renameOptionsFrom } from "../../utils/renameSources";
import { RenameOptionsPanel } from "../common/RenameOptionsPanel";
import { RenameRulesTab } from "./RenameRulesTab";

interface RenameSettingsTabProps {
  options: Partial<RenameOptions> | undefined;
  onOptionsChange: (options: RenameOptions) => void;
  newRomsRename: boolean;
  onNewRomsRenameChange: (on: boolean) => void;
  rules: RenameRule[] | undefined;
  onRulesChange: (rules: RenameRule[] | undefined) => void;
}

/**
 * Ajustes → Renombrar: arriba, las mismas opciones que la ventana de Renombrar (se guardan en
 * el mismo sitio, así que cambiar aquí o allí es lo mismo) y lo de las ROMs nuevas; debajo, las
 * reglas de limpieza con su prueba.
 */
export function RenameSettingsTab({ options, onOptionsChange, newRomsRename, onNewRomsRenameChange, rules, onRulesChange }: RenameSettingsTabProps) {
  const { t } = useTranslation();
  const current = renameOptionsFrom(options);
  const check = "rounded border-zinc-600 bg-zinc-700 text-indigo-600";

  return (
    <div className="space-y-8">
      <section>
        <h3 className="text-sm font-medium text-zinc-300 mb-1">{t("renameSettings.title")}</h3>
        <p className="text-xs text-zinc-500 mb-4">{t("renameSettings.description")}</p>
        <div className="grid gap-6 md:grid-cols-2">
          <RenameOptionsPanel options={current} onChange={onOptionsChange} disabled={false} />
          <div className="space-y-3 text-sm">
            <h4 className="text-xs font-semibold text-zinc-300 uppercase tracking-wide">{t("renameSettings.applyTitle")}</h4>
            <label className="flex items-center gap-2 cursor-pointer text-zinc-300">
              <input type="checkbox" checked={current.renameFiles} onChange={() => onOptionsChange({ ...current, renameFiles: !current.renameFiles })} className={check} />
              {t("rename.renameFiles")}
            </label>
            <label className="flex items-center gap-2 cursor-pointer text-zinc-300">
              <input type="checkbox" checked={current.changeTitles} onChange={() => onOptionsChange({ ...current, changeTitles: !current.changeTitles })} className={check} />
              {t("rename.changeTitles")}
            </label>
            {/* ROMs nuevas: renombrarlas solas al añadirlas (también se marca en su ventana) */}
            <label className="flex items-start gap-2 cursor-pointer text-zinc-300 pt-2">
              <input type="checkbox" checked={newRomsRename} onChange={() => onNewRomsRenameChange(!newRomsRename)} className={`mt-0.5 ${check}`} />
              <span>
                {t("renameSettings.newRoms")}
                <span className="block text-xs text-zinc-500 mt-0.5">{t("newRoms.renameAfterHelp")}</span>
              </span>
            </label>
          </div>
        </div>
      </section>
      <div className="border-t border-zinc-800 pt-6">
        <RenameRulesTab rules={rules} onChange={onRulesChange} />
      </div>
    </div>
  );
}
