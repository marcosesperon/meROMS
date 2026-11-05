import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, BackupEntry, EsdeDetectedPaths, Folder, MigrationMode, MigrationReport, NewRom, NewRomEntry, DetectResult, TrashResult, ScrapePolicy, RepeatedMedia, BrokenMediaRef, SwapResult, NormalizeItem, NormalizeResult, PendingStructure, Profile, RenameResult, RenameRule, RenameSource, RuleTestResult, Game, GameList, RenameEntry, RenameRequest, ScrapeResult, ScrapeResumeInfo, ScrapeRunResult, ScrapeSearch, SystemInfo, UnusedMediaFile } from "../types";

export async function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  return invoke("save_settings", { settings });
}

export async function exportSettings(destPath: string): Promise<void> {
  return invoke("export_settings", { destPath });
}

export async function importSettings(filePath: string): Promise<AppSettings> {
  return invoke("import_settings", { filePath });
}

/** Reads ROM/media folders from an ES-DE folder's settings/es_settings.xml */
export async function detectEsdePaths(esdeHome: string): Promise<EsdeDetectedPaths> {
  return invoke("detect_esde_paths", { esdeHome });
}

export async function scanSystems(rootPath: string): Promise<SystemInfo[]> {
  return invoke("scan_systems", { rootPath });
}

export async function loadGamelist(systemPath: string): Promise<GameList> {
  return invoke("load_gamelist", { systemPath });
}

export async function saveGamelist(
  systemPath: string,
  gamelist: GameList
): Promise<void> {
  return invoke("save_gamelist", { systemPath, gamelist });
}

export async function addGame(
  systemPath: string,
  game: Game
): Promise<GameList> {
  return invoke("add_game", { systemPath, game });
}

export async function removeGame(
  systemPath: string,
  gamePath: string
): Promise<GameList> {
  return invoke("remove_game", { systemPath, gamePath });
}

export async function generateGamelist(
  systemPath: string
): Promise<GameList> {
  return invoke("generate_gamelist", { systemPath });
}

/** Which scrapers can be used (ScreenScraper developer credentials may be built into the app) */
export interface ScraperStatus {
  built_in_dev: boolean;
  screenscraper: boolean;
  tgdb: boolean;
  /** LaunchBox database downloaded */
  launchbox: boolean;
}

/** LaunchBox database copy (Settings) */
export interface LaunchBoxStatus {
  downloaded: boolean;
  /** Unix seconds */
  downloaded_at: number | null;
  games: number;
  platforms: number;
}

export async function launchboxStatus(): Promise<LaunchBoxStatus> {
  return invoke("launchbox_status");
}

/** Downloads Metadata.zip (~110 MB) and keeps it summarised per platform; emits launchbox-progress */
export async function launchboxDownload(): Promise<LaunchBoxStatus> {
  return invoke("launchbox_download");
}

export async function launchboxDelete(): Promise<void> {
  return invoke("launchbox_delete");
}

export async function scraperStatus(): Promise<ScraperStatus> {
  return invoke("scraper_status");
}

export async function scrapeGame(
  systemPath: string,
  gamePath: string
): Promise<ScrapeResult> {
  return invoke("scrape_game", { systemPath, gamePath });
}

/** Manual search for one game: ScreenScraper (ROM match + text) and TheGamesDB, in one list */
export async function searchScrape(
  systemPath: string,
  gamePath: string,
  query: string,
  allSystems: boolean
): Promise<ScrapeSearch> {
  return invoke("search_scrape", { systemPath, gamePath, query, allSystems });
}

/** Small cover of a ScreenScraper result of the last search (data: URI, or null) */
export async function scrapeThumbnail(id: string): Promise<string | null> {
  return invoke("scrape_thumbnail", { id });
}

/** Full data of the picked result, written into the game as the policy says (not saved) */
export async function applyScrape(
  systemPath: string,
  gamePath: string,
  source: string,
  id: string
): Promise<ScrapeResult> {
  return invoke("apply_scrape", { systemPath, gamePath, source, id });
}

/** Scrape a whole system, or only `gamePaths` (e.g. the ROMs just added); `policy` = what may be
 * updated this time (missing = Settings) */
export async function scrapeSystem(
  systemPath: string,
  gamePaths?: string[],
  policy?: ScrapePolicy,
  /** Continue the run left halfway in this system (its games and rules) */
  resume?: boolean
): Promise<ScrapeRunResult> {
  return invoke("scrape_system", { systemPath, gamePaths, policy, resume });
}

/** Stop the running Scrape All (the game in progress finishes; what's done is saved) */
export async function cancelScrape(): Promise<void> {
  return invoke("cancel_scrape");
}

