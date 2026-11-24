import { create } from "zustand";
import type {
  AppSettings,
  CurrentView,
  Folder,
  Game,
  PendingStructure,
  Provider,
  SystemInfo,
  ViewMode,
} from "../types";
import * as commands from "../commands";
import { isLocale, translate, useI18nStore } from "../i18n";
import { showMessage } from "../utils/dialogs";
import { brokenKey } from "../utils/mediaFilter";
import { getActiveProfile, isProfileComplete, profileLocationKey } from "../utils/profiles";
import { newTaskId, subscribeTask, type TaskProgressInfo } from "../hooks/useTask";
import { getSystemView, shapeField } from "../utils/systemView";
import { hideWhilePlaying, showAfterPlaying } from "../utils/playWindow";
import { checkNames, statusChecks, type CollectionStatusState, type IssueKind } from "../utils/collectionStatus";

/** Herramientas del sistema (ventanas que abre App): menú Herramientas, estado del sistema… */
export type ToolId =
  | "order"
  | "versions"
  | "missing"
  | "clearFields"
  | "media"
  | "mediaSwap"
  | "rename"
  | "newRoms"
  | "missingRoms"
  | "scrapeAll"
  | "addGame"
  | "dat"
  | "bios"
  | "ra"
  | "collections"
  | "backups"
  | "export"
  | "exportDevice"
  | "compare"
  | "transfer"
  | "chd"
  | "migrate";

/** Qué herramienta abrir y cómo: pestaña (Medios), pasos marcados (Ponerlo todo en orden) o juegos */
export interface ToolRequest {
  id: ToolId;
  tab?: string;
  steps?: string[];
  gamePaths?: string[];
}

// Cada comprobación del estado lleva un número: si llega una vieja después de una nueva, no vale
let statusRun = 0;

// Library (profile + folders) we already asked about migrating, so the prompt shows
// once per profile activation and not on every rescan
let promptedLocationKey: string | null = null;

// Small UI preferences kept in the browser storage (may be unavailable: never fail because of it)
// Pending save of small UI preferences (see patchSettings)
let patchTimer: ReturnType<typeof setTimeout> | null = null;

