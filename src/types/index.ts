import type { TranslationKeys } from "../i18n/en";
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

/** How a library is organised on disk */
export type StructureKind = "classic" | "esde";

/** A saved library: where the ROMs are and which structure it follows */
export interface Profile {
  id: string;
  name: string;
  kind: StructureKind;
  rom_root_path: string;
  /** ES-DE only: ES-DE data folder (has settings/, gamelists/, collections/…) */
  esde_home_path?: string;
  /** ES-DE only: media folder when it isn't <ES-DE>/downloaded_media */
  media_dir?: string;
  /** "Don't ask again": no prompt when systems are found in the other structure */
  skip_structure_prompt?: boolean;
  /** Last system opened with this profile (folder name), reopened on start */
  last_system?: string;
  /** BIOS folder (Tools > BIOS); missing = the usual one next to the ROM folder */
  bios_dir?: string;
}

/** Systems of the active profile that are in the other structure, and where to migrate them from */
export interface PendingStructure {
  /** May be a profile that is not saved (detected classic data, or ~/ES-DE) */
  source: Profile;
  systems: string[];
}

/** Paths read from an ES-DE folder's es_settings.xml */
export interface EsdeDetectedPaths {
  /** ES-DE data folder to use (portable: the "ES-DE" subfolder of the one picked) */
  esde_home: string;
  /** Portable install (ES-DE.exe or portable.txt next to the data folder) */
  portable: boolean;
  rom_root_path: string;
  media_dir: string;
  settings_found: boolean;
  legacy_gamelist_location: boolean;
}

export interface AppSettings {
  /** Legacy single ROM root (before profiles). Migrated by the backend; don't use. */
  rom_root_path?: string;
  profiles?: Profile[];
  active_profile_id?: string;
  /** ROM name cleanup rules; undefined = built-in defaults */
  rename_rules?: RenameRule[];
  ss_devid?: string;
  ss_devpassword?: string;
  ss_user?: string;
  ss_password?: string;
  ss_media_types?: string[];
  /** What the scraper may update (missing = derived from ss_media_types by the backend) */
  scrape_fields?: ScrapePolicy;
  /** ScreenScraper region / language order (missing = defaults from the app language) */
  ss_regions?: string[];
  /** Table view: visible columns in order with their widths (same for every system) */
  table_columns?: { id: string; width?: number }[];
  /** Last list view */
  last_view?: "grid" | "table";
  /** Grid order (missing = by name) */
  grid_sort?: "name" | "most_played" | "recent" | "playtime" | "release" | "rating";
  /** Clic en un juego: enseñarlo en el panel derecho ("panel", por defecto) o abrir la ficha a pantalla completa */
  click_action?: "panel" | "full";
  /** Donation prompt: unix seconds of the first launch, then of the last time it was shown */
  donation_prompt_at?: number;
  /** Look for a new version at start and once a day (missing = yes) */
  check_updates?: boolean;
  ss_languages?: string[];
  tgdb_apikey?: string;
  /** "Export to device": destinations used, with their selection */
  export_targets?: { dest: string; systems: string[]; games: "all" | "favorites" | "visible"; media: string[]; remove_extra: boolean }[];
  /** "Compare with another folder": the last folder compared */
  compare_dest?: string;
  /** "Convert to CHD": chdman chosen by hand (missing = found in the PATH or the usual places) */
  chdman_path?: string;
  /** DeepL API key, to translate descriptions */
  deepl_apikey?: string;
  /** Order of the games when a gamelist is written (missing = as they are) */
  gamelist_sort?: "name" | "path";
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
  /** ROM path of the game (the progress window offers a manual search for the ones not found) */
  game_path: string;
  status: string;
}

