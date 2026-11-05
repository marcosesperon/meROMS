import { useEffect, useState, type ReactNode } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "../../i18n";
import { detectEsdePaths, suggestedBiosDir } from "../../commands";
import type { EsdeDetectedPaths, Profile, StructureKind } from "../../types";
import { createProfile, isDefaultEsdeMedia, isProfileComplete } from "../../utils/profiles";
import { TrashIcon } from "../common/Icons";
import { askConfirm } from "../../utils/dialogs";

interface ProfilesTabProps {
  profiles: Profile[];
  activeProfileId?: string;
  onChange: (profiles: Profile[], activeProfileId?: string) => void;
  /** Abrir la migración de estructura con este perfil como origen */
  onMigrate: (profile: Profile) => void;
  /** Usar este perfil ya: se guardan los ajustes y pasa a ser el activo */
  onActivate: (profileId: string) => void;
}

// Pestaña de perfiles: a la izquierda la lista, a la derecha el perfil que se está editando.
// Los cambios van al formulario de ajustes y se guardan con el botón «Guardar» general.
export function ProfilesTab({ profiles, activeProfileId, onChange, onMigrate, onActivate }: ProfilesTabProps) {
  const { t } = useTranslation();
  const [editingId, setEditingId] = useState<string | undefined>(activeProfileId ?? profiles[0]?.id);
  const [detected, setDetected] = useState<EsdeDetectedPaths | null>(null);
  const [detectError, setDetectError] = useState<string | null>(null);

  const editing = profiles.find((p) => p.id === editingId);
  // Carpeta de BIOS que se usaría si se deja vacía (se enseña como ejemplo)
  const [suggestedBios, setSuggestedBios] = useState<string | null>(null);
  useEffect(() => {
    if (!editing) return;
    suggestedBiosDir(editing).then(setSuggestedBios).catch(() => setSuggestedBios(null));
  }, [editing?.rom_root_path, editing?.esde_home_path, editing?.kind]);

  function updateProfile(changes: Partial<Profile>) {
    if (!editing) return;
    onChange(
      profiles.map((p) => (p.id === editing.id ? { ...p, ...changes } : p)),
      activeProfileId
    );
  }

  function handleAdd() {
    const profile = createProfile(t("profiles.defaultName"), "classic");
    // El primer perfil que se crea pasa a ser el activo directamente
    onChange([...profiles, profile], activeProfileId ?? profile.id);
    setEditingId(profile.id);
    setDetected(null);
  }

  async function handleDelete() {
    if (!editing || !(await askConfirm(t("profiles.deleteConfirm", { name: editing.name })))) return;
    const remaining = profiles.filter((p) => p.id !== editing.id);
    const nextActive = activeProfileId === editing.id ? remaining[0]?.id : activeProfileId;
    onChange(remaining, nextActive);
    setEditingId(remaining[0]?.id);
    setDetected(null);
  }

  function handleKindChange(kind: StructureKind) {
    updateProfile(kind === "classic" ? { kind, esde_home_path: undefined, media_dir: undefined } : { kind });
    setDetected(null);
  }

  async function pickFolder(): Promise<string | null> {
    const selected = await openDialog({ directory: true, multiple: false });
    return selected ? String(selected) : null;
  }

  async function handlePickEsdeHome() {
    const folder = await pickFolder();
    if (!folder) return;
    setDetectError(null);
    try {
      const paths = await detectEsdePaths(folder);
      setDetected(paths);
      // La carpeta de datos detectada (en portable no es la elegida); la de medios solo si
      // no es la de por defecto de ES-DE
      updateProfile({
        esde_home_path: paths.esde_home,
        rom_root_path: paths.rom_root_path,
        media_dir: isDefaultEsdeMedia(paths) ? undefined : paths.media_dir,
      });
    } catch (e) {
      setDetected(null);
      setDetectError(String(e));
      updateProfile({ esde_home_path: folder });
    }
  }

  return (
    <div className="flex gap-6">
      {/* Lista de perfiles */}
      <div className="w-56 shrink-0 space-y-2">
        <p className="text-xs text-zinc-500 mb-3">{t("profiles.description")}</p>
        {profiles.length === 0 && <p className="text-sm text-zinc-500">{t("profiles.none")}</p>}
        {profiles.map((p) => (
          <button
            key={p.id}
            onClick={() => {
              setEditingId(p.id);
              setDetected(null);
              setDetectError(null);
            }}
            className={`w-full text-left px-3 py-2 rounded border transition-colors ${
              p.id === editingId
                ? "border-indigo-500 bg-indigo-600/10"
                : "border-zinc-700 hover:bg-zinc-800"
            }`}
          >
            <div className="flex items-center justify-between gap-2">
              <span className="text-sm text-zinc-200 truncate">{p.name || t("profiles.defaultName")}</span>
              {p.id === activeProfileId && (
                <span className="text-[10px] px-1.5 py-0.5 rounded bg-indigo-600 text-white shrink-0">
                  {t("profiles.active")}
                </span>
              )}
            </div>
            <span className="text-xs text-zinc-500">
              {p.kind === "esde" ? t("profiles.kindEsde") : t("profiles.kindClassic")}
              {!isProfileComplete(p) && ` · ${t("profiles.incomplete")}`}
            </span>
          </button>
        ))}
        <button
          onClick={handleAdd}
          className="w-full px-3 py-2 text-sm bg-zinc-800 text-zinc-300 border border-dashed border-zinc-600 rounded hover:bg-zinc-700 transition-colors"
        >
          {t("profiles.add")}
        </button>
      </div>

      {/* Edición del perfil seleccionado */}
      {editing ? (
        <div className="flex-1 space-y-6">
          <section>
            <label className="text-xs text-zinc-400 mb-1 block">{t("profiles.name")}</label>
            <input
              type="text"
              value={editing.name}
              onChange={(e) => updateProfile({ name: e.target.value })}
              className="w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
            />
          </section>

          <section>
            <label className="text-xs text-zinc-400 mb-2 block">{t("profiles.kind")}</label>
            <div className="flex gap-2">
              {(["classic", "esde"] as StructureKind[]).map((kind) => (
                <button
                  key={kind}
                  onClick={() => handleKindChange(kind)}
                  className={`px-4 py-2 text-sm rounded transition-colors ${
                    editing.kind === kind
                      ? "bg-indigo-600 text-white"
                      : "bg-zinc-800 text-zinc-400 border border-zinc-600 hover:bg-zinc-700"
                  }`}
                >
                  {kind === "esde" ? t("profiles.kindEsde") : t("profiles.kindClassic")}
                </button>
              ))}
            </div>
            <p className="text-xs text-zinc-500 mt-2">
              {editing.kind === "esde" ? t("profiles.kindEsdeHelp") : t("profiles.kindClassicHelp")}
            </p>
          </section>

          {editing.kind === "esde" && (
            <FolderField
              label={t("profiles.esdeHome")}
              help={t("profiles.esdeHomeHelp")}
              value={editing.esde_home_path}
              placeholder={t("settings.noFolder")}
              browseLabel={t("settings.browse")}
              onBrowse={handlePickEsdeHome}
            >
              {detectError && <p className="text-xs text-red-400 mt-2">{detectError}</p>}
              {detected?.portable && (
                <p className="text-xs text-emerald-400 mt-2">{t("profiles.portableDetected", { path: detected.esde_home })}</p>
              )}
              {detected && !detected.settings_found && (
                <p className="text-xs text-amber-400 mt-2">{t("profiles.settingsNotFound")}</p>
              )}
              {detected?.legacy_gamelist_location && (
                <p className="text-xs text-amber-400 mt-2">{t("profiles.legacyWarning")}</p>
              )}
            </FolderField>
          )}

          <FolderField
            label={t("settings.romFolder")}
            help={t("settings.romFolderHelp")}
            value={editing.rom_root_path}
            placeholder={t("settings.noFolder")}
            browseLabel={t("settings.browse")}
            onBrowse={async () => {
              const folder = await pickFolder();
              if (folder) updateProfile({ rom_root_path: folder });
            }}
          />

          {editing.kind === "esde" && (
            <FolderField
              label={t("profiles.mediaDir")}
              help={t("profiles.mediaDirHelp")}
              value={editing.media_dir}
              placeholder={
                editing.esde_home_path ? `${editing.esde_home_path}/downloaded_media` : t("settings.noFolder")
              }
              browseLabel={t("settings.browse")}
              onBrowse={async () => {
                const folder = await pickFolder();
                if (folder) updateProfile({ media_dir: folder });
              }}
              onClear={editing.media_dir ? () => updateProfile({ media_dir: undefined }) : undefined}
            />
          )}

          {/* Carpeta de BIOS (Herramientas → BIOS); vacía = la habitual junto a las ROMs */}
          <FolderField
            label={t("bios.folder")}
            help={t("bios.folderHelp")}
            value={editing.bios_dir}
            placeholder={suggestedBios ?? t("settings.noFolder")}
            browseLabel={t("settings.browse")}
            onBrowse={async () => {
              const folder = await pickFolder();
              if (folder) updateProfile({ bios_dir: folder });
            }}
            onClear={editing.bios_dir ? () => updateProfile({ bios_dir: undefined }) : undefined}
          />

          <div className="flex items-center gap-3 pt-2 border-t border-zinc-700/50">
            {editing.id !== activeProfileId && (
              // Se aplica al momento (antes solo cambiaba el formulario y había que pulsar
              // «Guardar Ajustes»). Sin carpetas no se puede usar.
              <button
                onClick={() => onActivate(editing.id)}
                disabled={!isProfileComplete(editing)}
                className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
              >
                {t("profiles.activate")}
              </button>
            )}
            {/* Sin carpetas no hay nada que migrar */}
            <button
              onClick={() => onMigrate(editing)}
              disabled={!isProfileComplete(editing)}
              className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              {t("migration.button")}
            </button>
            <div className="flex-1" />
            <button
              onClick={handleDelete}
              className="flex items-center gap-1.5 px-3 py-2 text-sm text-red-400 border border-red-900/60 rounded hover:bg-red-950/40 transition-colors"
            >
              <TrashIcon size={14} />
              {t("profiles.delete")}
            </button>
          </div>
        </div>
      ) : (
        <div className="flex-1" />
      )}
    </div>
  );
}

interface FolderFieldProps {
  label: string;
  help: string;
  value?: string;
  placeholder: string;
  browseLabel: string;
  onBrowse: () => void;
  onClear?: () => void;
  children?: ReactNode;
}

// Campo de carpeta de solo lectura con su botón «Examinar» (se repite tres veces arriba).
function FolderField({ label, help, value, placeholder, browseLabel, onBrowse, onClear, children }: FolderFieldProps) {
  return (
    <section>
      <label className="text-xs text-zinc-400 mb-1 block">{label}</label>
      <div className="flex gap-3">
        <input
          type="text"
          readOnly
          value={value || ""}
          placeholder={placeholder}
          className="flex-1 bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500"
        />
        {onClear && (
          <button
            onClick={onClear}
            className="px-3 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            ×
          </button>
        )}
        <button
          onClick={onBrowse}
          className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
        >
          {browseLabel}
        </button>
      </div>
      <p className="text-xs text-zinc-500 mt-2">{help}</p>
      {children}
    </section>
  );
}
