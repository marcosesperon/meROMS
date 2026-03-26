export interface Game {
  // XML attributes
  id?: string;
  source?: string;

  // Core
  path: string;
  name?: string;
  sortname?: string;
  desc?: string;

  // Media: images
  image?: string;
  thumbnail?: string;
  marquee?: string;
  fanart?: string;
  titleshot?: string;
  boxback?: string;
  boxart?: string;
  cartridge?: string;
  wheel?: string;
  mix?: string;
  manual?: string;

  // Media: video
  video?: string;

  // Metadata
  rating?: string;
  releasedate?: string;
  developer?: string;
  publisher?: string;
  genre?: string;
  genreid?: string;
  players?: string;
  family?: string;
  arcadesystemname?: string;
  region?: string;
  lang?: string;

  // Boolean flags ("true"/"false")
  favorite?: string;
  hidden?: string;
  kidgame?: string;

  // Play stats
  playcount?: string;
  lastplayed?: string;
  gametime?: string;

  // Emulation overrides
  emulator?: string;
  core?: string;

  // Hashes
  hash?: string;
  md5?: string;
  crc32?: string;

  // RetroAchievements
  cheevosHash?: string;
  cheevosId?: string;
}

export interface Folder {
  path: string;
  name?: string;
  desc?: string;
  image?: string;
  thumbnail?: string;
}

export interface Provider {
  system?: string;
  software?: string;
  database?: string;
  web?: string;
}

export interface GameList {
  games: Game[];
  folders: Folder[];
  provider?: Provider;
}

export interface SystemInfo {
  name: string;
  full_name: string;
  path: string;
  has_gamelist: boolean;
  game_count: number;
  rom_count: number;
}

export interface AppSettings {
  rom_root_path?: string;
  ss_devid?: string;
  ss_devpassword?: string;
  ss_user?: string;
  ss_password?: string;
  ss_media_types?: string[];
  tgdb_apikey?: string;
  media_paths?: MediaPathConfig[];
  grid_media_field?: string;
  /** App language: "es", "en", etc. */
  language?: string;
  /** Per-system configuration (aspect ratio, etc.) keyed by normalized system name */
  system_configs?: Record<string, SystemConfig>;
}

export interface SystemConfig {
  aspect_ratio?: string;
}

/** Available media fields that can be used as the grid image source */
export const GRID_MEDIA_OPTIONS: { id: string; labelKey: string }[] = [
  { id: "image", labelKey: "images.boxArt" },
  { id: "thumbnail", labelKey: "images.thumbnail" },
  { id: "marquee", labelKey: "images.marquee" },
  { id: "fanart", labelKey: "images.fanart" },
  { id: "titleshot", labelKey: "images.titleshot" },
  { id: "boxback", labelKey: "images.boxback" },
  { id: "boxart", labelKey: "images.boxart" },
  { id: "mix", labelKey: "images.mix" },
  { id: "wheel", labelKey: "images.wheel" },
  { id: "cartridge", labelKey: "images.cartridge" },
];

export interface ScrapeResult {
  game: Game;
  found: boolean;
}

export interface ScrapeProgress {
  current: number;
  total: number;
  game_name: string;
  status: string;
}

export interface UnusedMediaFile {
  path: string;
  relative: string;
  size: number;
}

export interface BackupEntry {
  filename: string;
  path: string;
  timestamp: string;
  size: number;
}

export interface RenameEntry {
  current_path: string;
  current_name: string;
  game_title?: string;
  proposed_name: string;
  can_rename: boolean;
}

export interface RenameRequest {
  current_path: string;
  new_name: string;
}

export type ViewMode = "table" | "grid";
export type CurrentView = "browser" | "settings";

export const AVAILABLE_MEDIA_TYPES = [
  { id: "box-2D", label: "Box Art (2D)" },
  { id: "box-3D", label: "Box Art (3D)" },
  { id: "ss", label: "Screenshot" },
  { id: "sstitle", label: "Title Screen" },
  { id: "marquee", label: "Marquee" },
  { id: "wheel", label: "Wheel Logo" },
  { id: "wheel-hd", label: "Wheel Logo (HD)" },
  { id: "fanart", label: "Fan Art" },
  { id: "screenmarquee", label: "Screen Marquee" },
  { id: "mixrbv1", label: "Mix V1" },
  { id: "mixrbv2", label: "Mix V2" },
  { id: "video", label: "Video" },
] as const;

export const DEFAULT_MEDIA_TYPES = ["box-2D", "ss", "marquee"];

/** Configuration for how each media type is saved locally */
export interface MediaPathConfig {
  /** Scraper media type ID (e.g. "box-2D", "ss") */
  mediaType: string;
  /** Subfolder inside the system folder (e.g. "covers", "screenshots") */
  subfolder: string;
  /** Suffix appended to the ROM stem before extension (empty = no suffix) */
  suffix: string;
  /** Which gamelist.xml field this maps to */
  gameField: "image" | "thumbnail" | "marquee" | "video";
}

/** Default ES-DE standard media path configuration */
export const DEFAULT_MEDIA_PATHS: MediaPathConfig[] = [
  { mediaType: "box-2D", subfolder: "covers", suffix: "", gameField: "image" },
  { mediaType: "box-3D", subfolder: "3dboxes", suffix: "", gameField: "image" },
  { mediaType: "ss", subfolder: "screenshots", suffix: "", gameField: "thumbnail" },
  { mediaType: "sstitle", subfolder: "titlescreens", suffix: "", gameField: "thumbnail" },
  { mediaType: "marquee", subfolder: "marquees", suffix: "", gameField: "marquee" },
  { mediaType: "wheel", subfolder: "marquees", suffix: "", gameField: "marquee" },
  { mediaType: "wheel-hd", subfolder: "marquees", suffix: "", gameField: "marquee" },
  { mediaType: "fanart", subfolder: "fanart", suffix: "", gameField: "image" },
  { mediaType: "screenmarquee", subfolder: "marquees", suffix: "", gameField: "marquee" },
  { mediaType: "mixrbv1", subfolder: "miximages", suffix: "", gameField: "image" },
  { mediaType: "mixrbv2", subfolder: "miximages", suffix: "", gameField: "image" },
  { mediaType: "video", subfolder: "videos", suffix: "", gameField: "video" },
];

/** Legacy EmulationStation media path configuration (uses suffixes) */
export const LEGACY_MEDIA_PATHS: MediaPathConfig[] = [
  { mediaType: "box-2D", subfolder: "images", suffix: "-image", gameField: "image" },
  { mediaType: "box-3D", subfolder: "images", suffix: "-box3d", gameField: "image" },
  { mediaType: "ss", subfolder: "images", suffix: "-screenshot", gameField: "thumbnail" },
  { mediaType: "sstitle", subfolder: "images", suffix: "-titlescreen", gameField: "thumbnail" },
  { mediaType: "marquee", subfolder: "images", suffix: "-marquee", gameField: "marquee" },
  { mediaType: "wheel", subfolder: "images", suffix: "-wheel", gameField: "marquee" },
  { mediaType: "wheel-hd", subfolder: "images", suffix: "-wheel", gameField: "marquee" },
  { mediaType: "fanart", subfolder: "images", suffix: "-fanart", gameField: "image" },
  { mediaType: "screenmarquee", subfolder: "images", suffix: "-marquee", gameField: "marquee" },
  { mediaType: "mixrbv1", subfolder: "images", suffix: "-mix", gameField: "image" },
  { mediaType: "mixrbv2", subfolder: "images", suffix: "-mix", gameField: "image" },
  { mediaType: "video", subfolder: "videos", suffix: "", gameField: "video" },
];
