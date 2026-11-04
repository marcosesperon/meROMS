import { useEffect, useState } from "react";
import { useAppStore } from "./store/useAppStore";
import { MainLayout } from "./components/layout/MainLayout";
import { GameListView } from "./components/games/GameListView";
import { GameDetailPanel } from "./components/detail/GameDetailPanel";
import { GameInfoPanel } from "./components/detail/GameInfoPanel";
import { SettingsView } from "./components/settings/SettingsView";
import { ScrapeProgressModal } from "./components/common/ScrapeProgressModal";
import { UnusedMediaModal } from "./components/common/UnusedMediaModal";
import { RenameModal } from "./components/common/RenameModal";
import { BackupsModal } from "./components/common/BackupsModal";
import { ExportModal } from "./components/common/ExportModal";
import { DuplicatesModal } from "./components/common/DuplicatesModal";
import { CollectionsModal } from "./components/common/CollectionsModal";
import { NormalizeMediaModal } from "./components/common/NormalizeMediaModal";
import { NewRomsModal } from "./components/common/NewRomsModal";
import { ToolsMenu } from "./components/common/ToolsMenu";
import { MediaReviewModal } from "./components/common/MediaReviewModal";
import { MediaSwapModal } from "./components/common/MediaSwapModal";
import { MigrationModal } from "./components/common/MigrationModal";
import { StructurePromptModal } from "./components/common/StructurePromptModal";
import { listNewRoms, scrapeSystem } from "./commands";
import { bumpMediaVersion } from "./utils/images";
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
  const [showNewRomsModal, setShowNewRomsModal] = useState(false);
  const [showMediaReview, setShowMediaReview] = useState(false);
  const [showMediaSwap, setShowMediaSwap] = useState(false);
  // ROMs of the selected system that aren't in its gamelist (badge on the "New ROMs" button)
  const [newRomsCount, setNewRomsCount] = useState(0);
  const [showSettings, setShowSettings] = useState(false);
  const games = useAppStore((s) => s.games);
  const editorOpen = useAppStore((s) => s.editorOpen);
  const closeEditor = useAppStore((s) => s.closeEditor);
  // Selecting a game only shows it in the info panel; the editor opens on "Edit" / double click
  const showDetailPanel = (editorOpen && selectedGame !== null) || showNewGame;

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

  // Recount whenever the games change (system loaded, ROMs added, renamed, removed...)
  useEffect(() => {
    if (!selectedSystem) {
      setNewRomsCount(0);
      return;
    }
    let cancelled = false;
    listNewRoms(selectedSystem.path)
      .then((roms) => !cancelled && setNewRomsCount(roms.length))
      .catch(() => !cancelled && setNewRomsCount(0));
    return () => {
      cancelled = true;
    };
  }, [selectedSystem?.path, games]);

  // After media files moved between types: drop cached images, reload and refresh the selection
  async function reloadAfterMediaChange() {
    if (!selectedSystem) return;
    bumpMediaVersion();
    await loadGames(selectedSystem.path);
    const { games: fresh, selectedGame: current } = useAppStore.getState();
    if (current) selectGame(fresh.find((g) => g.path === current.path) ?? null);
  }

  async function handleScrapeAll(gamePaths?: string[]) {
    if (!selectedSystem) return;
    setShowScrapeModal(true);
    try {
      await scrapeSystem(selectedSystem.path, gamePaths);
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
            <div className="px-6 h-12 shrink-0 border-t border-zinc-800 bg-zinc-900 flex items-center gap-2">
              {newRomsCount > 0 && (
                <button
                  onClick={() => setShowNewRomsModal(true)}
                  className="px-3 py-1.5 text-sm bg-amber-600/20 text-amber-300 border border-amber-700/60 rounded hover:bg-amber-600/30 transition-colors whitespace-nowrap"
                  title={t("newRoms.description")}
                >
                  {t("newRoms.button", { count: newRomsCount })}
                </button>
              )}
              <div className="flex-1" />
              {/* Secondary tools live in a menu so the bar never wraps */}
              <ToolsMenu
                label={t("toolbar.tools")}
                items={[
                  { label: t("rename.button"), onClick: () => setShowRenameModal(true) },
                  {
                    label: t("normalize.button"),
                    onClick: () => setShowNormalizeModal(true),
                    title: t(activeProfile?.kind === "esde" ? "normalize.descriptionEsde" : "normalize.description"),
                  },
                  { label: t("cleanup.button"), onClick: () => setShowCleanupModal(true) },
                  { label: t("mediaKinds.reviewButton"), onClick: () => setShowMediaReview(true) },
                  { label: t("mediaKinds.swapButton"), onClick: () => setShowMediaSwap(true) },
                  { label: t("duplicates.button"), onClick: () => setShowDuplicatesModal(true) },
                  { label: t("collections.button"), onClick: () => setShowCollectionsModal(true), separatorBefore: true },
                  { label: t("backup.button"), onClick: () => setShowBackupsModal(true) },
                  { label: t("export.button"), onClick: () => setShowExportModal(true) },
                ]}
              />
              {hasCredentials && (
                <button
                  onClick={() => handleScrapeAll()}
                  className="px-3 py-1.5 text-sm bg-emerald-600 text-white rounded hover:bg-emerald-500 transition-colors whitespace-nowrap"
                >
                  {t("app.scrapeAll")}
                </button>
              )}
              <button
                onClick={() => {
                  selectGame(null);
                  setShowNewGame(true);
                }}
                className="px-3 py-1.5 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors whitespace-nowrap"
              >
                {t("app.addGame")}
              </button>
            </div>
          )}
        </div>
        {selectedSystem && <GameInfoPanel />}

      {showDetailPanel && (
        <GameDetailPanel
          isNewGame={showNewGame}
          onClose={() => {
            // Back to the info panel with the game still selected
            closeEditor();
            setShowNewGame(false);
          }}
        />
      )}

      <MediaReviewModal
        open={showMediaReview}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowMediaReview(false)}
        onDone={reloadAfterMediaChange}
      />

      <MediaSwapModal
        open={showMediaSwap}
        systemPath={selectedSystem?.path || ""}
        games={games}
        onClose={() => setShowMediaSwap(false)}
        onDone={reloadAfterMediaChange}
      />

      <NewRomsModal
        open={showNewRomsModal}
        systemPath={selectedSystem?.path || ""}
        canScrape={hasCredentials}
        onClose={() => setShowNewRomsModal(false)}
        onAdded={(paths, scrape) => {
          if (!selectedSystem) return;
          // Scraping reloads the games when it ends; otherwise reload now
          if (scrape) handleScrapeAll(paths);
          else loadGames(selectedSystem.path);
        }}
      />

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