/** Scrape All left halfway in this system (app closed, crash or Stop), if any */
export async function getScrapeResume(systemPath: string): Promise<ScrapeResumeInfo | null> {
  return invoke("get_scrape_resume", { systemPath });
}

/** Region and language order the scraper uses (saved, or defaults from the app language) */
export async function getScrapeOrder(): Promise<{ regions: string[]; languages: string[] }> {
  return invoke("get_scrape_order");
}

/** What the scraper may update (saved, or derived from the old "media to download" list) */
export async function getScrapePolicy(): Promise<ScrapePolicy> {
  return invoke("get_scrape_policy");
}

/** Swap two media types (gamelist fields, e.g. "image" and "thumbnail") in the given games */
export async function swapMedia(systemPath: string, gamePaths: string[], fieldA: string, fieldB: string): Promise<SwapResult> {
  return invoke("swap_media", { systemPath, gamePaths, fieldA, fieldB });
}

/** Games whose cover and screenshot look swapped */
export async function detectSwappedMedia(systemPath: string): Promise<DetectResult> {
  return invoke("detect_swapped_media", { systemPath });
}

/** ROM files in the system folder that the gamelist doesn't have */
export async function listNewRoms(systemPath: string): Promise<NewRom[]> {
  return invoke("list_new_roms", { systemPath });
}

/** Move files of the system folder to the system trash (only paths inside that folder) */
export async function trashFiles(systemPath: string, paths: string[]): Promise<TrashResult> {
  return invoke("trash_files", { systemPath, paths });
}

/** Games of the gamelist whose ROM file/folder no longer exists */
export interface MissingRoms {
  games: { path: string; name: string }[];
  /** Every ROM is missing: the folder or drive is probably not connected */
  all_missing: boolean;
}

export async function listMissingRoms(systemPath: string): Promise<MissingRoms> {
  return invoke("list_missing_roms", { systemPath });
}

/** Add ROMs to the gamelist with the given names; returns how many were added */
export async function addNewRoms(systemPath: string, entries: NewRomEntry[]): Promise<number> {
  return invoke("add_new_roms", { systemPath, entries });
}

export async function findUnusedMedia(
  systemPath: string
): Promise<UnusedMediaFile[]> {
  return invoke("find_unused_media", { systemPath });
}

/** Media files named after one game's ROM that the game doesn't use. */
export async function findGameLeftoverMedia(
  systemPath: string,
  gamePath: string
): Promise<UnusedMediaFile[]> {
  return invoke("find_game_leftover_media", { systemPath, gamePath });
}

/** Same file in more than one media type (optionally only one game) */
export async function findRepeatedMedia(systemPath: string, gamePath?: string): Promise<RepeatedMedia[]> {
  return invoke("find_repeated_media", { systemPath, gamePath });
}

/** Gamelist media fields pointing to missing files (Classic; optionally only one game) */
export async function findBrokenMediaRefs(systemPath: string, gamePath?: string): Promise<BrokenMediaRef[]> {
  return invoke("find_broken_media_refs", { systemPath, gamePath });
}

/** Remove those fields from the gamelist (one save, with backup); returns how many */
export async function clearMediaRefs(systemPath: string, refs: BrokenMediaRef[]): Promise<number> {
  return invoke("clear_media_refs", { systemPath, refs });
}

/** Send media files of the system to the trash (Classic: clears the gamelist fields using them) */
export async function trashMedia(systemPath: string, paths: string[]): Promise<TrashResult> {
  return invoke("trash_media", { systemPath, paths });
}

/** With gamePath, only that game is proposed (game detail panel). */
export async function listRenameCandidates(
  systemPath: string,
  source: RenameSource = "title",
  gamePath?: string
): Promise<RenameEntry[]> {
  return invoke("list_rename_candidates", { systemPath, source, gamePath });
}

/** null = the name can be used; otherwise "invalid" or "exists". */
export async function checkRenameTarget(
  systemPath: string,
  gamePath: string,
  newName: string
): Promise<"invalid" | "exists" | null> {
  return invoke("check_rename_target", { systemPath, gamePath, newName });
}

export async function renameRoms(
  systemPath: string,
  renames: RenameRequest[]
): Promise<RenameResult> {
  return invoke("rename_roms", { systemPath, renames });
}

// ROM name cleanup rules
export async function getDefaultRenameRules(): Promise<RenameRule[]> {
  return invoke("get_default_rename_rules");
}

export async function testRenameRules(name: string, rules: RenameRule[]): Promise<RuleTestResult> {
  return invoke("test_rename_rules", { name, rules });
}

