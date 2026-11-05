import { useEffect, useRef, useState } from "react";
import { useAppStore } from "./store/useAppStore";
import { MainLayout } from "./components/layout/MainLayout";
import { GameListView } from "./components/games/GameListView";
import { GameDetailPanel } from "./components/detail/GameDetailPanel";
import { GameInfoPanel } from "./components/detail/GameInfoPanel";
import { BulkEditPanel } from "./components/detail/BulkEditPanel";
import { SettingsView } from "./components/settings/SettingsView";
import { ScrapeProgressModal } from "./components/common/ScrapeProgressModal";
import { UnusedMediaModal } from "./components/common/UnusedMediaModal";
import { RenameModal } from "./components/common/RenameModal";
import { BackupsModal } from "./components/common/BackupsModal";
import { ExportModal } from "./components/common/ExportModal";
import { BestVersionModal } from "./components/common/BestVersionModal";
import { DuplicatesModal } from "./components/common/DuplicatesModal";
import { CollectionsModal } from "./components/common/CollectionsModal";
import { NormalizeMediaModal } from "./components/common/NormalizeMediaModal";
import { NewRomsModal } from "./components/common/NewRomsModal";
import { ToolsMenu } from "./components/common/ToolsMenu";
import { MediaReviewModal } from "./components/common/MediaReviewModal";
import { MediaSwapModal } from "./components/common/MediaSwapModal";
import { MigrationModal } from "./components/common/MigrationModal";
import { StructurePromptModal } from "./components/common/StructurePromptModal";
import { datReport, listMissingRoms, listNewRoms, scrapeSystem } from "./commands";
import { DatModal } from "./components/common/DatModal";
import { MissingRomsModal } from "./components/common/MissingRomsModal";
import { useFilteredGames } from "./hooks/useFilteredGames";
import { isArcadeSystem } from "./utils/arcade";
import { ScrapeAllModal } from "./components/common/ScrapeAllModal";
import type { ScrapePolicy } from "./types";
import { bumpMediaVersion } from "./utils/images";
import { useTranslation } from "./i18n";
import { listen } from "@tauri-apps/api/event";
import { getActiveProfile, isProfileComplete } from "./utils/profiles";
import { donationPromptDue } from "./utils/links";
import { DonationModal } from "./components/common/DonationModal";
import { UpdateBanner } from "./components/common/UpdateBanner";
import { UpdateModal } from "./components/common/UpdateModal";
import { useUpdateStore } from "./store/useUpdateStore";
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
  // Scrape All finished and the games were reloaded: only then can the ones not found be searched by hand
  const [scrapeFinished, setScrapeFinished] = useState(false);
  // How the last run ended: stopped by the user, or the error that cut it
  const [scrapeStopped, setScrapeStopped] = useState(false);
  const [scrapeError, setScrapeError] = useState<string | null>(null);
  const [showCleanupModal, setShowCleanupModal] = useState(false);
  const [showRenameModal, setShowRenameModal] = useState(false);
  const [showBackupsModal, setShowBackupsModal] = useState(false);
  const [showExportModal, setShowExportModal] = useState(false);
  const [showDuplicatesModal, setShowDuplicatesModal] = useState(false);
  const [showBestVersion, setShowBestVersion] = useState(false);
  const [showCollectionsModal, setShowCollectionsModal] = useState(false);
  const [showNormalizeModal, setShowNormalizeModal] = useState(false);
  const [showNewRomsModal, setShowNewRomsModal] = useState(false);
  const [showMediaReview, setShowMediaReview] = useState(false);
  const [showMediaSwap, setShowMediaSwap] = useState(false);
  // ROMs of the selected system that aren't in its gamelist (badge on the "New ROMs" button)
  const [newRomsCount, setNewRomsCount] = useState(0);
  const [showMissingRoms, setShowMissingRoms] = useState(false);
  const [showScrapeAll, setShowScrapeAll] = useState(false);
  const [showDat, setShowDat] = useState(false);
  const setDatMarks = useAppStore((s) => s.setDatMarks);
  // Bumped when the DAT changes, so the marks are reloaded
  const [datVersion, setDatVersion] = useState(0);
  const multiSelect = useAppStore((s) => s.multiSelect);
  const missingRomPaths = useAppStore((s) => s.missingRomPaths);
  const setMissingRomPaths = useAppStore((s) => s.setMissingRomPaths);
  const [showSettings, setShowSettings] = useState(false);
  const games = useAppStore((s) => s.games);
  // What the list shows now (search, folder, filters): Scrape All can do only these
  const visibleGames = useFilteredGames();
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
      setMissingRomPaths(new Set());
      return;
    }
    let cancelled = false;
    listNewRoms(selectedSystem.path)
      .then((roms) => !cancelled && setNewRomsCount(roms.length))
      .catch(() => !cancelled && setNewRomsCount(0));
    // And the other way round: games whose ROM is gone (marked in the list, "No ROM (N)" button)
    listMissingRoms(selectedSystem.path)
      .then((m) => !cancelled && setMissingRomPaths(new Set(m.games.map((g) => g.path))))
      .catch(() => !cancelled && setMissingRomPaths(new Set()));
    return () => {
      cancelled = true;
    };
  }, [selectedSystem?.path, games]);

  // Clone/BIOS marks from the system's DAT (null = no DAT): on system change, DAT change or games change
  useEffect(() => {
    if (!selectedSystem) {
      setDatMarks(null);
      return;
    }
    let cancelled = false;
    datReport(selectedSystem.path)
      .then((r) => !cancelled && setDatMarks(r ? new Map(r.marks.map((m) => [m.game_path, m])) : null))
      .catch(() => !cancelled && setDatMarks(null));
    return () => {
      cancelled = true;
    };
  }, [selectedSystem?.path, datVersion, games.length]);

  // After media files moved between types: drop cached images, reload and refresh the selection
  async function reloadAfterMediaChange() {
    if (!selectedSystem) return;
    bumpMediaVersion();
    await loadGames(selectedSystem.path);
    const { games: fresh, selectedGame: current } = useAppStore.getState();
    if (current) selectGame(fresh.find((g) => g.path === current.path) ?? null);
  }

  // resume = continue the run left halfway in this system (its own games and rules)
  async function handleScrapeAll(gamePaths?: string[], policy?: ScrapePolicy, resume?: boolean) {
    if (!selectedSystem) return;
    setScrapeFinished(false);
    setScrapeStopped(false);
    setScrapeError(null);
    setShowScrapeModal(true);
    try {
      const result = await scrapeSystem(selectedSystem.path, gamePaths, policy, resume);
      setScrapeStopped(result.stopped);
    } catch (e) {
      console.error("Batch scrape failed:", e);
      setScrapeError(String(e));
    } finally {
      // Reload games after scraping ends (also when stopped or cut: what was done is saved)
      await loadGames(selectedSystem.path).catch(() => {});
      setScrapeFinished(true);
    }
  }

  // Developer credentials may be built into the app, so ask the backend instead of the settings
  const scraperStatus = useAppStore((s) => s.scraperStatus);
  // Some source can scrape this system: an account/key, the LaunchBox database, or ArcadeItalia (arcade, no account)
  const hasCredentials =
    scraperStatus.screenscraper ||
    scraperStatus.tgdb ||
    scraperStatus.launchbox ||
    (!!selectedSystem && isArcadeSystem(selectedSystem.name));

  // Auto-show settings while there is no usable profile (no ROM folder, or ES-DE without its folder).
  // Only once the saved settings are read: before that they are empty and would always look unusable.
  const activeProfile = getActiveProfile(settings);
  const hasUsableProfile = isProfileComplete(activeProfile);
  useEffect(() => {
    if (settingsLoaded && !hasUsableProfile) {
      setShowSettings(true);
    }
  }, [settingsLoaded, hasUsableProfile]);

  // Donation prompt: checked once per session, a moment after start, and only with a usable
  // profile and nothing else open (no settings window over the first profile, no editor...).
  // If something is open then, it waits for the next launch. Nothing is saved while another
  // window is open either: the settings window would overwrite it with its own copy.
  const [showDonation, setShowDonation] = useState(false);
  const donationChecked = useRef(false);
  useEffect(() => {
    if (!settingsLoaded || !hasUsableProfile || donationChecked.current) return;
    // The "checked" mark goes inside the timer: in development React runs effects twice
    // (the first timer is cancelled), and marking it here would skip the check for good
    const timer = setTimeout(() => {
      donationChecked.current = true;
      if (document.querySelector(".fixed.inset-0")) return;
      const { settings: current, patchSettings } = useAppStore.getState();
      const due = donationPromptDue(current.donation_prompt_at, Math.floor(Date.now() / 1000));
      if (due.saveAt) patchSettings({ donation_prompt_at: due.saveAt });
      if (due.show) setShowDonation(true);
    }, 2000);
    return () => clearTimeout(timer);
  }, [settingsLoaded, hasUsableProfile]);

  // New versions: look at start and then once a day while the app stays open, unless turned
  // off in Settings > About. Automatic checks fail silently (offline, dev build...)
  const checkUpdates = settings.check_updates !== false;
  useEffect(() => {
    if (!settingsLoaded || !checkUpdates) return;
    const run = () => useUpdateStore.getState().checkNow();
    const first = setTimeout(run, 3000);
    const daily = setInterval(run, 24 * 3600 * 1000);
    return () => {
      clearTimeout(first);
      clearInterval(daily);
    };
  }, [settingsLoaded, checkUpdates]);

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
          <UpdateBanner />
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
              {missingRomPaths.size > 0 && (
                <button
                  onClick={() => setShowMissingRoms(true)}
                  className="px-3 py-1.5 text-sm bg-red-600/15 text-red-300 border border-red-800/60 rounded hover:bg-red-600/25 transition-colors whitespace-nowrap"
                  title={t("missingRoms.description")}
                >
                  {t("missingRoms.button", { count: missingRomPaths.size })}
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
                  { label: t("best.button"), onClick: () => setShowBestVersion(true) },
                  { label: t("dat.button"), onClick: () => setShowDat(true), separatorBefore: true },
                  { label: t("collections.button"), onClick: () => setShowCollectionsModal(true) },
                  { label: t("backup.button"), onClick: () => setShowBackupsModal(true) },
                  { label: t("export.button"), onClick: () => setShowExportModal(true) },
                ]}
              />
              {hasCredentials && (
                <button
                  // First choose what to update this time
                  onClick={() => setShowScrapeAll(true)}
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
        {/* Multi-select mode: the right panel edits every selected game at once */}
        {selectedSystem && (multiSelect ? <BulkEditPanel /> : <GameInfoPanel />)}

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

      <DatModal
        open={showDat}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowDat(false)}
        onChanged={() => {
          setDatVersion((v) => v + 1);
          if (selectedSystem) {
            // New ROMs / no-ROM counters and the list (files may have gone to the trash)
            listNewRoms(selectedSystem.path).then((r) => setNewRomsCount(r.length)).catch(() => {});
          }
        }}
      />

      <ScrapeAllModal
        open={showScrapeAll}
        gameCount={games.length}
        visibleCount={visibleGames.length}
        systemPath={selectedSystem?.path ?? ""}
        onClose={() => setShowScrapeAll(false)}
        onStart={(policy, onlyVisible) => {
          setShowScrapeAll(false);
          handleScrapeAll(onlyVisible ? visibleGames.map((g) => g.path) : undefined, policy);
        }}
        onContinue={() => {
          setShowScrapeAll(false);
          handleScrapeAll(undefined, undefined, true);
        }}
      />

      <MissingRomsModal
        open={showMissingRoms}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowMissingRoms(false)}
        onDone={() => {
          if (!selectedSystem) return;
          selectGame(null);
          loadGames(selectedSystem.path);
        }}
      />

      <MediaReviewModal
        open={showMediaReview}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowMediaReview(false)}
        onDone={reloadAfterMediaChange}
        onOpenSwap={() => setShowMediaSwap(true)}
      />

      <MediaSwapModal
        open={showMediaSwap}
        systemPath={selectedSystem?.path || ""}
        games={games}
        onClose={() => setShowMediaSwap(false)}
        onDone={reloadAfterMediaChange}
      />

      <NewRomsModal
        onTrashed={() => {
          // Deleting files doesn't change the games, so recount by hand
          if (selectedSystem) listNewRoms(selectedSystem.path).then((r) => setNewRomsCount(r.length)).catch(() => {});
        }}
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
        finished={scrapeFinished}
        stopped={scrapeStopped}
        error={scrapeError}
        systemPath={selectedSystem?.path ?? ""}
        onClose={() => setShowScrapeModal(false)}
      />

      <UnusedMediaModal
        open={showCleanupModal}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowCleanupModal(false)}
        onDone={reloadAfterMediaChange}
        // Damaged media sent to the trash: offer to download it again (only with a scraper configured)
        onRescrape={hasCredentials ? (paths, policy) => handleScrapeAll(paths, policy) : undefined}
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

      <BestVersionModal
        open={showBestVersion}
        systemPath={selectedSystem?.path || ""}
        onClose={() => setShowBestVersion(false)}
        onDone={() => {
          if (selectedSystem) loadGames(selectedSystem.path);
          useAppStore.getState().loadSystems();
        }}
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

      <DonationModal open={showDonation} onClose={() => setShowDonation(false)} />

      <UpdateModal />
    </MainLayout>
  );
}

export default App;
