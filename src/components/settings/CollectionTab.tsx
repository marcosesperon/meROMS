import type { ReactNode } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "../../i18n";
import type { AppSettings, CollectionRules, Profile, VersionsAction } from "../../types";
import { collectionRulesFrom, DATA_FIELDS, MEDIA_FIELDS } from "../../utils/collectionRules";
import { getActiveProfile } from "../../utils/profiles";
import { FolderField } from "./ProfilesTab";
import { RenameSettingsTab } from "./RenameSettingsTab";

interface CollectionTabProps {
  form: AppSettings;
  setForm: (form: AppSettings) => void;
}

const VERSION_ACTIONS: VersionsAction[] = ["set_aside", "hide", "trash"];
const LEFTOVER_ACTIONS: CollectionRules["leftover_media_action"][] = ["trash", "set_aside"];

/**
 * Ajustes → Mi colección: lo que debe cumplir la colección. Datos y medios que todo juego debería
 * tener, qué hacer con las versiones que sobran y con los medios sobrantes o dañados, adónde se
 * aparta lo que se aparta (por perfil) y los nombres (lo de la antigua pestaña Renombrar).
 * Lo usan el estado del sistema y «Ponerlo todo en orden».
 */
export function CollectionTab({ form, setForm }: CollectionTabProps) {
  const { t } = useTranslation();
  const rules = collectionRulesFrom(form.collection_rules);
  const setRules = (patch: Partial<CollectionRules>) => setForm({ ...form, collection_rules: { ...rules, ...patch } });
  const toggleIn = (list: string[], field: string) => (list.includes(field) ? list.filter((f) => f !== field) : [...list, field]);
  const check = "rounded border-zinc-600 bg-zinc-700 text-indigo-600";

  // La carpeta de apartados es del perfil activo (cada perfil puede estar en otro disco)
  const profile = getActiveProfile(form);
  const setAsideDir = profile?.set_aside_dir;
  const updateProfile = (patch: Partial<Profile>) =>
    profile && setForm({ ...form, profiles: (form.profiles ?? []).map((p) => (p.id === profile.id ? { ...p, ...patch } : p)) });
  // «Apartar» sin carpeta elegida no se puede hacer: se avisa junto a la opción
  const needsFolder = !setAsideDir && (rules.versions_action === "set_aside" || rules.leftover_media_action === "set_aside");

  return (
    <div className="space-y-8">
      <p className="text-xs text-zinc-500">{t("collection.description")}</p>

      <Section title={t("collection.requiredData")} help={t("collection.requiredDataHelp")}>
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-x-4 gap-y-1.5">
          {DATA_FIELDS.map(({ field, label }) => (
            <label key={field} className="flex items-center gap-2 cursor-pointer text-sm text-zinc-300">
              <input
                type="checkbox"
                checked={rules.required_data.includes(field)}
                onChange={() => setRules({ required_data: toggleIn(rules.required_data, field) })}
                className={check}
              />
              {t(label)}
            </label>
          ))}
        </div>
      </Section>

      <Section title={t("collection.requiredMedia")} help={t("collection.requiredMediaHelp")}>
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-x-4 gap-y-1.5">
          {MEDIA_FIELDS.filter((m) => !m.classicOnly || profile?.kind !== "esde").map(({ field, label }) => (
            <label key={field} className="flex items-center gap-2 cursor-pointer text-sm text-zinc-300">
              <input
                type="checkbox"
                checked={rules.required_media.includes(field)}
                onChange={() => setRules({ required_media: toggleIn(rules.required_media, field) })}
                className={check}
              />
              {t(label)}
            </label>
          ))}
        </div>
      </Section>

      <Section title={t("collection.versions")} help={t("collection.versionsHelp")}>
        <div className="space-y-1.5">
          {VERSION_ACTIONS.map((action) => (
            <label key={action} className="flex items-start gap-2 cursor-pointer text-sm text-zinc-300">
              <input
                type="radio"
                name="versions_action"
                checked={rules.versions_action === action}
                onChange={() => setRules({ versions_action: action })}
                className="mt-0.5 border-zinc-600 bg-zinc-700 text-indigo-600"
              />
              <span>
                {t(`collection.versions_${action}`)}
                <span className="block text-xs text-zinc-500">{t(`collection.versionsHelp_${action}`)}</span>
              </span>
            </label>
          ))}
          <label className="flex items-center gap-2 cursor-pointer text-sm text-zinc-300 pt-2">
            <input type="checkbox" checked={rules.protect_played} onChange={() => setRules({ protect_played: !rules.protect_played })} className={check} />
            {t("collection.protectPlayed")}
          </label>
        </div>
      </Section>

      <Section title={t("collection.leftoverMedia")} help={t("collection.leftoverMediaHelp")}>
        <div className="space-y-1.5">
          {LEFTOVER_ACTIONS.map((action) => (
            <label key={action} className="flex items-center gap-2 cursor-pointer text-sm text-zinc-300">
              <input
                type="radio"
                name="leftover_media_action"
                checked={rules.leftover_media_action === action}
                onChange={() => setRules({ leftover_media_action: action })}
                className="border-zinc-600 bg-zinc-700 text-indigo-600"
              />
              {t(`collection.leftover_${action}`)}
            </label>
          ))}
        </div>
      </Section>

      {/* Carpeta de apartados del perfil activo (también en Perfiles, para los demás) */}
      {profile && (
        <FolderField
          label={t("collection.setAsideFolderOf", { name: profile.name })}
          help={t("collection.setAsideFolderHelp")}
          value={setAsideDir}
          placeholder={t("settings.noFolder")}
          browseLabel={t("settings.browse")}
          onBrowse={async () => {
            const folder = await openDialog({ directory: true, multiple: false });
            if (folder) updateProfile({ set_aside_dir: String(folder) });
          }}
          onClear={setAsideDir ? () => updateProfile({ set_aside_dir: undefined }) : undefined}
        >
          {needsFolder && <p className="text-xs text-amber-400 mt-1">{t("collection.setAsideMissing")}</p>}
        </FolderField>
      )}

      {/* Nombres: las opciones de Renombrar y las reglas de limpieza (antes, la pestaña Renombrar) */}
      <div className="border-t border-zinc-800 pt-6">
        <RenameSettingsTab
          options={form.rename_options}
          onOptionsChange={(options) => setForm({ ...form, rename_options: options })}
          newRomsRename={form.new_roms_rename === true}
          onNewRomsRenameChange={(on) => setForm({ ...form, new_roms_rename: on })}
          rules={form.rename_rules}
          onRulesChange={(rename_rules) => setForm({ ...form, rename_rules })}
        />
      </div>
    </div>
  );
}

function Section({ title, help, children }: { title: string; help: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="text-sm font-medium text-zinc-300 mb-1">{title}</h3>
      <p className="text-xs text-zinc-500 mb-3">{help}</p>
      {children}
    </section>
  );
}