export async function importRenameRules(filePath: string): Promise<RenameRule[]> {
  return invoke("import_rename_rules", { filePath });
}

/** Writes the enabled rules in the original rules.json format */
export async function exportRenameRules(rules: RenameRule[], destPath: string): Promise<void> {
  return invoke("export_rename_rules", { rules, destPath });
}

export async function listBackups(
  systemPath: string
): Promise<BackupEntry[]> {
  return invoke("list_backups", { systemPath });
}

export async function restoreBackup(
  systemPath: string,
  backupFilename: string
): Promise<void> {
  return invoke("restore_backup", { systemPath, backupFilename });
}

export async function deleteBackup(
  systemPath: string,
  backupFilename: string
): Promise<void> {
  return invoke("delete_backup", { systemPath, backupFilename });
}

/** Copies a picked file as a game's media using the active profile's naming.
 * Returns the value to store in the game field. */
export async function setGameMedia(
  systemPath: string,
  gamePath: string,
  field: string,
  source: string
): Promise<string> {
  return invoke("set_game_media", { systemPath, gamePath, field, source });
}

export async function scanGameMedia(
  systemPath: string,
  gamePath: string
): Promise<Record<string, string>> {
  return invoke("scan_game_media", { systemPath, gamePath });
}

export async function exportGamelist(
  systemPath: string,
  format: string,
  outputPath: string
): Promise<string> {
  return invoke("export_gamelist", { systemPath, format, outputPath });
}

export async function switchImageType(
  systemPath: string,
  primarySubfolder: string,
  fallbackSubfolders: string[]
): Promise<GameList> {
  return invoke("switch_image_type", {
    systemPath,
    primarySubfolder,
    fallbackSubfolders,
  });
}

// Where a system's data lives with the active profile (shown in the system summary)
export interface SystemLocations {
  structure: "classic" | "esde";
  rom_dir: string;
  gamelist_path: string;
  gamelist_exists: boolean;
  backups_dir: string;
  media_dir: string;
  media_folders: { folder: string; path: string; files: number }[];
  collections_dir: string;
}

export async function systemLocations(systemPath: string): Promise<SystemLocations> {
  return invoke("system_locations", { systemPath });
}

// DAT files (per system)
export interface DatSummary {
  name: string;
  games: number;
  clones: number;
  bios: number;
  /** Saved with ROM CRCs (DATs associated before can't be checked by content: pick them again) */
  has_crcs: boolean;
}

export interface DatMark {
  game_path: string;
  /** "clone" | "bios" | "device" */
  kind: string;
  parent: string | null;
}

export interface DatReport {
  summary: DatSummary;
  missing: { name: string; description: string; cloneof: string | null; year: string; manufacturer: string }[];
  unknown_files: string[];
  marks: DatMark[];
  names: { game_path: string; current: string | null; proposed: string }[];
  /** Files recognised by content (CRC already in the cache) */
  content_matched: number;
}

export async function setSystemDat(systemPath: string, datPath: string): Promise<DatSummary> {
  return invoke("set_system_dat", { systemPath, datPath });
}

export async function clearSystemDat(systemPath: string): Promise<void> {
  return invoke("clear_system_dat", { systemPath });
}

/** null when the system has no DAT */
/** A source's placeholder: the same file (byte for byte) in many different games of the system */
export interface PlaceholderGroup {
  md5: string;
  size: number;
  fields: string[];
  games: number;
  files: { path: string; field: string; game_path: string; game_name: string }[];
}

/** A media file that is empty, not what its extension says, or cut short */
export interface DamagedMedia {
  path: string;
  field: string;
  game_path: string;
  game_name: string;
  damage: "empty" | "not_media" | "truncated";
  size: number;
  /** Scraper media types that fill this field (to download it again) */
  scraper_types: string[];
}

export async function findDamagedMedia(systemPath: string, gamePath?: string): Promise<DamagedMedia[]> {
  return invoke("find_damaged_media", { systemPath, gamePath });
}

export async function findPlaceholderMedia(systemPath: string): Promise<PlaceholderGroup[]> {
  return invoke("find_placeholder_media", { systemPath });
}

/** Add placeholders to the blocklist: the scraper won't save them again. Returns how many were new */
export async function learnPlaceholders(entries: { size: number; md5: string; field: string }[]): Promise<number> {
  return invoke("learn_placeholders", { entries });
}

export async function placeholderCount(): Promise<number> {
  return invoke("placeholder_count");
}

export async function forgetPlaceholders(): Promise<void> {
  return invoke("forget_placeholders");
}

