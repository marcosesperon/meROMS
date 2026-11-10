import { useState, useEffect, useRef } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation, useI18nStore, type Locale } from "../../i18n";
import { CloseIcon, DownloadIcon, CopyIcon } from "../common/Icons";
import {
  DEFAULT_MEDIA_PATHS,
  GRID_MEDIA_OPTIONS,
  type MediaPathConfig,
} from "../../types";
import { MediaConfigTab } from "./MediaConfigTab";
import { ProfilesTab } from "./ProfilesTab";
import { RenameRulesTab } from "./RenameRulesTab";
import { AboutTab } from "./AboutTab";
import { PlaceholdersSetting } from "./PlaceholdersSetting";
import { LaunchBoxSetting } from "./LaunchBoxSetting";
import { BiosListSetting } from "./BiosListSetting";
import { exportSettings, importSettings } from "../../commands";
import { askConfirm, showMessage } from "../../utils/dialogs";
import { ScrapePolicyEditor } from "../common/ScrapePolicyEditor";
import { getScrapeOrder, getScrapePolicy } from "../../commands";
import { ScrapeOrderEditor } from "./ScrapeOrderEditor";
import { MigrationModal } from "../common/MigrationModal";
import type { AppSettings, Profile } from "../../types";

type AppSettingsSort = AppSettings["gamelist_sort"];

type SettingsTab = "profiles" | "general" | "media" | "rules" | "about";

interface SettingsViewProps {
  open: boolean;
  onClose: () => void;
}

