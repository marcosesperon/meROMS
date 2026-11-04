import { useEffect, useState } from "react";
import { useAppStore } from "./store/useAppStore";
import { MainLayout } from "./components/layout/MainLayout";
import { GameListView } from "./components/games/GameListView";
import { GameDetailPanel } from "./components/detail/GameDetailPanel";
import { SettingsView } from "./components/settings/SettingsView";
import { ScrapeProgressModal } from "./components/common/ScrapeProgressModal";
import { UnusedMediaModal } from "./components/common/UnusedMediaModal";
import { RenameModal } from "./components/common/RenameModal";
import { BackupsModal } from "./components/common/BackupsModal";
import { ExportModal } from "./components/common/ExportModal";
import { DuplicatesModal } from "./components/common/DuplicatesModal";
import { CollectionsModal } from "./components/common/CollectionsModal";
import { NormalizeMediaModal } from "./components/common/NormalizeMediaModal";
import { MigrationModal } from "./components/common/MigrationModal";
import { StructurePromptModal } from "./components/common/StructurePromptModal";
import { scrapeSystem } from "./commands";
import { useTranslation } from "./i18n";
import { listen } from "@tauri-apps/api/event";
import { getActiveProfile, isProfileComplete } from "./utils/profiles";
import "./App.css";