/** One version of a game in "Best version" (tags read from the file name) */
export interface GameVersion {
  game_path: string;
  name: string;
  file: string;
  tags: { regions: string[]; languages: string[]; revision: number; unwanted: boolean; verified: boolean };
  chosen: boolean;
}

/** Versions of one game with the proposed one; reason: not_unwanted | region | language | revision | verified | shortest */
export interface VersionGroup {
  title: string;
  reason: string;
  versions: GameVersion[];
}

/** Groups of versions of the same game (by DAT if any, else by name) with the preferred one proposed */
export async function bestVersions(systemPath: string): Promise<VersionGroup[]> {
  return invoke("best_versions", { systemPath });
}

/** Translate texts with DeepL to the app language ("es" | "en"); text null = already in that language */
export async function translateTexts(texts: string[], target: string): Promise<{ text: string | null }[]> {
  return invoke("translate_texts", { texts, target });
}

/** ROM subfolders of a system (relative, with "/"), for "Move to..." */
export async function listSubfolders(systemPath: string): Promise<string[]> {
  return invoke("list_subfolders", { systemPath });
}

/** Move games to another system or subfolder with their files, media, gamelist entry and collections */
export async function moveGames(
  systemPath: string,
  gamePaths: string[],
  destSystemPath: string,
  destSubfolder?: string
): Promise<{ moved: number; skipped: string[] }> {
  return invoke("move_games", { systemPath, gamePaths, destSystemPath, destSubfolder });
}

export async function datReport(systemPath: string): Promise<DatReport | null> {
  return invoke("dat_report", { systemPath });
}

/** "Check by content": computes the missing ROM CRCs (emits dat-crc-progress); returns the files the DAT recognises */
export async function datComputeCrcs(systemPath: string): Promise<number> {
  return invoke("dat_compute_crcs", { systemPath });
}

// Duplicates
export interface DuplicateGroup {
  /** "screenscraper_id" | "hash" | "similar_name" */
  reason: string;
  /** Extra detail (the ScreenScraper ID) */
  detail: string | null;
  games: Game[];
}

export async function findDuplicates(systemPath: string): Promise<DuplicateGroup[]> {
  return invoke("find_duplicates", { systemPath });
}

export interface RemoveResult {
  removed: number;
  files: string[];
  failed: string[];
}

/** Remove games from the gamelist and send their ROM + media to the trash (dryRun: only list) */
export async function removeGamesWithFiles(systemPath: string, gamePaths: string[], dryRun: boolean): Promise<RemoveResult> {
  return invoke("remove_games_with_files", { systemPath, gamePaths, dryRun });
}

// Collections
export interface DiscGroup {
  base_name: string;
  files: string[];
}

export async function detectMultidiscGames(systemPath: string): Promise<DiscGroup[]> {
  return invoke("detect_multidisc_games", { systemPath });
}

export async function generateM3uPlaylists(systemPath: string, groups: DiscGroup[]): Promise<string[]> {
  return invoke("generate_m3u_playlists", { systemPath, groups });
}

/** Writes the collection where the active profile keeps them (system folder or ES-DE collections/). */
export async function generateCollection(systemPath: string, name: string, gamePaths: string[]): Promise<string> {
  return invoke("generate_collection", { systemPath, name, gamePaths });
}

// Media normalizer (Classic profiles)
export async function planMediaNormalization(systemPath: string, gamePath?: string): Promise<NormalizeItem[]> {
  return invoke("plan_media_normalization", { systemPath, gamePath });
}

export async function applyMediaNormalization(systemPath: string, items: NormalizeItem[]): Promise<NormalizeResult> {
  return invoke("apply_media_normalization", { systemPath, items });
}

// Structure migration (Classic <-> ES-DE). The target always keeps the source's ROM folder.
/** systems: only these systems (omit for all) */
export async function planMigration(
  source: Profile,
  target: Profile,
  mode: MigrationMode,
  systems?: string[]
): Promise<MigrationReport> {
  return invoke("plan_migration", { source, target, mode, systems });
}

export async function runMigration(
  source: Profile,
  target: Profile,
  mode: MigrationMode,
  systems?: string[]
): Promise<MigrationReport> {
  return invoke("run_migration", { source, target, mode, systems });
}

export async function detectPendingStructure(): Promise<PendingStructure | null> {
  return invoke("detect_pending_structure");
}

// Folders
export async function addFolder(systemPath: string, folder: Folder): Promise<GameList> {
  return invoke("add_folder", { systemPath, folder });
}

export async function removeFolder(systemPath: string, folderPath: string): Promise<GameList> {
  return invoke("remove_folder", { systemPath, folderPath });
}