/** One result of a manual search (the full data is fetched with applyScrape once picked) */
export interface ScrapeCandidate {
  source: "screenscraper" | "thegamesdb" | "launchbox" | "arcadeitalia";
  id: string;
  name: string;
  system: string | null;
  year: string | null;
  developer: string | null;
  /** data: URI or public URL; ScreenScraper ones come later with scrapeThumbnail */
  thumbnail: string | null;
  /** ScreenScraper recognised the ROM file itself */
  rom_match: boolean;
}

/** How a Scrape All run ended */
export interface ScrapeRunResult {
  /** Ended by Stop: the rest can be continued from Scrape All */
  stopped: boolean;
}

/** Scrape All left halfway in a system */
export interface ScrapeResumeInfo {
  remaining: number;
  total: number;
  /** Unix seconds */
  started_at: number;
}

export interface ScrapeSearch {
  candidates: ScrapeCandidate[];
  /** Sources that failed (the others still give results) */
  errors: string[];
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
  /** Tracks of a .cue/.gdi renamed along with it (the ones named after the game) */
  tracks?: number;
  /** duplicate = another ROM would get the same name · exists = a different file has it */
  conflict?: "duplicate" | "exists";
}

/** Where the proposed name comes from */
export type RenameSource = "title" | "rules" | "dat";

export interface RenameResult {
  gamelist: GameList;
  renamed: number;
  skipped: string[];
}

/** ROM name cleanup rule (rules.json format of ES-DE-Roms-Renamer, plus "enabled") */
export interface RenameRule {
  pattern: string;
  replacement: string;
  description?: string;
  enabled: boolean;
}

export interface RuleTestResult {
  result: string;
  errors: { index: number; message: string }[];
}

export interface RenameRequest {
  current_path: string;
  new_name: string;
}

/** What the scraper does with one data field or media type */
export type FieldPolicy = "overwrite" | "if_empty" | "keep";

export interface ScrapePolicy {
  data: Record<string, FieldPolicy>;
  media: Record<string, FieldPolicy>;
}

/** Image media types that can be swapped / re-typed (mirrors media::MEDIA_KINDS, ES-DE ones) */
export const SWAPPABLE_MEDIA: { field: keyof Game; labelKey: TranslationKeys }[] = [
  { field: "image", labelKey: "images.boxArt" },
  { field: "thumbnail", labelKey: "info.screenshot" },
  { field: "titleshot", labelKey: "images.titleshot" },
  { field: "boxart", labelKey: "images.boxart" },
  { field: "boxback", labelKey: "images.boxback" },
  { field: "marquee", labelKey: "images.marquee" },
  { field: "fanart", labelKey: "images.fanart" },
  { field: "mix", labelKey: "images.mix" },
  { field: "cartridge", labelKey: "images.cartridge" },
];

export interface SwapResult {
  swapped: number;
  skipped: string[];
}

export interface MediaFileInfo {
  path: string;
  width: number;
  height: number;
}

/** Usual size of covers / screenshots in a system */
export interface MediaProfile {
  width: number;
  height: number;
}

export interface DetectResult {
  cover_profile: MediaProfile | null;
  screenshot_profile: MediaProfile | null;
  /** The whole system looks the other way round (use "Swap media types") */
  looks_swapped: boolean;
  proposals: SwapProposal[];
}

/** A game whose cover and screenshot look swapped (compared with the system's usual sizes) */
export interface SwapProposal {
  game_path: string;
  game_name: string;
  cover: MediaFileInfo | null;
  screenshot: MediaFileInfo | null;
}

/** A ROM file in the system folder that isn't in the gamelist */
export interface NewRom {
  /** As it will go in the gamelist: "./sub/Game.sms" */
  path: string;
  proposed_name: string;
  /** false = extension not in the system's list (offered unchecked) */
  known_extension: boolean;
}

/** A gamelist media field pointing to a file that doesn't exist (Classic) */
export interface BrokenMediaRef {
  game_path: string;
  game_name: string;
  field: string;
  value: string;
}

