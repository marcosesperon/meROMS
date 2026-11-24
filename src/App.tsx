import { useEffect, useMemo, useRef, useState } from "react";
import { useAppStore, type ToolId } from "./store/useAppStore";
import { MainLayout } from "./components/layout/MainLayout";
import { GameListView } from "./components/games/GameListView";
import { GameDetailPanel } from "./components/detail/GameDetailPanel";
import { GameFullView } from "./components/detail/GameFullView";
import { GameInfoPanel } from "./components/detail/GameInfoPanel";
import { BulkEditPanel } from "./components/detail/BulkEditPanel";
import { SettingsView } from "./components/settings/SettingsView";
import { ScrapeProgressModal } from "./components/common/ScrapeProgressModal";
import { RenameModal } from "./components/common/RenameModal";
import { BackupsModal } from "./components/common/BackupsModal";
import { ExportModal } from "./components/common/ExportModal";
import { runUndo } from "./utils/undo";
import { ExportDeviceModal } from "./components/common/ExportDeviceModal";
import { CompareModal } from "./components/common/CompareModal";
import { ChdModal } from "./components/common/ChdModal";
import { BiosModal } from "./components/common/BiosModal";
import { CollectionsModal } from "./components/common/CollectionsModal";
import { NewRomsModal } from "./components/common/NewRomsModal";
import { ToolsMenu } from "./components/common/ToolsMenu";
import { MediaSwapModal } from "./components/common/MediaSwapModal";
import { MediaModal } from "./components/common/MediaModal";
import { VersionsModal } from "./components/common/VersionsModal";
import { OrderModal } from "./components/common/OrderModal";
import { MissingModal } from "./components/common/MissingModal";
import { ClearFieldsModal } from "./components/common/ClearFieldsModal";
import { MigrationModal } from "./components/common/MigrationModal";
import { StructurePromptModal } from "./components/common/StructurePromptModal";
import { datReport, listMissingRoms, listNewRoms, profilesAvailable, scrapeSystem, watchRoms } from "./commands";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { DatModal } from "./components/common/DatModal";
import { MissingRomsModal } from "./components/common/MissingRomsModal";
import { useFilteredGames } from "./hooks/useFilteredGames";
import { isArcadeSystem } from "./utils/arcade";
import { ScrapeAllModal } from "./components/common/ScrapeAllModal";
import type { ScrapePolicy } from "./types";
import { bumpMediaVersion } from "./utils/images";
import { autoRename } from "./utils/autoRename";
import { useTranslation } from "./i18n";
import { getActiveProfile, isProfileComplete, profileLocationKey } from "./utils/profiles";
import { donationPromptDue } from "./utils/links";
import { CouchMode } from "./components/couch/CouchMode";
import { useGamepad } from "./hooks/useGamepad";
import { listen } from "@tauri-apps/api/event";
import type { GameExited } from "./commands";
import { PlayingBanner } from "./components/play/PlayingBanner";
import { LaunchProblemModal } from "./components/play/LaunchProblemModal";
import { SystemViewModal } from "./components/common/SystemViewModal";
import { RandomGameModal } from "./components/random/RandomGameModal";
import { RaCheckModal } from "./components/common/RaCheckModal";
import { shotsRunning, startShots } from "./dev/shots";
import { GlobalSearch } from "./components/common/GlobalSearch";
import { ResultDialog } from "./components/common/ResultDialog";
import { ChoiceDialog } from "./components/common/ChoiceDialog";
import { TransferProfileModal } from "./components/common/TransferProfileModal";
import { DonationModal } from "./components/common/DonationModal";
import { UpdateBanner } from "./components/common/UpdateBanner";
import { UpdateModal } from "./components/common/UpdateModal";
import { useUpdateStore } from "./store/useUpdateStore";
import "./App.css";

/** How often the active profile's folder is checked (SD card plugged in / out) */
const PROFILE_CHECK_MS = 3000;