const INFO_PANEL_KEY = "meroms.infoPanelCollapsed";
const ALL_SYSTEMS_KEY = "meroms.showAllSystems";
function readFlag(key: string): boolean {
  try {
    return localStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}
function writeFlag(key: string, value: boolean) {
  try {
    localStorage.setItem(key, value ? "1" : "0");
  } catch {
    // nothing: it's just a preference
  }
}

interface AppState {
  // Settings
  settings: AppSettings;
  /** True once the saved settings have been read (or failed to): until then `settings` is just empty */
  settingsLoaded: boolean;
  /** Which scrapers are available (refreshed whenever the settings are loaded or saved) */
  scraperStatus: commands.ScraperStatus;
  /** Ask again which scrapers can be used (after downloading or deleting the LaunchBox database) */
  refreshScraperStatus: () => void;
  /** Last file operation that can be undone (Tools > Undo) */
  undoInfo: commands.UndoInfo | null;
  refreshUndo: () => Promise<void>;
  loadSettings: () => Promise<void>;
  updateSettings: (settings: AppSettings) => Promise<void>;
  /** Small UI preferences (view, columns, last system): applied now, saved a moment later */
  patchSettings: (patch: Partial<AppSettings>) => void;
  setActiveProfile: (profileId: string) => Promise<void>;

  // Systems
  systems: SystemInfo[];
  selectedSystem: SystemInfo | null;
  loadSystems: () => Promise<void>;
  /** La carpeta del perfil activo no existe ahora (SD o disco sin conectar): pantalla inicial con aviso */
  profileUnavailable: boolean;
  /** Marca el perfil activo como no disponible (vuelve a la pantalla inicial y vacía los sistemas) o disponible */
  setProfileUnavailable: (unavailable: boolean) => void;
  selectSystem: (system: SystemInfo) => void;
  /** Back to the start screen (no system chosen; the profile's last system is kept) */
  goHome: () => void;

  // Games
  games: Game[];
  folders: Folder[];
  provider: Provider | undefined;
  selectedGame: Game | null;
  /** mediaChanged: media files were added/moved/overwritten (scrape, swap...): the grid's card
   *  shape is measured again instead of taken from what was saved. skipStatus: don't check the
   *  system status now (Put it all in order reloads between steps and checks once at the end) */
  loadGames: (systemPath: string, options?: { mediaChanged?: boolean; skipStatus?: boolean }) => Promise<void>;
  /** Background jobs still running for the selected system (card shape, statuses, new ROMs...):
   *  while > 0 a discreet "Analysing..." bar shows above the list */
  backgroundJobs: number;
  /** Counts a job in backgroundJobs until it ends (returns the same promise) */
  trackBackground: <T>(job: Promise<T>) => Promise<T>;
  selectGame: (game: Game | null) => void;
  setGames: (games: Game[]) => void;
  setFolders: (folders: Folder[]) => void;
  toggleGameFlag: (gamePath: string, field: "favorite" | "hidden" | "kidgame") => Promise<void>;

  // UI
  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;
  /** The game editor (slide-over) is open for the selected game */
  editorOpen: boolean;
  openEditor: () => void;
  closeEditor: () => void;
  /** Read-only game page over the whole window (setting "On click" or the panel's expand button) */
  fullViewOpen: boolean;
  openFullView: () => void;
  closeFullView: () => void;
  /** Couch mode: the carousel full screen, driven with a gamepad (CouchMode; not saved) */
  couchMode: boolean;
  setCouchMode: (on: boolean) => void;
  /** The game being played (an emulator is open; launch.rs) */
  runningGame: { systemPath: string; gamePath: string; name: string } | null;
  /** «Play» couldn't find what it needs: the plan says what's missing (LaunchProblemModal) */
  launchProblem: { game: Game; plan: commands.LaunchPlan } | null;
  setLaunchProblem: (problem: { game: Game; plan: commands.LaunchPlan } | null) => void;
  /** Play a game of the selected system (or say what's missing) */
  playGame: (game: Game) => Promise<void>;
  /** The emulator closed: forget the running game and take the new play stats */
  gameExited: (exited: commands.GameExited) => void;
  /** "Which game?" window (random pick with a roulette; RandomGameModal) */
  randomOpen: boolean;
  setRandomOpen: (open: boolean) => void;
  /** The system's view window (sliders button in the header; also «Configure emulator») */
  systemViewOpen: boolean;
  setSystemViewOpen: (open: boolean) => void;
  /** Global search palette (⌘K / Ctrl+K or the magnifier in the sidebar) */
  searchOpen: boolean;
  setSearchOpen: (open: boolean) => void;
  /** Paths of the games in the order the grid/table shows them (previous/next in the full view) */
  visibleOrder: string[];
  setVisibleOrder: (paths: string[]) => void;
  /** Multi-select mode (switch in the filter bar): a click marks games instead of opening them,
   * and the right panel becomes the bulk editor */
  multiSelect: boolean;
  selectedPaths: Set<string>;
  setMultiSelect: (on: boolean) => void;
  toggleSelected: (path: string) => void;
  setSelectedPaths: (paths: Set<string>) => void;
  /** Clone/BIOS marks from the system's DAT (null = the system has no DAT) */
  datMarks: Map<string, commands.DatMark> | null;
  /** Media fields pointing to files that don't exist (Classic only), keys from `brokenKey` */
  brokenMedia: Set<string>;
  /** Forma que más se repite en el medio de la cuadrícula de este sistema (la de sus tarjetas) */
  mediaShape: { system: string; field: string; width: number; height: number } | null;
  /** Vuelve a medir esa forma (al cargar un sistema o al cambiar el medio de la cuadrícula) */
  refreshMediaShape: (systemPath: string, force?: boolean) => void;
  /** RetroAchievements marks of the selected system ("Check achievements"), by game path */
  /** New and missing ROMs per system path (watching the ROM folders; sidebar marks) */
  romChanges: Record<string, commands.RomChanges>;
  /** Recount them (the given systems, or every system with a gamelist) */
  refreshRomChanges: (paths?: string[]) => void;
  setRomChange: (path: string, changes: commands.RomChanges) => void;
  raMarks: Map<string, commands.RaMark>;
  /** Reads them again (after "Check achievements") */
  refreshRaMarks: () => void;
  /** Game statuses of the selected system (only games that have one), by game path */
  statuses: Map<string, commands.GameStatus>;
  /** Change the status of some games (saved by meROMS; "completed" also in the gamelist) */
  setStatuses: (gamePaths: string[], change: commands.StatusChange) => Promise<void>;
  setDatMarks: (marks: Map<string, commands.DatMark> | null) => void;
  /** Games of the selected system whose ROM is missing on disk (marked in the grid/table) */
  missingRomPaths: Set<string>;
  setMissingRomPaths: (paths: Set<string>) => void;
  /** ROMs of the selected system that aren't in its gamelist (status line and toolbar notice) */
  newRomsCount: number;
  setNewRomsCount: (count: number) => void;
  /** La herramienta abierta (una a la vez; App pinta su ventana) */
  tool: ToolRequest | null;
  openTool: (request: ToolRequest | ToolId) => void;
  closeTool: () => void;
  /** Estado del sistema abierto frente a «Mi colección» (panel derecho, filtro «Con problemas de…») */
  collectionStatus: CollectionStatusState | null;
  /** Lo vuelve a comprobar en segundo plano (force: sin caché, «Volver a revisar») */
  checkCollectionStatus: (force?: boolean) => Promise<void>;
  /** Right-hand game info panel folded away (remembered between sessions) */
  infoPanelCollapsed: boolean;
  /** Carousel with "click shows it in the panel": the panel is open (a click on the centre box
   *  opens it, its close button hides it); not saved, off on entering the carousel or a system */
  carouselPanelOpen: boolean;
  setCarouselPanelOpen: (open: boolean) => void;
  /** Sidebar shows only systems with games (default) or every system folder */
  onlySystemsWithGames: boolean;
  toggleOnlySystemsWithGames: () => void;
  toggleInfoPanel: () => void;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  currentView: CurrentView;
  setCurrentView: (view: CurrentView) => void;
  loading: boolean;
  /** Escaneando las carpetas de sistemas (barra lateral, pantalla de inicio). Aparte de
   *  `loading` (la lista de juegos): un reescaneo tras eliminar o mover no debe desmontar la
   *  lista, o se pierde la posición */
  systemsLoading: boolean;

  // Progreso del escaneo de sistemas (barra lateral) y de la carga de un sistema (centro)
  scanProgress: TaskProgressInfo | null;
  loadProgress: TaskProgressInfo | null;

  // Filters
  filters: {
    genre?: string;
    developer?: string;
    /** Region code (a game can list several: «us, eu») */
    region?: string;
    favorite?: boolean;
    hidden?: boolean;
    kidgame?: boolean;
    /** Hide the games the system's DAT marks as clones */
    hideClones?: boolean;
    /** Only games lacking this media field ("any" = without any media) */
    missingMedia?: string;
    /** Played (has play count or last played) / never played */
    played?: "played" | "unplayed";
    /** Game status: a play status, "playing", "backlog" or "none" (no status at all) */
    status?: string;
    /** RetroAchievements: games with achievements (any version) / with a compatible ROM */
    ra?: "with" | "compatible";
    /** Solo los juegos con ese problema del estado del sistema */
    issue?: IssueKind;
  };
  setFilters: (filters: Partial<AppState["filters"]>) => void;
  clearFilters: () => void;

  // Folder browsing
  activeFolderPath: string | null;
  setActiveFolderPath: (path: string | null) => void;

  // Systems in the other structure (offered to migrate)
  pendingStructure: PendingStructure | null;
  checkPendingStructure: () => Promise<void>;
  structurePromptOpen: boolean;
  closeStructurePrompt: (dontAskAgain: boolean) => Promise<void>;
  /** Migration window pre-filled with source, destination (active profile) and systems */
  migrationPreset: PendingStructure | null;
  openPresetMigration: (systems?: string[]) => void;
  closePresetMigration: () => void;
  /** Rescan systems and reload the selected one (after a migration) */
  reloadLibrary: () => Promise<void>;
}

export const useAppStore = create<AppState>((set, get) => ({
  // Settings
  settings: {},
  settingsLoaded: false,
  scraperStatus: { built_in_dev: false, screenscraper: false, tgdb: false, launchbox: false },
  undoInfo: null,
  refreshUndo: async () => {
    const undoInfo = await commands.undoStatus().catch(() => null);
    set({ undoInfo });
  },
  refreshScraperStatus: () => {
    commands.scraperStatus().then((scraperStatus) => set({ scraperStatus })).catch(() => {});
  },
  loadSettings: async () => {
    try {
      const settings = await commands.getSettings();
      set({ settings, settingsLoaded: true, viewMode: settings.last_view ?? "grid" });
      commands.scraperStatus().then((scraperStatus) => set({ scraperStatus })).catch(() => {});
      // Sync saved language with i18n store
      if (isLocale(settings.language)) {
        useI18nStore.getState().setLocale(settings.language);
      }
      if (isProfileComplete(getActiveProfile(settings))) {
        get().loadSystems();
      }
    } catch (e) {
      console.error("Failed to load settings:", e);
      set({ settingsLoaded: true });
    }
  },
  patchSettings: (patch: Partial<AppSettings>) => {
    set({ settings: { ...get().settings, ...patch } });
    // Several quick changes (dragging a column width) end up in a single save
    if (patchTimer) clearTimeout(patchTimer);
    patchTimer = setTimeout(() => {
      patchTimer = null;
      commands.saveSettings(get().settings).catch((e) => console.error("Failed to save preferences:", e));
    }, 600);
  },
  updateSettings: async (settings: AppSettings) => {
    const previousKey = profileLocationKey(getActiveProfile(get().settings));
    await commands.saveSettings(settings);
    set({ settings });
    commands.scraperStatus().then((scraperStatus) => set({ scraperStatus })).catch(() => {});

    const active = getActiveProfile(settings);
    // Another library (or the same one with other folders/structure): start from scratch
    if (profileLocationKey(active) !== previousKey) {
      set({
        systems: [],
        selectedSystem: null,
        selectedGame: null,
        games: [],
        folders: [],
        provider: undefined,
        searchQuery: "",
        filters: {},
        activeFolderPath: null,
        collectionStatus: null,
        tool: null,
      });
    } else if (get().selectedSystem) {
      // Las opciones de nombres pueden haber cambiado: los «fuera de formato» se recalculan
      get().checkCollectionStatus();
    }
    if (isProfileComplete(active)) {
      get().loadSystems();
    }
  },
  setActiveProfile: async (profileId: string) => {
    const { settings, updateSettings } = get();
    if (settings.active_profile_id === profileId) return;
    await updateSettings({ ...settings, active_profile_id: profileId });
  },

  // Systems
  systems: [],
  selectedSystem: null,
  profileUnavailable: false,
  setProfileUnavailable: (unavailable) => {
    if (unavailable) {
      get().goHome();
      set({ systems: [], profileUnavailable: true, systemsLoading: false, scanProgress: null });
    } else {
      set({ profileUnavailable: false });
    }
  },
  loadSystems: async () => {
    const profile = getActiveProfile(get().settings);
    if (!profile || !isProfileComplete(profile)) return;
    // SD o disco sin conectar: ni se intenta (daría error); App vuelve a mirar cada pocos segundos
    const available = await commands
      .profilesAvailable([profile])
      .then((found) => found[profile.id] !== false)
      .catch(() => true);
    get().setProfileUnavailable(!available);
    if (!available) return;
    try {
      set({ systemsLoading: true, scanProgress: null });
      const task = newTaskId();
      const stop = subscribeTask(task, (p) => set({ scanProgress: p }));
      const systems = await commands.scanSystems(profile.rom_root_path, task).finally(stop);
      set({ systems, systemsLoading: false, scanProgress: null });
      // Reopen the last system of this profile (if nothing is open and it still exists)
      const last = profile.last_system && systems.find((s) => s.name === profile.last_system);
      if (last && !get().selectedSystem) get().selectSystem(last);
      get().checkPendingStructure();
    } catch (e) {
      console.error("Failed to scan systems:", e);
      set({ systemsLoading: false, scanProgress: null });
    }
  },
  goHome: () =>
    set({
      selectedSystem: null,
      selectedGame: null,
      games: [],
      folders: [],
      searchQuery: "",
      filters: {},
      multiSelect: false,
      selectedPaths: new Set(),
      fullViewOpen: false,
      editorOpen: false,
      tool: null,
      collectionStatus: null,
    }),
  selectSystem: (system: SystemInfo) => {
    // Remember it for this profile (reopened next time)
    const { settings, patchSettings } = get();
    const active = getActiveProfile(settings);
    if (active && active.last_system !== system.name) {
      patchSettings({
        profiles: (settings.profiles ?? []).map((p) => (p.id === active.id ? { ...p, last_system: system.name } : p)),
      });
    }
    set({
      selectedSystem: system,
      selectedGame: null,
      games: [],
      folders: [],
      provider: undefined,
      searchQuery: "",
      filters: {},
      activeFolderPath: null,
      // Another system: multi-select starts off and empty
      multiSelect: false,
      selectedPaths: new Set(),
      raMarks: new Map(),
      statuses: new Map(),
      carouselPanelOpen: false,
      tool: null,
      collectionStatus: null,
      newRomsCount: 0,
    });
    get().loadGames(system.path);
  },

  // Games
  games: [],
  folders: [],
  provider: undefined,
  selectedGame: null,
  loadGames: async (systemPath: string, options) => {
    try {
      // Spinner only when there's nothing on screen yet (a system just picked). A reload after
      // scraping, renaming, etc. keeps the list mounted, so the scroll position isn't lost.
      if (get().games.length === 0) set({ loading: true, loadProgress: null });
      const task = newTaskId();
      const stop = subscribeTask(task, (p) => set({ loadProgress: p }));
      const gamelist = await commands.loadGamelist(systemPath, task).finally(() => {
        stop();
        set({ loadProgress: null });
      });
      const { systems, selectedSystem } = get();
      const count = gamelist.games.length;
      const updatedSystems = systems.map((s) =>
        s.path === systemPath ? { ...s, game_count: count } : s
      );
      const updatedSelected =
        selectedSystem?.path === systemPath
          ? { ...selectedSystem, game_count: count }
          : selectedSystem;
      set({
        games: gamelist.games,
        folders: gamelist.folders || [],
        provider: gamelist.provider,
        loading: false,
        systems: updatedSystems,
        selectedSystem: updatedSelected,
      });
      // Rutas de media que apuntan a ficheros que ya no existen (para el filtro «Le falta»).
      // Va aparte para no retrasar la lista; en ES-DE siempre sale vacío.
      const track = get().trackBackground;
      track(commands.findBrokenMediaRefs(systemPath))
        .then((refs) => {
          if (get().selectedSystem?.path !== systemPath) return;
          set({ brokenMedia: new Set(refs.map((r) => brokenKey(r.game_path, r.field, r.value))) });
        })
        .catch(() => set({ brokenMedia: new Set() }));
      // Forma de las tarjetas de la cuadrícula (puede haber cambiado tras scrapear)
      get().refreshMediaShape(systemPath, options?.mediaChanged);
      // Estado frente a «Mi colección»: lo caro sale de la caché si nada ha cambiado; tras
      // scrapear o cambiar medios (un fichero sobrescrito no cambia la firma) se rehace
      // (no mientras «Ponerlo todo en orden» va recargando entre pasos: se mira al final)
      if (!options?.skipStatus) get().checkCollectionStatus(options?.mediaChanged);
      // RetroAchievements: lo que dejó «Comprobar logros» en este sistema (insignia y filtro)
      get().refreshRaMarks();
      // Estados de juego: después de leer la gamelist (lo marcado «completed» en ES-DE se adopta)
      track(commands.getGameStatuses(systemPath))
        .then((map) => {
          if (get().selectedSystem?.path !== systemPath) return;
          set({ statuses: new Map(Object.entries(map)) });
        })
        .catch(() => set({ statuses: new Map() }));
    } catch (e) {
      console.error("Failed to load gamelist:", e);
      set({ loading: false });
      // Before, a gamelist that couldn't be read just looked like an empty system
      showMessage(translate("gameList.loadFailed").replace("{error}", String(e)), "error");
    }
  },
  selectGame: (game: Game | null) => set({ selectedGame: game }),
  setGames: (games: Game[]) => {
    const { systems, selectedSystem } = get();
    // Update game_count on the selected system in the sidebar
    if (selectedSystem) {
      const updatedSystems = systems.map((s) =>
        s.path === selectedSystem.path
          ? { ...s, game_count: games.length }
          : s
      );
      const updatedSelected = { ...selectedSystem, game_count: games.length };
      set({ games, systems: updatedSystems, selectedSystem: updatedSelected });
    } else {
      set({ games });
    }
  },
  setFolders: (folders: Folder[]) => set({ folders }),
  toggleGameFlag: async (gamePath, field) => {
    const { games, folders, provider, selectedSystem, selectedGame } = get();
    if (!selectedSystem) return;
    const updatedGames = games.map((g) => {
      if (g.path !== gamePath) return g;
      const current = g[field] === "true";
      return { ...g, [field]: current ? undefined : "true" };
    });
    // Keep the selected game (info panel) in step with the list
    const updatedSelected =
      selectedGame?.path === gamePath ? updatedGames.find((g) => g.path === gamePath) ?? selectedGame : selectedGame;
    set({ games: updatedGames, selectedGame: updatedSelected });
    try {
      await commands.saveGamelist(selectedSystem.path, { games: updatedGames, folders, provider });
    } catch (e) {
      console.error("Failed to save toggle:", e);
    }
  },

  // UI
  viewMode: "grid",
  setViewMode: (mode: ViewMode) => {
    set({ viewMode: mode, carouselPanelOpen: false });
    get().patchSettings({ last_view: mode });
  },
  editorOpen: false,
  openEditor: () => set({ editorOpen: true }),
  closeEditor: () => set({ editorOpen: false }),
  fullViewOpen: false,
  openFullView: () => set({ fullViewOpen: true }),
  closeFullView: () => set({ fullViewOpen: false }),
  couchMode: false,
  setCouchMode: (on) => set({ couchMode: on }),
  runningGame: null,
  launchProblem: null,
  setLaunchProblem: (problem) => set({ launchProblem: problem }),
  playGame: async (game) => {
    const { selectedSystem, runningGame } = get();
    if (!selectedSystem || runningGame) return;
    // Primero, cómo se lanzaría: si falta algo, se dice qué (y no se intenta)
    const plan = await commands.resolveLaunch(selectedSystem.path, game.path);
    if (!plan.command) {
      set({ launchProblem: { game, plan } });
      return;
    }
    try {
      await commands.launchGame(selectedSystem.path, game.path);
      set({ runningGame: { systemPath: selectedSystem.path, gamePath: game.path, name: game.name || game.path } });
    } catch (e) {
      showMessage(`${translate("play.failed")}\n\n${String(e)}`, "error");
      return;
    }
    // meROMS se aparta para que el emulador quede delante; si el emulador ya se ha cerrado
    // mientras se ocultaba, vuelve en seguida
    await hideWhilePlaying();
    if (!get().runningGame) showAfterPlaying(get().couchMode);
  },
  gameExited: (exited) => {
    set({ runningGame: null });
    showAfterPlaying(get().couchMode);
    // Las estadísticas nuevas, si es el sistema que se ve
    const { selectedSystem, games, selectedGame } = get();
    const updated = exited.game;
    if (!updated || selectedSystem?.path !== exited.system_path) return;
    set({
      games: games.map((g) => (g.path === updated.path ? { ...g, playcount: updated.playcount, lastplayed: updated.lastplayed, gametime: updated.gametime } : g)),
      selectedGame:
        selectedGame?.path === updated.path
          ? { ...selectedGame, playcount: updated.playcount, lastplayed: updated.lastplayed, gametime: updated.gametime }
          : selectedGame,
    });
  },
  romChanges: {},
  refreshRomChanges: (paths) => {
    const list = paths ?? get().systems.filter((s) => s.has_gamelist).map((s) => s.path);
    if (list.length === 0) return;
    commands
      .romChanges(list)
      .then((found) => set({ romChanges: paths ? { ...get().romChanges, ...found } : found }))
      .catch(() => {});
  },
  setRomChange: (path, changes) => set({ romChanges: { ...get().romChanges, [path]: changes } }),
  raMarks: new Map(),
  refreshRaMarks: () => {
    const systemPath = get().selectedSystem?.path;
    if (!systemPath) return;
    get()
      .trackBackground(commands.raSystemMarks(systemPath))
      .then((map) => {
        if (get().selectedSystem?.path === systemPath) set({ raMarks: new Map(Object.entries(map)) });
      })
      .catch(() => set({ raMarks: new Map() }));
  },
  randomOpen: false,
  setRandomOpen: (open) => set({ randomOpen: open }),
  systemViewOpen: false,
  setSystemViewOpen: (open) => set({ systemViewOpen: open }),
  searchOpen: false,
  setSearchOpen: (open) => set({ searchOpen: open }),
  visibleOrder: [],
  setVisibleOrder: (paths) => set({ visibleOrder: paths }),
  multiSelect: false,
  selectedPaths: new Set(),
  setMultiSelect: (on) => set({ multiSelect: on, selectedPaths: new Set(), selectedGame: null }),
  toggleSelected: (path) => {
    const next = new Set(get().selectedPaths);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    set({ selectedPaths: next });
  },
  setSelectedPaths: (paths) => set({ selectedPaths: paths }),
  datMarks: null,
  brokenMedia: new Set(),
  mediaShape: null,
  backgroundJobs: 0,
  trackBackground: (job) => {
    set({ backgroundJobs: get().backgroundJobs + 1 });
    const done = () => set({ backgroundJobs: Math.max(0, get().backgroundJobs - 1) });
    job.then(done, done);
    return job;
  },
  refreshMediaShape: (systemPath: string, force?: boolean) => {
    const { settings, selectedSystem, viewMode } = get();
    // Con una relación de aspecto elegida a mano para el sistema, esa manda: no hace falta medir
    if (getSystemView(settings, selectedSystem?.name).aspect_ratio) return;
    const field = shapeField(settings, selectedSystem?.name, viewMode);
    get()
      .trackBackground(commands.usualMediaShape(systemPath, field, force))
      .then((shape) => {
        // Si mientras tanto se ha cambiado de sistema o de medio, esta respuesta ya no vale
        const now = get();
        if (now.selectedSystem?.path !== systemPath || shapeField(now.settings, now.selectedSystem?.name, now.viewMode) !== field) return;
        set({ mediaShape: shape ? { system: systemPath, field, width: shape.width, height: shape.height } : null });
      })
      .catch(() => set({ mediaShape: null }));
  },
  statuses: new Map(),
  setStatuses: async (gamePaths, change) => {
    const { selectedSystem } = get();
    if (!selectedSystem || gamePaths.length === 0) return;
    const updated = await commands.setGameStatuses(selectedSystem.path, gamePaths, change);
    const statuses = new Map(get().statuses);
    // Los juegos sin nada quedan fuera del mapa (como los devuelve get_game_statuses)
    for (const path of gamePaths) {
      const s = updated[path];
      if (s && (s.status || s.playing || s.backlog)) statuses.set(path, s);
      else statuses.delete(path);
    }
    // La gamelist ya lleva el «completed» nuevo: se copia a los juegos en memoria, para que
    // un guardado del editor no lo pise con el valor viejo (misma regla que game_status.rs)
    const done = (p: string) => ["finished", "completed"].includes(statuses.get(p)?.status ?? "");
    const touched = new Set(gamePaths);
    const fix = (g: Game): Game => {
      if (!touched.has(g.path)) return g;
      const completed = done(g.path) ? "true" : g.completed === undefined ? undefined : "false";
      return completed === g.completed ? g : { ...g, completed };
    };
    const { games, selectedGame } = get();
    set({ statuses, games: games.map(fix), selectedGame: selectedGame ? fix(selectedGame) : null });
  },
  setDatMarks: (marks) => set({ datMarks: marks }),
  missingRomPaths: new Set(),
  setMissingRomPaths: (paths) => set({ missingRomPaths: paths }),
  newRomsCount: 0,
  setNewRomsCount: (count) => set({ newRomsCount: count }),
  tool: null,
  openTool: (request) => set({ tool: typeof request === "string" ? { id: request } : request }),
  closeTool: () => set({ tool: null }),
  collectionStatus: null,
  checkCollectionStatus: async (force) => {
    const system = get().selectedSystem?.path;
    if (!system) return;
    const run = ++statusRun;
    const previous = get().collectionStatus;
    // Lo que ya se sabía de este sistema se sigue viendo mientras se comprueba (y los números
    // se animan hasta el valor nuevo cuando llega)
    set({
      collectionStatus:
        previous?.system === system
          ? { ...previous, checking: true, errors: [] }
          : { system, parts: {}, names: null, checking: true, errors: [], checkedAt: null },
    });
    const { trackBackground, settings, scraperStatus } = get();
    const current = () => run === statusRun && get().selectedSystem?.path === system;
    // «Volver a revisar» (o tras scrapear): lo guardado en el backend ya no vale
    if (force) await commands.forgetSystemStatus(system).catch(() => {});
    const failed = (name: string) => (e: unknown) => {
      console.error(`Status check ${name} failed:`, e);
      if (current()) set({ collectionStatus: { ...get().collectionStatus!, errors: [...get().collectionStatus!.errors, `${name}: ${String(e)}`] } });
    };
    // Todas a la vez; cada parte se enseña en cuanto llega
    const jobs = statusChecks(system).map(({ name, run: check }) =>
      trackBackground(check())
        .then((part) => {
          if (current()) set({ collectionStatus: { ...get().collectionStatus!, parts: { ...get().collectionStatus!.parts, ...part } } });
        })
        .catch(failed(name))
    );
    jobs.push(
      trackBackground(checkNames(system, settings.rename_options, scraperStatus))
        .then((n) => {
          if (current()) set({ collectionStatus: { ...get().collectionStatus!, names: n } });
        })
        .catch(failed("names"))
    );
    await Promise.all(jobs);
    if (current()) set({ collectionStatus: { ...get().collectionStatus!, checking: false, checkedAt: Date.now() } });
  },
  infoPanelCollapsed: readFlag(INFO_PANEL_KEY),
  carouselPanelOpen: false,
  setCarouselPanelOpen: (open) => set({ carouselPanelOpen: open }),
  // Stored as "show all" so that the default (nothing stored) is "only with games"
  onlySystemsWithGames: !readFlag(ALL_SYSTEMS_KEY),
  toggleOnlySystemsWithGames: () => {
    const only = !get().onlySystemsWithGames;
    writeFlag(ALL_SYSTEMS_KEY, !only);
    set({ onlySystemsWithGames: only });
  },
  toggleInfoPanel: () => {
    const collapsed = !get().infoPanelCollapsed;
    writeFlag(INFO_PANEL_KEY, collapsed);
    set({ infoPanelCollapsed: collapsed });
  },
  searchQuery: "",
  setSearchQuery: (query: string) => set({ searchQuery: query }),
  currentView: "browser",
  setCurrentView: (view: CurrentView) => set({ currentView: view }),
  loading: false,
  systemsLoading: false,

  // Scan progress
  scanProgress: null,
  loadProgress: null,

  // Filters
  filters: {},
  setFilters: (newFilters) =>
    set((s) => ({ filters: { ...s.filters, ...newFilters } })),
  clearFilters: () => set({ filters: {} }),

  // Folder browsing
  activeFolderPath: null,
  setActiveFolderPath: (path) => set({ activeFolderPath: path }),

  // Systems in the other structure
  pendingStructure: null,
  structurePromptOpen: false,
  migrationPreset: null,
  checkPendingStructure: async () => {
    const profile = getActiveProfile(get().settings);
    try {
      const pending = await commands.detectPendingStructure();
      const key = profileLocationKey(profile);
      const justEntered = key !== promptedLocationKey;
      promptedLocationKey = key;
      set({
        pendingStructure: pending,
        structurePromptOpen: !!pending && justEntered && !profile?.skip_structure_prompt,
      });
    } catch (e) {
      console.error("Failed to detect pending structure:", e);
      set({ pendingStructure: null });
    }
  },
  closeStructurePrompt: async (dontAskAgain: boolean) => {
    set({ structurePromptOpen: false });
    if (!dontAskAgain) return;
    const { settings, updateSettings } = get();
    const active = getActiveProfile(settings);
    if (!active) return;
    const profiles = (settings.profiles ?? []).map((p) =>
      p.id === active.id ? { ...p, skip_structure_prompt: true } : p
    );
    await updateSettings({ ...settings, profiles });
  },
  openPresetMigration: (systems?: string[]) => {
    const pending = get().pendingStructure;
    if (!pending) return;
    set({ migrationPreset: { source: pending.source, systems: systems ?? pending.systems } });
  },
  closePresetMigration: () => set({ migrationPreset: null }),
  reloadLibrary: async () => {
    await get().loadSystems();
    const { systems, selectedSystem } = get();
    if (!selectedSystem) return;
    const fresh = systems.find((s) => s.path === selectedSystem.path);
    set({ selectedSystem: fresh ?? null, selectedGame: null });
    if (fresh) get().loadGames(fresh.path);
  },
}));