/** A game with the same file in more than one media type */
export interface RepeatedMedia {
  game_path: string;
  game_name: string;
  keep_path: string;
  keep_field: string;
  remove: { path: string; field: string; size: number }[];
}

export interface TrashResult {
  trashed: number;
  failed: string[];
}

export interface NewRomEntry {
  path: string;
  name: string;
}

/** One proposed change of the media normalizer (Classic profiles) */
export interface NormalizeItem {
  game_path: string;
  game_name: string;
  field: string;
  from: string;
  to: string;
  from_relative: string;
  to_relative: string;
  /** move = file is moved/renamed · link = only the field changes · conflict = destination taken */
  status: "move" | "link" | "conflict";
}

export interface NormalizeResult {
  moved: number;
  linked: number;
  skipped: string[];
}

export type MigrationMode = "copy" | "move";

/** Per-system result (or preview) of a structure migration */
export interface MigrationSystemReport {
  name: string;
  has_gamelist: boolean;
  dest_gamelist_exists: boolean;
  games: number;
  media_transferred: number;
  media_skipped_count: number;
  media_skipped: string[];
  media_unsupported: number;
  /** Classic -> ES-DE: legacy-named media that go over already renamed (included in media_transferred) */
  media_renamed: number;
}

export interface MigrationReport {
  dry_run: boolean;
  systems: MigrationSystemReport[];
  collections: string[];
  warnings: string[];
}

export interface MigrationProgress {
  current: number;
  total: number;
  system: string;
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

/** Gamelist fields that can hold a media path */
export type MediaGameField =
  | "image" | "thumbnail" | "marquee" | "video" | "fanart" | "titleshot"
  | "boxback" | "boxart" | "wheel" | "mix" | "cartridge" | "manual";

export const MEDIA_GAME_FIELDS: MediaGameField[] = [
  "image", "thumbnail", "marquee", "video", "fanart", "titleshot",
  "boxback", "boxart", "wheel", "mix", "cartridge", "manual",
];

/**
 * Folder each scraper media type is saved to (ES-DE folder names, same in every profile).
 * Mirrors media::kind_for_scraper_type in Rust.
 */
export const MEDIA_TYPE_FOLDERS: Record<string, string> = {
  "box-2D": "covers",
  "box-3D": "3dboxes",
  ss: "screenshots",
  sstitle: "titlescreens",
  marquee: "marquees",
  wheel: "marquees",
  "wheel-hd": "marquees",
  fanart: "fanart",
  screenmarquee: "marquees",
  mixrbv1: "miximages",
  mixrbv2: "miximages",
  video: "videos",
};

/** Per scraper media type: which gamelist field it fills (Classic structure only) */
export interface MediaPathConfig {
  /** Scraper media type ID (e.g. "box-2D", "ss") */
  mediaType: string;
  /** Legacy: folder used before the unified naming. Only a hint for the media normalizer. */
  subfolder?: string;
  /** Legacy: suffix used before the unified naming. Only a hint for the media normalizer. */
  suffix?: string;
  /** Which gamelist.xml field this maps to */
  gameField: MediaGameField;
}

/** Default field for each media type (meROMS' historical choice) */
export const DEFAULT_MEDIA_PATHS: MediaPathConfig[] = [
  { mediaType: "box-2D", gameField: "image" },
  { mediaType: "box-3D", gameField: "image" },
  { mediaType: "ss", gameField: "thumbnail" },
  { mediaType: "sstitle", gameField: "thumbnail" },
  { mediaType: "marquee", gameField: "marquee" },
  { mediaType: "wheel", gameField: "marquee" },
  { mediaType: "wheel-hd", gameField: "marquee" },
  { mediaType: "fanart", gameField: "image" },
  { mediaType: "screenmarquee", gameField: "marquee" },
  { mediaType: "mixrbv1", gameField: "image" },
  { mediaType: "mixrbv2", gameField: "image" },
  { mediaType: "video", gameField: "video" },
];
