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
import { useI18nStore } from "../i18n";
import { getActiveProfile, isProfileComplete, profileLocationKey } from "../utils/profiles";

// Library (profile + folders) we already asked about migrating, so the prompt shows
// once per profile activation and not on every rescan
let promptedLocationKey: string | null = null;

// Small UI preferences kept in the browser storage (may be unavailable: never fail because of it)
const INFO_PANEL_KEY = "meroms.infoPanelCollapsed";
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
  loadSettings: () => Promise<void>;
  updateSettings: (settings: AppSettings) => Promise<void>;
  setActiveProfile: (profileId: string) => Promise<void>;

  // Systems
  systems: SystemInfo[];
  selectedSystem: SystemInfo | null;
  loadSystems: () => Promise<void>;
  selectSystem: (system: SystemInfo) => void;

  // Games
  games: Game[];
  folders: Folder[];
  provider: Provider | undefined;
  selectedGame: Game | null;
  loadGames: (systemPath: string) => Promise<void>;
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
  /** Right-hand game info panel folded away (remembered between sessions) */
  infoPanelCollapsed: boolean;
  toggleInfoPanel: () => void;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  currentView: CurrentView;
  setCurrentView: (view: CurrentView) => void;
  loading: boolean;

  // Scan progress
  scanProgress: { current: number; total: number; currentName: string } | null;
  setScanProgress: (progress: { current: number; total: number; currentName: string } | null) => void;

  // Filters
  filters: {
    genre?: string;
    developer?: string;
    favorite?: boolean;
    hidden?: boolean;
    kidgame?: boolean;
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
  loadSettings: async () => {
    try {
      const settings = await commands.getSettings();
      set({ settings, settingsLoaded: true });
      // Sync saved language with i18n store
      if (settings.language === "es" || settings.language === "en") {
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
  updateSettings: async (settings: AppSettings) => {
    const previousKey = profileLocationKey(getActiveProfile(get().settings));
    await commands.saveSettings(settings);
    set({ settings });

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
      });
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
  loadSystems: async () => {
    const profile = getActiveProfile(get().settings);
    if (!profile || !isProfileComplete(profile)) return;
    try {
      set({ loading: true, scanProgress: null });
      const systems = await commands.scanSystems(profile.rom_root_path);
      set({ systems, loading: false, scanProgress: null });
      get().checkPendingStructure();
    } catch (e) {
      console.error("Failed to scan systems:", e);
      set({ loading: false, scanProgress: null });
    }
  },
  selectSystem: (system: SystemInfo) => {
    set({
      selectedSystem: system,
      selectedGame: null,
      games: [],
      folders: [],
      provider: undefined,
      searchQuery: "",
      filters: {},
      activeFolderPath: null,
    });
    get().loadGames(system.path);
  },

  // Games
  games: [],
  folders: [],
  provider: undefined,
  selectedGame: null,
  loadGames: async (systemPath: string) => {
    try {
      // Spinner only when there's nothing on screen yet (a system just picked). A reload after
      // scraping, renaming, etc. keeps the list mounted, so the scroll position isn't lost.
      if (get().games.length === 0) set({ loading: true });
      const gamelist = await commands.loadGamelist(systemPath);
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
    } catch (e) {
      console.error("Failed to load gamelist:", e);
      set({ loading: false });
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
  setViewMode: (mode: ViewMode) => set({ viewMode: mode }),
  editorOpen: false,
  openEditor: () => set({ editorOpen: true }),
  closeEditor: () => set({ editorOpen: false }),
  infoPanelCollapsed: readFlag(INFO_PANEL_KEY),
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

  // Scan progress
  scanProgress: null,
  setScanProgress: (progress) => set({ scanProgress: progress }),

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
