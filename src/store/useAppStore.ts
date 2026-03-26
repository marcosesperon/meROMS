import { create } from "zustand";
import type {
  AppSettings,
  CurrentView,
  Folder,
  Game,
  Provider,
  SystemInfo,
  ViewMode,
} from "../types";
import * as commands from "../commands";
import { useI18nStore } from "../i18n";

interface AppState {
  // Settings
  settings: AppSettings;
  loadSettings: () => Promise<void>;
  updateSettings: (settings: AppSettings) => Promise<void>;

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
}

export const useAppStore = create<AppState>((set, get) => ({
  // Settings
  settings: {},
  loadSettings: async () => {
    try {
      const settings = await commands.getSettings();
      set({ settings });
      // Sync saved language with i18n store
      if (settings.language === "es" || settings.language === "en") {
        useI18nStore.getState().setLocale(settings.language);
      }
      if (settings.rom_root_path) {
        get().loadSystems();
      }
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  },
  updateSettings: async (settings: AppSettings) => {
    await commands.saveSettings(settings);
    set({ settings });
    if (settings.rom_root_path) {
      get().loadSystems();
    }
  },

  // Systems
  systems: [],
  selectedSystem: null,
  loadSystems: async () => {
    const { settings } = get();
    if (!settings.rom_root_path) return;
    try {
      set({ loading: true, scanProgress: null });
      const systems = await commands.scanSystems(settings.rom_root_path);
      set({ systems, loading: false, scanProgress: null });
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
      set({ loading: true });
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
    const { games, folders, provider, selectedSystem } = get();
    if (!selectedSystem) return;
    const updatedGames = games.map((g) => {
      if (g.path !== gamePath) return g;
      const current = g[field] === "true";
      return { ...g, [field]: current ? undefined : "true" };
    });
    set({ games: updatedGames });
    try {
      await commands.saveGamelist(selectedSystem.path, { games: updatedGames, folders, provider });
    } catch (e) {
      console.error("Failed to save toggle:", e);
    }
  },

  // UI
  viewMode: "table",
  setViewMode: (mode: ViewMode) => set({ viewMode: mode }),
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
}));
