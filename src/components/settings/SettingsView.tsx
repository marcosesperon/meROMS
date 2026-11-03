import { useState, useEffect, useRef } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation, useI18nStore, type Locale } from "../../i18n";
import { CloseIcon, DownloadIcon, CopyIcon } from "../common/Icons";
import {
  AVAILABLE_MEDIA_TYPES,
  DEFAULT_MEDIA_PATHS,
  DEFAULT_MEDIA_TYPES,
  GRID_MEDIA_OPTIONS,
  type MediaPathConfig,
} from "../../types";
import { MediaConfigTab } from "./MediaConfigTab";
import { ProfilesTab } from "./ProfilesTab";
import { SystemsConfigTab } from "./SystemsConfigTab";
import { exportSettings, importSettings } from "../../commands";

type SettingsTab = "profiles" | "general" | "media" | "systems";

interface SettingsViewProps {
  open: boolean;
  onClose: () => void;
}

export function SettingsView({ open, onClose }: SettingsViewProps) {
  const settings = useAppStore((s) => s.settings);
  const updateSettings = useAppStore((s) => s.updateSettings);
  const systems = useAppStore((s) => s.systems);
  const { t } = useTranslation();
  const locale = useI18nStore((s) => s.locale);
  const setLocale = useI18nStore((s) => s.setLocale);

  const [form, setForm] = useState(settings);
  const [activeTab, setActiveTab] = useState<SettingsTab>("profiles");
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setForm(settings);
  }, [settings]);

  const selectedMedia = form.ss_media_types ?? [...DEFAULT_MEDIA_TYPES];
  const mediaPaths: MediaPathConfig[] =
    form.media_paths ?? [...DEFAULT_MEDIA_PATHS];

  async function handleSave() {
    await updateSettings(form);
  }

  async function handleExport() {
    const dest = await saveDialog({
      defaultPath: "meroms-settings.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (dest) {
      try {
        await exportSettings(dest);
        alert(t("settings.exportSuccess"));
      } catch (e) {
        console.error("Export failed:", e);
      }
    }
  }

  async function handleImport() {
    const file = await openDialog({
      filters: [{ name: "JSON", extensions: ["json"] }],
      multiple: false,
    });
    if (file) {
      try {
        const imported = await importSettings(file as string);
        setForm(imported);
        if (imported.language === "es" || imported.language === "en") {
          setLocale(imported.language);
        }
        alert(t("settings.importSuccess"));
      } catch {
        alert(t("settings.importError"));
      }
    }
  }

  function toggleMediaType(id: string) {
    const current = new Set(selectedMedia);
    if (current.has(id)) {
      current.delete(id);
    } else {
      current.add(id);
    }
    setForm({ ...form, ss_media_types: Array.from(current) });
  }

  const tabs: { id: SettingsTab; label: string }[] = [
    { id: "profiles", label: t("settings.tabProfiles") },
    { id: "general", label: t("settings.tabGeneral") },
    { id: "media", label: t("settings.tabMedia") },
    { id: "systems", label: t("settings.tabSystems") },
  ];

  function handleTabChange(tab: SettingsTab) {
    setActiveTab(tab);
    scrollRef.current?.scrollTo({ top: 0 });
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-8 bg-black/60" onClick={onClose}>
      <div
        className="bg-zinc-900 border border-zinc-700 rounded-lg shadow-2xl w-full max-w-4xl max-h-full flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
      {/* Header with tabs — fixed */}
      <div className="px-8 pt-6 pb-0 shrink-0">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-semibold text-zinc-100">
            {t("settings.title")}
          </h2>
          <button
            onClick={onClose}
            className="text-zinc-400 hover:text-zinc-200 transition-colors"
          >
            <CloseIcon size={20} />
          </button>
        </div>
        <div className="flex gap-1 border-b border-zinc-700">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => handleTabChange(tab.id)}
              className={`px-4 py-2 text-sm font-medium transition-colors border-b-2 -mb-px ${
                activeTab === tab.id
                  ? "border-indigo-500 text-indigo-400"
                  : "border-transparent text-zinc-400 hover:text-zinc-200"
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Tab content — scrollable */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-8">
        <div className="space-y-8">
          {activeTab === "general" && (
            <>
              {/* Language */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">
                  {t("settings.language")}
                </h3>
                <div className="flex gap-2">
                  {(["en", "es"] as Locale[]).map((lang) => (
                    <button
                      key={lang}
                      onClick={() => {
                        setLocale(lang);
                        setForm({ ...form, language: lang });
                      }}
                      className={`px-4 py-2 text-sm rounded transition-colors ${
                        locale === lang
                          ? "bg-indigo-600 text-white"
                          : "bg-zinc-800 text-zinc-400 border border-zinc-600 hover:bg-zinc-700"
                      }`}
                    >
                      {lang === "en" ? "English" : "Español"}
                    </button>
                  ))}
                </div>
              </section>

              {/* ScreenScraper Credentials */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">
                  {t("settings.ssCredentials")}
                </h3>
                <div className="space-y-3">
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="text-xs text-zinc-400 mb-1 block">
                        {t("settings.devId")}
                      </label>
                      <input
                        type="text"
                        value={form.ss_devid || ""}
                        onChange={(e) =>
                          setForm({ ...form, ss_devid: e.target.value || undefined })
                        }
                        placeholder="devid"
                        className="w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                      />
                    </div>
                    <div>
                      <label className="text-xs text-zinc-400 mb-1 block">
                        {t("settings.devPassword")}
                      </label>
                      <input
                        type="password"
                        value={form.ss_devpassword || ""}
                        onChange={(e) =>
                          setForm({
                            ...form,
                            ss_devpassword: e.target.value || undefined,
                          })
                        }
                        placeholder="devpassword"
                        className="w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                      />
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="text-xs text-zinc-400 mb-1 block">
                        {t("settings.username")}
                      </label>
                      <input
                        type="text"
                        value={form.ss_user || ""}
                        onChange={(e) =>
                          setForm({ ...form, ss_user: e.target.value || undefined })
                        }
                        placeholder="screenscraper username"
                        className="w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                      />
                    </div>
                    <div>
                      <label className="text-xs text-zinc-400 mb-1 block">
                        {t("settings.password")}
                      </label>
                      <input
                        type="password"
                        value={form.ss_password || ""}
                        onChange={(e) =>
                          setForm({
                            ...form,
                            ss_password: e.target.value || undefined,
                          })
                        }
                        placeholder="screenscraper password"
                        className="w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                      />
                    </div>
                  </div>
                  <p className="text-xs text-zinc-500">{t("settings.ssHelp")}</p>
                </div>
              </section>

              {/* TheGamesDB */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">
                  {t("settings.tgdbTitle")}
                </h3>
                <div>
                  <label className="text-xs text-zinc-400 mb-1 block">
                    {t("settings.tgdbApiKey")}
                  </label>
                  <input
                    type="text"
                    value={form.tgdb_apikey || ""}
                    onChange={(e) =>
                      setForm({ ...form, tgdb_apikey: e.target.value || undefined })
                    }
                    placeholder="TheGamesDB API key"
                    className="w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                  />
                  <p className="text-xs text-zinc-500 mt-2">
                    {t("settings.tgdbHelp")}
                  </p>
                </div>
              </section>

              {/* Media Types to download */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">
                  {t("settings.mediaTitle")}
                </h3>
                <div className="grid grid-cols-2 gap-2">
                  {AVAILABLE_MEDIA_TYPES.map(({ id, label }) => (
                    <label
                      key={id}
                      className="flex items-center gap-2 text-sm text-zinc-300 cursor-pointer hover:text-zinc-100"
                    >
                      <input
                        type="checkbox"
                        checked={selectedMedia.includes(id)}
                        onChange={() => toggleMediaType(id)}
                        className="rounded border-zinc-600 bg-zinc-800 text-indigo-600 focus:ring-indigo-500"
                      />
                      {label}
                    </label>
                  ))}
                </div>
              </section>

              {/* Grid media field */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">
                  {t("settings.gridMedia")}
                </h3>
                <select
                  value={form.grid_media_field || "image"}
                  onChange={(e) => setForm({ ...form, grid_media_field: e.target.value })}
                  className="w-full max-w-xs bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
                >
                  {GRID_MEDIA_OPTIONS.map(({ id, labelKey }) => (
                    <option key={id} value={id}>
                      {t(labelKey as any)}
                    </option>
                  ))}
                </select>
                <p className="text-xs text-zinc-500 mt-2">
                  {t("settings.gridMediaHelp")}
                </p>
              </section>
            </>
          )}

          {activeTab === "profiles" && (
            <ProfilesTab
              profiles={form.profiles ?? []}
              activeProfileId={form.active_profile_id}
              onChange={(profiles, activeProfileId) =>
                setForm({ ...form, profiles, active_profile_id: activeProfileId })
              }
            />
          )}

          {activeTab === "media" && (
            <MediaConfigTab
              mediaPaths={mediaPaths}
              onChange={(paths) => setForm({ ...form, media_paths: paths })}
            />
          )}

          {activeTab === "systems" && (
            <SystemsConfigTab
              systems={systems}
              systemConfigs={form.system_configs || {}}
              onChange={(configs) =>
                setForm({
                  ...form,
                  system_configs: Object.keys(configs).length > 0 ? configs : undefined,
                })
              }
            />
          )}

        </div>
      </div>

      {/* Footer — fixed */}
      <div className="px-8 py-4 border-t border-zinc-700 shrink-0 flex items-center gap-3">
        <button
          onClick={handleExport}
          className="flex items-center gap-1.5 px-3 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          title={t("settings.exportSettings")}
        >
          <DownloadIcon size={14} />
          {t("settings.exportSettings")}
        </button>
        <button
          onClick={handleImport}
          className="flex items-center gap-1.5 px-3 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          title={t("settings.importSettings")}
        >
          <CopyIcon size={14} />
          {t("settings.importSettings")}
        </button>
        <div className="flex-1" />
        <button
          onClick={onClose}
          className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
        >
          {t("detail.cancel")}
        </button>
        <button
          onClick={async () => { await handleSave(); onClose(); }}
          className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
        >
          {t("settings.save")}
        </button>
      </div>
      </div>
    </div>
  );
}