export function SettingsView({ open, onClose }: SettingsViewProps) {
  const settings = useAppStore((s) => s.settings);
  const updateSettings = useAppStore((s) => s.updateSettings);
  const { t } = useTranslation();
  const locale = useI18nStore((s) => s.locale);
  const setLocale = useI18nStore((s) => s.setLocale);

  const [form, setForm] = useState(settings);
  const builtInDev = useAppStore((s) => s.scraperStatus.built_in_dev);
  // The ScreenScraper account only makes sense with developer credentials (built-in or typed)
  const showSsAccount = builtInDev || (!!form.ss_devid?.trim() && !!form.ss_devpassword?.trim());
  // «Avanzado» starts open only when there's nothing else to show; then the user decides
  // (not tied to showSsAccount, or it would fold while typing the developer fields)
  const [advancedOpen, setAdvancedOpen] = useState(!showSsAccount);
  useEffect(() => {
    if (open) setAdvancedOpen(!showSsAccount);
    // Only when the settings window opens
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);
  const [activeTab, setActiveTab] = useState<SettingsTab>("profiles");
  const scrollRef = useRef<HTMLDivElement>(null);
  // Perfil que se está migrando (la ventana de migración está abierta mientras no sea null)
  const [migrating, setMigrating] = useState<Profile | null>(null);

  useEffect(() => {
    setForm(settings);
  }, [settings]);

  // Values this window filled in by itself (scraper policy and region/language order, when
  // they weren't saved yet). They don't count as changes while they stay as filled in.
  const filledIn = useRef<Partial<Record<"scrape_fields" | "ss_regions" | "ss_languages", string>>>({});
  function withoutFilledIn(values: typeof form) {
    const copy: Record<string, unknown> = { ...values };
    for (const [key, json] of Object.entries(filledIn.current)) {
      if (JSON.stringify(copy[key]) === json) delete copy[key];
    }
    return copy;
  }

  // Not saved yet: start from what the backend derives (old "media to download" list)
  useEffect(() => {
    if (open && !form.scrape_fields) {
      getScrapePolicy()
        .then((scrape_fields) => {
          filledIn.current.scrape_fields = JSON.stringify(scrape_fields);
          setForm((prev) => (prev.scrape_fields ? prev : { ...prev, scrape_fields }));
        })
        .catch(() => {});
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, form.scrape_fields]);

  // Same for the region/language order
  useEffect(() => {
    if (open && (!form.ss_regions || !form.ss_languages)) {
      getScrapeOrder()
        .then(({ regions, languages }) => {
          filledIn.current.ss_regions = JSON.stringify(regions);
          filledIn.current.ss_languages = JSON.stringify(languages);
          setForm((prev) => ({ ...prev, ss_regions: prev.ss_regions ?? regions, ss_languages: prev.ss_languages ?? languages }));
        })
        .catch(() => {});
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, form.ss_regions, form.ss_languages]);

  const mediaPaths: MediaPathConfig[] =
    form.media_paths ?? [...DEFAULT_MEDIA_PATHS];

  async function handleSave() {
    await updateSettings(form);
  }

  // «Usar este perfil»: se guarda todo lo del formulario y ese perfil pasa a ser el activo
  // (los sistemas se vuelven a cargar en el momento)
  async function handleActivate(profileId: string) {
    await updateSettings({ ...form, active_profile_id: profileId });
  }

  // La migración trabaja con los ajustes guardados (y al acabar puede crear y activar un
  // perfil), así que lo pendiente del formulario se guarda antes o no se abre
  async function handleMigrate(profile: Profile) {
    const dirty = JSON.stringify(withoutFilledIn(form)) !== JSON.stringify(withoutFilledIn(settings));
    if (dirty) {
      if (!(await askConfirm(t("profiles.migrateUnsaved")))) return;
      await handleSave();
    }
    setMigrating(profile);
  }

  async function handleExport() {
    const dest = await saveDialog({
      defaultPath: "meroms-settings.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (dest) {
      try {
        await exportSettings(dest);
        await showMessage(t("settings.exportSuccess"));
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
        await showMessage(t("settings.importSuccess"));
      } catch {
        await showMessage(t("settings.importError"), "error");
      }
    }
  }


  const tabs: { id: SettingsTab; label: string }[] = [
    { id: "profiles", label: t("settings.tabProfiles") },
    { id: "general", label: t("settings.tabGeneral") },
    { id: "media", label: t("settings.tabMedia") },
    { id: "rules", label: t("settings.tabRules") },
    { id: "about", label: t("settings.tabAbout") },
  ];

  function handleTabChange(tab: SettingsTab) {
    setActiveTab(tab);
    scrollRef.current?.scrollTo({ top: 0 });
  }

  if (!open) return null;

  return (
    <>
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
            data-shot="settings-close"
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
              data-shot={`settings-tab-${tab.id}`}
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

              {/* ScreenScraper: usuario y contraseña solo si hay credenciales de desarrollador
                  (las que lleva la app o las de «Avanzado»); estas, plegadas en «Avanzado» */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">
                  {t("settings.ssCredentials")}
                </h3>
                <div className="space-y-3">
                  {showSsAccount && (
                    <>
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
                    </>
                  )}
                  <details open={advancedOpen} onToggle={(e) => setAdvancedOpen(e.currentTarget.open)}>
                    <summary className="cursor-pointer text-xs text-zinc-400 hover:text-zinc-200 select-none">
                      {t("settings.advanced")}
                    </summary>
                    <div className="grid grid-cols-2 gap-3 mt-3">
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
                  </details>
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

              {/* BIOS list (libretro System.dat): download / update / delete */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">BIOS</h3>
                <BiosListSetting />
              </section>

              {/* LaunchBox: offline database (download / update / delete) */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">LaunchBox</h3>
                <LaunchBoxSetting />
              </section>

              {/* Region / language order of the scraper */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">{t("scrapeOrder.title")}</h3>
                {form.ss_regions && form.ss_languages && (
                  <ScrapeOrderEditor
                    regions={form.ss_regions}
                    languages={form.ss_languages}
                    onChange={(ss_regions, ss_languages) => setForm({ ...form, ss_regions, ss_languages })}
                  />
                )}
              </section>

              {/* What the scraper may update (replaces the old "media to download" list) */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">{t("scrapePolicy.title")}</h3>
                {form.scrape_fields && (
                  <ScrapePolicyEditor
                    policy={form.scrape_fields}
                    onChange={(scrape_fields) => setForm({ ...form, scrape_fields })}
                  />
                )}
                {/* Rellenos aprendidos en «Limpiar media» (el scraper los ignora) */}
                <PlaceholdersSetting />
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
                {/* Cajas 3D: cada sistema puede cambiarlo en su vista (botón de deslizadores) */}
                <label className="flex items-start gap-2 mt-4 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={!!form.grid_boxes_3d}
                    onChange={(e) => setForm({ ...form, grid_boxes_3d: e.target.checked ? true : undefined })}
                    className="mt-0.5 rounded border-zinc-600 bg-zinc-700 text-indigo-600"
                  />
                  <span>
                    <span className="block text-sm text-zinc-300">{t("settings.boxes3d")}</span>
                    <span className="block text-xs text-zinc-500 mt-0.5">{t("settings.boxes3dHelp")}</span>
                  </span>
                </label>
              </section>

              {/* What a click on a game does: right panel or the full-screen page */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">{t("settings.clickAction")}</h3>
                <select
                  value={form.click_action ?? "panel"}
                  onChange={(e) =>
                    setForm({ ...form, click_action: e.target.value === "full" ? "full" : undefined })
                  }
                  className="w-full max-w-xs bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
                >
                  <option value="panel">{t("settings.clickAction_panel")}</option>
                  <option value="full">{t("settings.clickAction_full")}</option>
                </select>
                <p className="text-xs text-zinc-500 mt-2">{t("settings.clickActionHelp")}</p>
              </section>

              {/* Order of the games when a gamelist is written */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">{t("settings.gamelistSort")}</h3>
                <select
                  value={form.gamelist_sort ?? ""}
                  onChange={(e) =>
                    setForm({ ...form, gamelist_sort: (e.target.value || undefined) as AppSettingsSort })
                  }
                  className="w-full max-w-xs bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
                >
                  <option value="">{t("settings.gamelistSortNone")}</option>
                  <option value="name">{t("settings.gamelistSortName")}</option>
                  <option value="path">{t("settings.gamelistSortPath")}</option>
                </select>
                <p className="text-xs text-zinc-500 mt-2">{t("settings.gamelistSortHelp")}</p>
              </section>

              {/* DeepL key to translate descriptions */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">{t("translate.settingsTitle")}</h3>
                <input
                  type="text"
                  value={form.deepl_apikey || ""}
                  onChange={(e) => setForm({ ...form, deepl_apikey: e.target.value || undefined })}
                  placeholder="DeepL API key"
                  className="w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                />
                <p className="text-xs text-zinc-500 mt-2">{t("translate.settingsHelp")}</p>
              </section>

              {/* chdman for "Convert to CHD" (empty = look for it) */}
              <section>
                <h3 className="text-sm font-medium text-zinc-300 mb-3">{t("chd.settingsTitle")}</h3>
                <input
                  type="text"
                  value={form.chdman_path || ""}
                  onChange={(e) => setForm({ ...form, chdman_path: e.target.value || undefined })}
                  placeholder="chdman"
                  className="w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                />
                <p className="text-xs text-zinc-500 mt-2">{t("chd.settingsHelp")}</p>
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
              onMigrate={handleMigrate}
              onActivate={handleActivate}
            />
          )}

          {activeTab === "media" && (
            <MediaConfigTab
              mediaPaths={mediaPaths}
              onChange={(paths) => setForm({ ...form, media_paths: paths })}
            />
          )}

          {activeTab === "rules" && (
            <RenameRulesTab
              rules={form.rename_rules}
              onChange={(rules) => setForm({ ...form, rename_rules: rules })}
            />
          )}


          {activeTab === "about" && (
            <AboutTab
              checkUpdates={form.check_updates !== false}
              onCheckUpdatesChange={(on) => setForm({ ...form, check_updates: on ? undefined : false })}
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
    {/* Fuera del fondo de Ajustes: si no, un clic en la migración también cerraría los Ajustes */}
    <MigrationModal open={!!migrating} sourceProfile={migrating} onClose={() => setMigrating(null)} />
    </>
  );
}