function App() {
  const currentView = useAppStore((s) => s.currentView);
  const selectedGame = useAppStore((s) => s.selectedGame);
  const selectGame = useAppStore((s) => s.selectGame);
  const settings = useAppStore((s) => s.settings);
  const loadSettings = useAppStore((s) => s.loadSettings);
  const settingsLoaded = useAppStore((s) => s.settingsLoaded);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const loadGames = useAppStore((s) => s.loadGames);
  const setScanProgress = useAppStore((s) => s.setScanProgress);
  const pendingStructure = useAppStore((s) => s.pendingStructure);
  const migrationPreset = useAppStore((s) => s.migrationPreset);
  const openPresetMigration = useAppStore((s) => s.openPresetMigration);
  const closePresetMigration = useAppStore((s) => s.closePresetMigration);

  const { t } = useTranslation();
  const [showNewGame, setShowNewGame] = useState(false);
  const [showScrapeModal, setShowScrapeModal] = useState(false);
  const [showCleanupModal, setShowCleanupModal] = useState(false);
  const [showRenameModal, setShowRenameModal] = useState(false);
  const [showBackupsModal, setShowBackupsModal] = useState(false);
  const [showExportModal, setShowExportModal] = useState(false);
  const [showDuplicatesModal, setShowDuplicatesModal] = useState(false);
  const [showCollectionsModal, setShowCollectionsModal] = useState(false);
  const [showNormalizeModal, setShowNormalizeModal] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const games = useAppStore((s) => s.games);
  const showDetailPanel = selectedGame !== null || showNewGame;

  useEffect(() => {
    loadSettings();
    const unlisten = listen<{ current: number; total: number; current_name: string }>(
      "scan-progress",
      (event) => {
        setScanProgress({
          current: event.payload.current,
          total: event.payload.total,
          currentName: event.payload.current_name,
        });
      }
    );
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  async function handleScrapeAll() {
    if (!selectedSystem) return;
    setShowScrapeModal(true);
    try {
      await scrapeSystem(selectedSystem.path);
      // Reload games after scraping completes
      loadGames(selectedSystem.path);
    } catch (e) {
      console.error("Batch scrape failed:", e);
    }
  }

  const hasCredentials =
    !!(settings.ss_devid && settings.ss_devpassword) || !!settings.tgdb_apikey;

  // Auto-show settings while there is no usable profile (no ROM folder, or ES-DE without its folder).
  // Only once the saved settings are read: before that they are empty and would always look unusable.
  const activeProfile = getActiveProfile(settings);
  const hasUsableProfile = isProfileComplete(activeProfile);
  useEffect(() => {
    if (settingsLoaded && !hasUsableProfile) {
      setShowSettings(true);
    }
  }, [settingsLoaded, hasUsableProfile]);

  // Open settings modal when currentView changes to "settings"
  useEffect(() => {
    if (currentView === "settings") {
      setShowSettings(true);
      // Reset view back to browser so clicking settings again works
      useAppStore.getState().setCurrentView("browser");
    }
  }, [currentView]);

  return (
    <MainLayout>
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* This system's data is in the other structure: offer to migrate just this one */}
          {selectedSystem && pendingStructure?.systems.includes(selectedSystem.name) && (
            <div className="px-6 py-2 shrink-0 border-b border-amber-700/40 bg-amber-600/10 flex items-center justify-between gap-3">
              <p className="text-sm text-amber-300">
                {t("structure.banner", {
                  kind: pendingStructure.source.kind === "esde" ? "ES-DE" : t("profiles.kindClassic"),
                })}
              </p>
              <button
                onClick={() => openPresetMigration([selectedSystem.name])}
                className="shrink-0 px-3 py-1 text-sm bg-amber-600 text-white rounded hover:bg-amber-500 transition-colors"
              >
                {t("structure.migrate")}
              </button>
            </div>
          )}
          <GameListView />
          {selectedSystem && (
            <div className="px-6 h-12 shrink-0 border-t border-zinc-700 flex items-center justify-end gap-2">
              <button
                onClick={() => setShowExportModal(true)}
                className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
              >
                {t("export.button")}
              </button>
              <button
                onClick={() => setShowDuplicatesModal(true)}
                className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
              >
                {t("duplicates.button")}
              </button>
              <button
                onClick={() => setShowCollectionsModal(true)}
                className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
              >
                {t("collections.button")}
              </button>
              <button
                onClick={() => setShowBackupsModal(true)}
                className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
              >
                {t("backup.button")}
              </button>
              <button
                onClick={() => setShowRenameModal(true)}
                className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
              >
                {t("rename.button")}
              </button>
              <button
                onClick={() => setShowNormalizeModal(true)}
                className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
                title={t(activeProfile?.kind === "esde" ? "normalize.descriptionEsde" : "normalize.description")}
              >
                {t("normalize.button")}
              </button>
              <button
                onClick={() => setShowCleanupModal(true)}
                className="px-3 py-1.5 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
              >
                {t("cleanup.button")}
              </button>
              {hasCredentials && (
                <button
                  onClick={handleScrapeAll}
                  className="px-3 py-1.5 text-sm bg-emerald-600 text-white rounded hover:bg-emerald-500 transition-colors"
                >
                  {t("app.scrapeAll")}
                </button>
              )}
              <button
                onClick={() => {
                  selectGame(null);
                  setShowNewGame(true);
                }}
                className="px-3 py-1.5 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
              >
                {t("app.addGame")}
              </button>
            </div>
          )}
        </div>

      {showDetailPanel && (
        <GameDetailPanel
          isNewGame={showNewGame}
          onClose={() => {
            selectGame(null);
            setShowNewGame(false);
          }}
        />
      )}

      <ScrapeProgressModal
        open={showScrapeModal}
        onClose={() => setShowScrapeModal(false)}
      />

      <UnusedMediaModal
        open={showCleanupModal}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowCleanupModal(false)}
      />

      <RenameModal
        open={showRenameModal}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowRenameModal(false)}
      />

      <BackupsModal
        open={showBackupsModal}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowBackupsModal(false)}
      />

      <ExportModal
        open={showExportModal}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowExportModal(false)}
      />

      <DuplicatesModal
        open={showDuplicatesModal}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowDuplicatesModal(false)}
        onGamesChanged={() => {
          if (selectedSystem) loadGames(selectedSystem.path);
        }}
      />

      <CollectionsModal
        open={showCollectionsModal}
        systemPath={selectedSystem?.path || ""}
        games={games}
        onClose={() => setShowCollectionsModal(false)}
      />

      <NormalizeMediaModal
        open={showNormalizeModal}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowNormalizeModal(false)}
      />

      <StructurePromptModal />

      <MigrationModal
        open={!!migrationPreset}
        preset={migrationPreset}
        onClose={closePresetMigration}
      />

      <SettingsView
        open={showSettings}
        onClose={() => setShowSettings(false)}
      />
    </MainLayout>
  );
}

export default App;