function App() {
  const currentView = useAppStore((s) => s.currentView);
  const selectedGame = useAppStore((s) => s.selectedGame);
  const selectGame = useAppStore((s) => s.selectGame);
  const settings = useAppStore((s) => s.settings);
  const loadSettings = useAppStore((s) => s.loadSettings);
  const settingsLoaded = useAppStore((s) => s.settingsLoaded);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const loadGames = useAppStore((s) => s.loadGames);
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
  // System tools: one open at a time, kept in the store (the system status, "Put it all in
  // order" and other windows open them too)
  const tool = useAppStore((s) => s.tool);
  const openTool = useAppStore((s) => s.openTool);
  const closeTool = useAppStore((s) => s.closeTool);
  const isTool = (id: ToolId) => tool?.id === id;
  // ROMs of the selected system that aren't in its gamelist (status line and toolbar notice)
  const newRomsCount = useAppStore((s) => s.newRomsCount);
  const setNewRomsCount = useAppStore((s) => s.setNewRomsCount);
  const setDatMarks = useAppStore((s) => s.setDatMarks);
  // Bumped when the DAT changes, so the marks are reloaded
  const [datVersion, setDatVersion] = useState(0);
  const multiSelect = useAppStore((s) => s.multiSelect);
  const viewMode = useAppStore((s) => s.viewMode);
  // Couch mode: the list and the panels aren't drawn underneath (they'd answer the same keys
  // and play a second video); the gamepad is read from here (Start enters it from anywhere, the
  // start screen too: then it begins on the systems screen, with "Continue playing" first)
  const couchMode = useAppStore((s) => s.couchMode);
  useGamepad();
  // Play: the emulator closed (new play stats); the system's view window lives here too, so
  // «Configure emulator» can open it from the couch mode
  useEffect(() => {
    const unlisten = listen<GameExited>("game-exited", (e) => useAppStore.getState().gameExited(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);
  // Watching the ROM folders (Settings, on unless turned off): new and missing ROMs show up in
  // the sidebar and in the selected system's buttons without entering it. Real-time watching in
  // Rust ("roms-changed") plus a full pass when the list of systems changes, and a pass of the
  // open system when the window comes back to the front (network or USB drives may not send
  // changes), at most every 15 s. On a network drive a pass of the whole library is slow, so
  // it isn't repeated when only a game count changes (entering a system updates it).
  const watchOn = useAppStore((s) => s.settings.watch_roms !== false);
  const profileKey = useAppStore((s) => profileLocationKey(getActiveProfile(s.settings)));
  const systemsKey = useAppStore((s) => s.systems.filter((x) => x.has_gamelist).map((x) => x.path).join("\n"));
  // Bumped when the selected system's folder changed: its buttons are recounted
  const [romsTick, setRomsTick] = useState(0);
  const profileUnavailable = useAppStore((s) => s.profileUnavailable);
  useEffect(() => {
    // Without its folder (SD card not plugged in) there is nothing to watch
    watchRoms(watchOn && !profileUnavailable).catch(() => {});
  }, [watchOn, profileKey, profileUnavailable]);

  // A profile on a disk that comes and goes (an SD card): every few seconds check that its folder
  // exists. When it appears the systems are loaded; when it goes, back to the start screen.
  useEffect(() => {
    let busy = false;
    const timer = setInterval(async () => {
      const state = useAppStore.getState();
      const profile = getActiveProfile(state.settings);
      if (busy || !profile || !isProfileComplete(profile)) return;
      busy = true;
      try {
        const available = (await profilesAvailable([profile]).catch(() => ({}) as Record<string, boolean>))[profile.id] !== false;
        // The profile may have changed in the meantime
        if (getActiveProfile(useAppStore.getState().settings)?.id !== profile.id) return;
        if (available && useAppStore.getState().profileUnavailable) await useAppStore.getState().loadSystems();
        else if (!available && !useAppStore.getState().profileUnavailable) useAppStore.getState().setProfileUnavailable(true);
      } finally {
        busy = false;
      }
    }, PROFILE_CHECK_MS);
    return () => clearInterval(timer);
  }, []);
  useEffect(() => {
    if (watchOn) useAppStore.getState().refreshRomChanges();
    else useAppStore.setState({ romChanges: {} });
  }, [watchOn, systemsKey]);
  useEffect(() => {
    if (!watchOn) return;
    let last = Date.now();
    const unlistenFocus = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (!focused || Date.now() - last < 15000) return;
      const open = useAppStore.getState().selectedSystem?.path;
      if (!open) return;
      last = Date.now();
      useAppStore.getState().refreshRomChanges([open]);
      setRomsTick((n) => n + 1);
    });
    const unlistenChanges = listen<string[]>("roms-changed", (e) => {
      useAppStore.getState().refreshRomChanges(e.payload);
      if (e.payload.includes(useAppStore.getState().selectedSystem?.path ?? "")) setRomsTick((n) => n + 1);
    });
    return () => {
      unlistenFocus.then((f) => f());
      unlistenChanges.then((f) => f());
    };
  }, [watchOn]);
  const systemViewOpen = useAppStore((s) => s.systemViewOpen);
  const setSystemViewOpen = useAppStore((s) => s.setSystemViewOpen);
  const clickAction = useAppStore((s) => s.settings.click_action);
  const carouselPanelOpen = useAppStore((s) => s.carouselPanelOpen);
  const setCarouselPanelOpen = useAppStore((s) => s.setCarouselPanelOpen);
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
  }, []);

  // Recount when the games' paths change (system loaded, ROMs added, renamed, removed...), not
  // on every data edit: each count walks the system folder, slow on a network drive
  const gamePathsKey = useMemo(() => games.map((g) => g.path).join("\n"), [games]);
  useEffect(() => {
    if (!selectedSystem) {
      setNewRomsCount(0);
      setMissingRomPaths(new Set());
      return;
    }
    let cancelled = false;
    const path = selectedSystem.path;
    const { trackBackground } = useAppStore.getState();
    const newRoms = trackBackground(listNewRoms(path)).then((roms) => roms.length);
    // And the other way round: games whose ROM is gone (marked in the list, "No ROM (N)" button)
    const missing = trackBackground(listMissingRoms(path)).then((m) => new Set(m.games.map((g) => g.path)));
    newRoms.then((n) => !cancelled && setNewRomsCount(n)).catch(() => !cancelled && setNewRomsCount(0));
    missing.then((m) => !cancelled && setMissingRomPaths(m)).catch(() => !cancelled && setMissingRomPaths(new Set()));
    // The sidebar mark of this system follows the same counts (adding ROMs clears it)
    Promise.all([newRoms, missing])
      .then(([n, m]) => {
        if (useAppStore.getState().settings.watch_roms !== false) useAppStore.getState().setRomChange(path, { new_roms: n, missing: m.size });
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [selectedSystem?.path, gamePathsKey, romsTick]);

  // Clone/BIOS marks from the system's DAT (null = no DAT): on system change, DAT change or games change
  useEffect(() => {
    if (!selectedSystem) {
      setDatMarks(null);
      return;
    }
    let cancelled = false;
    useAppStore
      .getState()
      .trackBackground(datReport(selectedSystem.path))
      .then((r) => !cancelled && setDatMarks(r ? new Map(r.marks.map((m) => [m.game_path, m])) : null))
      .catch(() => !cancelled && setDatMarks(null));
    return () => {
      cancelled = true;
    };
  }, [selectedSystem?.path, datVersion, games.length]);

  // Scrape All with the marked games (multi-select "Scrape (N)..."), else what the list shows
  const scrapeMarked = tool?.id === "scrapeAll" ? tool.gamePaths : undefined;
  const scrapeSubset = scrapeMarked ?? visibleGames.map((g) => g.path);

  // The bottom bar notices lead to the system status: no game selected and the right panel open
  const selectedPaths = useAppStore((s) => s.selectedPaths);
  function showStatus() {
    const state = useAppStore.getState();
    state.setMultiSelect(false);
    selectGame(null);
    if (state.infoPanelCollapsed) state.toggleInfoPanel();
  }

  // After media files moved between types: drop cached images, reload and refresh the selection
  async function reloadAfterMediaChange() {
    if (!selectedSystem) return;
    bumpMediaVersion();
    await loadGames(selectedSystem.path, { mediaChanged: true });
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
      await loadGames(selectedSystem.path, { mediaChanged: true }).catch(() => {});
      setScrapeFinished(true);
    }
  }

  // Developer credentials may be built into the app, so ask the backend instead of the settings
  const scraperStatus = useAppStore((s) => s.scraperStatus);
  const undoInfo = useAppStore((s) => s.undoInfo);
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
  // Hay otro perfil completo, con otra carpeta de ROMs, al que llevar un sistema
  const hasTransferTarget = (settings.profiles ?? []).some(
    (p) => p.id !== activeProfile?.id && isProfileComplete(p) && p.rom_root_path !== activeProfile?.rom_root_path
  );
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
      if (document.querySelector(".fixed.inset-0") || shotsRunning()) return;
      const { settings: current, patchSettings } = useAppStore.getState();
      const due = donationPromptDue(current.donation_prompt_at, Math.floor(Date.now() / 1000));
      if (due.saveAt) patchSettings({ donation_prompt_at: due.saveAt });
      if (due.show) setShowDonation(true);
    }, 2000);
    return () => clearTimeout(timer);
  }, [settingsLoaded, hasUsableProfile]);

  // Global search: Cmd+K / Ctrl+K anywhere (also while typing in a field), unless another
  // window is open on top; the same keys close it
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key.toLowerCase() !== "k" || !(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey) return;
      const { searchOpen, setSearchOpen, systems } = useAppStore.getState();
      if (!searchOpen && (document.querySelector(".fixed.inset-0:not([data-full-view])") || systems.length === 0)) return;
      e.preventDefault();
      setSearchOpen(!searchOpen);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  // "Which game?": Cmd+R / Ctrl+R anywhere (the browser's reload is cancelled), unless another
  // window is open on top (the full view doesn't count)
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key.toLowerCase() !== "r" || !(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey) return;
      e.preventDefault();
      const { randomOpen, setRandomOpen, systems } = useAppStore.getState();
      if (randomOpen || systems.length === 0 || document.querySelector(".fixed.inset-0:not([data-full-view])")) return;
      setRandomOpen(true);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  // Development only: screenshots for the website (MEROMS_SHOTS=1 pnpm tauri dev, src/dev/shots.ts)
  useEffect(() => {
    if (settingsLoaded) startShots();
  }, [settingsLoaded]);

  // New versions: look at start and then once a day while the app stays open, unless turned
  // off in Settings > About. Automatic checks fail silently (offline, dev build...)
  const checkUpdates = settings.check_updates !== false;
  useEffect(() => {
    if (!settingsLoaded || !checkUpdates) return;
    const run = () => !shotsRunning() && useUpdateStore.getState().checkNow();
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
          {!couchMode && <GameListView />}
          {selectedSystem && (
            <div className="px-6 h-12 shrink-0 border-t border-zinc-800 bg-zinc-900 flex items-center gap-2">
              {/* New ROMs / no ROM: small notices that lead to the system status (right panel) */}
              {newRomsCount > 0 && (
                <button
                  onClick={showStatus}
                  className="px-2 py-1 text-xs bg-amber-600/15 text-amber-300 border border-amber-700/50 rounded hover:bg-amber-600/25 transition-colors whitespace-nowrap"
                  title={t("newRoms.description")}
                >
                  {t("newRoms.button", { count: newRomsCount })}
                </button>
              )}
              {missingRomPaths.size > 0 && (
                <button
                  onClick={showStatus}
                  className="px-2 py-1 text-xs bg-red-600/10 text-red-300 border border-red-800/50 rounded hover:bg-red-600/20 transition-colors whitespace-nowrap"
                  title={t("missingRoms.description")}
                >
                  {t("missingRoms.button", { count: missingRomPaths.size })}
                </button>
              )}
              <div className="flex-1" />
              {/* Tools in three groups: the usual (tied to the status), the library and the rare ones */}
              <ToolsMenu
                label={t("toolbar.tools")}
                items={[
                  // On top: Undo (while there is something) and Add game
                  ...(undoInfo
                    ? [{ label: t("undo.menu", { what: t(`undo.kind_${undoInfo.kind}`), count: undoInfo.files }), onClick: () => runUndo() }]
                    : []),
                  {
                    label: t("app.addGame"),
                    onClick: () => {
                      selectGame(null);
                      setShowNewGame(true);
                    },
                  },
                  { label: t("order.button"), onClick: () => openTool("order"), groupLabel: t("toolbar.groupCollection"), shot: "tools-order" },
                  { label: t("versions.button"), onClick: () => openTool("versions") },
                  { label: t("rename.button"), onClick: () => openTool("rename") },
                  { label: t("mediaTool.button"), onClick: () => openTool("media"), shot: "tools-media" },
                  { label: t("clear.button"), onClick: () => openTool("clearFields"), shot: "tools-clear" },
                  { label: t("dat.button"), onClick: () => openTool("dat") },
                  { label: t("backup.button"), onClick: () => openTool("backups"), groupLabel: t("toolbar.groupLibrary") },
                  { label: t("collections.button"), onClick: () => openTool("collections") },
                  { label: t("chd.button"), onClick: () => openTool("chd") },
                  { label: t("bios.button"), onClick: () => openTool("bios") },
                  { label: t("ra.toolsButton"), onClick: () => openTool("ra") },
                  { label: t("export.button"), onClick: () => openTool("export"), groupLabel: t("toolbar.groupMove") },
                  { label: t("exportDevice.button"), onClick: () => openTool("exportDevice") },
                  { label: t("compare.button"), onClick: () => openTool("compare") },
                  // Take the system to another profile (only if there is one to take it to)
                  ...(hasTransferTarget ? [{ label: t("transfer.button"), onClick: () => openTool("transfer") }] : []),
                  { label: t("toolbar.migrate"), onClick: () => openTool("migrate") },
                ]}
              />
              {hasCredentials && (
                <button
                  // First choose what to update this time
                  onClick={() => openTool("scrapeAll")}
                  className="px-3 py-1.5 text-sm bg-emerald-600 text-white rounded hover:bg-emerald-500 transition-colors whitespace-nowrap"
                >
                  {t("app.scrapeAll")}
                </button>
              )}
            </div>
          )}
        </div>
        {/* Multi-select mode: the right panel edits every selected game at once. The carousel
            with "click opens the full view" uses the whole width: no panel there (its collapsed
            state is left as it was) */}
        {selectedSystem && !couchMode && viewMode !== "carousel" && (multiSelect ? <BulkEditPanel /> : <GameInfoPanel />)}
        {/* Carousel with "show it in the panel": only after a click on the centre box, and its
            button closes it (until the next click) */}
        {selectedSystem && !couchMode && viewMode === "carousel" && clickAction !== "full" && carouselPanelOpen && (
          <GameInfoPanel onClose={() => setCarouselPanelOpen(false)} />
        )}

      {/* Couch mode: the carousel full screen for a gamepad (under the full view, which A opens) */}
      {couchMode && <CouchMode />}

      {/* Play: "Playing X..." while an emulator is open, and what's missing when it can't start */}
      <PlayingBanner />
      <LaunchProblemModal />
      <RandomGameModal />
      {isTool("ra") && <RaCheckModal onClose={closeTool} />}
      <SystemViewModal open={systemViewOpen} system={selectedSystem} onClose={() => setSystemViewOpen(false)} />

      {/* Read-only game page over the whole window (the editor opens on top of it) */}
      <GameFullView />

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
        open={isTool("dat")}
        systemPath={selectedSystem?.path || ""}
        onClose={closeTool}
        onChanged={() => {
          setDatVersion((v) => v + 1);
          if (selectedSystem) {
            // New ROMs / no-ROM counters and the list (files may have gone to the trash)
            listNewRoms(selectedSystem.path).then((r) => setNewRomsCount(r.length)).catch(() => {});
          }
        }}
      />

      <ScrapeAllModal
        open={isTool("scrapeAll")}
        gameCount={games.length}
        visibleCount={scrapeSubset.length}
        marked={!!scrapeMarked}
        systemPath={selectedSystem?.path ?? ""}
        onClose={closeTool}
        onStart={(policy, onlyVisible) => {
          closeTool();
          handleScrapeAll(onlyVisible ? scrapeSubset : undefined, policy);
        }}
        onContinue={() => {
          closeTool();
          handleScrapeAll(undefined, undefined, true);
        }}
      />

      <MissingRomsModal
        open={isTool("missingRoms")}
        systemPath={selectedSystem?.path || ""}
        onClose={closeTool}
        onDone={() => {
          if (!selectedSystem) return;
          selectGame(null);
          loadGames(selectedSystem.path);
        }}
      />

      {/* Media: damaged and placeholders - leftovers - swapped types - old names */}
      <MediaModal
        open={isTool("media")}
        systemPath={selectedSystem?.path || ""}
        tab={tool?.tab}
        onClose={closeTool}
        onDone={reloadAfterMediaChange}
        onOpenSwap={() => openTool("mediaSwap")}
        // Damaged media sent to the trash: offer to download it again (only with a scraper configured)
        onRescrape={hasCredentials ? (paths, policy) => handleScrapeAll(paths, policy) : undefined}
      />

      <MediaSwapModal
        open={isTool("mediaSwap")}
        systemPath={selectedSystem?.path || ""}
        games={games}
        onClose={closeTool}
        onDone={reloadAfterMediaChange}
      />

      <NewRomsModal
        onTrashed={() => {
          // Deleting files doesn't change the games, so recount by hand
          if (selectedSystem) listNewRoms(selectedSystem.path).then((r) => setNewRomsCount(r.length)).catch(() => {});
        }}
        open={isTool("newRoms")}
        systemPath={selectedSystem?.path || ""}
        canScrape={hasCredentials}
        onClose={closeTool}
        onAdded={async (paths, scrape, rename) => {
          if (!selectedSystem) return;
          const systemPath = selectedSystem.path;
          // Scraping reloads the games when it ends; otherwise reload now
          if (scrape) await handleScrapeAll(paths);
          else await loadGames(systemPath);
          // Then rename them with Settings > Rename options (scraping already saved the
          // ScreenScraper names; without it, those ROMs are asked now)
          if (rename) await autoRename(systemPath, paths, !scrape);
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

      <RenameModal
        open={isTool("rename")}
        systemPath={selectedSystem?.path || ""}
        onClose={closeTool}
      />

      <BackupsModal
        open={isTool("backups")}
        systemPath={selectedSystem?.path || ""}
        onClose={closeTool}
      />

      <ExportModal
        open={isTool("export")}
        systemPath={selectedSystem?.path || ""}
        onClose={closeTool}
      />

      <ExportDeviceModal open={isTool("exportDevice")} onClose={closeTool} />
      <CompareModal open={isTool("compare")} onClose={closeTool} />

      <BiosModal open={isTool("bios")} onClose={closeTool} />

      <ChdModal
        open={isTool("chd")}
        systemPath={selectedSystem?.path || ""}
        systemName={selectedSystem?.full_name || selectedSystem?.name || ""}
        onClose={closeTool}
        onDone={() => {
          if (selectedSystem) loadGames(selectedSystem.path);
        }}
      />

      {/* Versions and duplicates (was Duplicates and Best version) */}
      <VersionsModal
        open={isTool("versions")}
        systemPath={selectedSystem?.path || ""}
        onlyPaths={tool?.id === "versions" ? tool.gamePaths : undefined}
        onClose={closeTool}
        onDone={() => {
          if (selectedSystem) loadGames(selectedSystem.path);
          useAppStore.getState().loadSystems();
        }}
      />

      {/* What the required data / media lines are missing, field by field and game by game */}
      <MissingModal open={isTool("missing")} kind={tool?.tab === "media" ? "media" : tool?.tab === "invalid" ? "invalid" : "data"} onClose={closeTool} />

      {/* Clear fields: a data field, a flag, the stats, the status or a media type, from many games */}
      <ClearFieldsModal
        open={isTool("clearFields")}
        systemPath={selectedSystem?.path || ""}
        visiblePaths={visibleGames.map((g) => g.path)}
        markedPaths={multiSelect ? [...selectedPaths] : []}
        initialScope={tool?.id === "clearFields" && tool.tab === "marked" ? "marked" : "all"}
        onClose={closeTool}
      />

      {/* Put it all in order: the steps of My collection in one go */}
      <OrderModal
        open={isTool("order")}
        systemPath={selectedSystem?.path || ""}
        steps={tool?.id === "order" ? tool.steps : undefined}
        initialScope={tool?.id === "order" && tool.tab === "marked" ? "marked" : "all"}
        canScrape={hasCredentials}
        visiblePaths={visibleGames.map((g) => g.path)}
        markedPaths={multiSelect ? [...selectedPaths] : []}
        totalGames={games.length}
        onClose={closeTool}
      />

      <CollectionsModal
        open={isTool("collections")}
        systemPath={selectedSystem?.path || ""}
        games={games}
        onClose={closeTool}
      />

      <StructurePromptModal />

      <MigrationModal
        open={!!migrationPreset}
        preset={migrationPreset}
        onClose={closePresetMigration}
      />
      {/* Tools > Migrate structure (the active profile; also in Settings > Profiles) */}
      <MigrationModal open={isTool("migrate")} onClose={closeTool} />

      <SettingsView
        open={showSettings}
        onClose={() => setShowSettings(false)}
      />

      <DonationModal open={showDonation} onClose={() => setShowDonation(false)} />

      <UpdateModal />

      {/* Global search (Cmd+K / Ctrl+K) */}
      <GlobalSearch />
      <ResultDialog />
      <ChoiceDialog />
      <TransferProfileModal open={isTool("transfer")} system={selectedSystem} onClose={closeTool} />
    </MainLayout>
  );
}

export default App;
