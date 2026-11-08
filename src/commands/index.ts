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

export async function scanSystems(rootPath: string, task?: string): Promise<SystemInfo[]> {
  return invoke("scan_systems", { rootPath, task });
}

export async function loadGamelist(systemPath: string, task?: string): Promise<GameList> {
  return invoke("load_gamelist", { systemPath, task });
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
  systemPath: string,
  task?: string
): Promise<GameList> {
  return invoke("generate_gamelist", { systemPath, task });
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
export async function swapMedia(systemPath: string, gamePaths: string[], fieldA: string, fieldB: string, task?: string): Promise<SwapResult> {
  return invoke("swap_media", { systemPath, gamePaths, fieldA, fieldB, task });
}

/** Games whose cover and screenshot look swapped */
export async function detectSwappedMedia(systemPath: string, task?: string): Promise<DetectResult> {
  return invoke("detect_swapped_media", { systemPath, task });
}

/** ROM files in the system folder that the gamelist doesn't have */
export async function listNewRoms(systemPath: string, task?: string): Promise<NewRom[]> {
  return invoke("list_new_roms", { systemPath, task });
}

/** Move files of the system folder to the system trash (only paths inside that folder) */
export async function trashFiles(systemPath: string, paths: string[], task?: string): Promise<TrashResult> {
  return invoke("trash_files", { systemPath, paths, task });
}

/** Games of the gamelist whose ROM file/folder no longer exists */
export interface MissingRoms {
  games: { path: string; name: string }[];
  /** Every ROM is missing: the folder or drive is probably not connected */
  all_missing: boolean;
}

export async function listMissingRoms(systemPath: string, task?: string): Promise<MissingRoms> {
  return invoke("list_missing_roms", { systemPath, task });
}

/** Add ROMs to the gamelist with the given names; returns how many were added */
export async function addNewRoms(systemPath: string, entries: NewRomEntry[], task?: string): Promise<number> {
  return invoke("add_new_roms", { systemPath, entries, task });
}

export async function findUnusedMedia(
  systemPath: string,
  task?: string
): Promise<UnusedMediaFile[]> {
  return invoke("find_unused_media", { systemPath, task });
}

/** Media files named after one game's ROM that the game doesn't use. */
export async function findGameLeftoverMedia(
  systemPath: string,
  gamePath: string
): Promise<UnusedMediaFile[]> {
  return invoke("find_game_leftover_media", { systemPath, gamePath });
}

/** Same file in more than one media type (optionally only one game) */
export async function findRepeatedMedia(systemPath: string, gamePath?: string, task?: string): Promise<RepeatedMedia[]> {
  return invoke("find_repeated_media", { systemPath, gamePath, task });
}

/** Gamelist media fields pointing to missing files (Classic; optionally only one game) */
export async function findBrokenMediaRefs(systemPath: string, gamePath?: string, task?: string): Promise<BrokenMediaRef[]> {
  return invoke("find_broken_media_refs", { systemPath, gamePath, task });
}

/** Remove those fields from the gamelist (one save, with backup); returns how many */
export async function clearMediaRefs(systemPath: string, refs: BrokenMediaRef[]): Promise<number> {
  return invoke("clear_media_refs", { systemPath, refs });
}

/** Send media files of the system to the trash (Classic: clears the gamelist fields using them) */
export async function trashMedia(systemPath: string, paths: string[], task?: string): Promise<TrashResult> {
  return invoke("trash_media", { systemPath, paths, task });
}

/** With gamePath, only that game is proposed (game detail panel). */
export async function listRenameCandidates(
  systemPath: string,
  source: RenameSource = "title",
  gamePath?: string,
  task?: string
): Promise<RenameEntry[]> {
  return invoke("list_rename_candidates", { systemPath, source, gamePath, task });
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
  renames: RenameRequest[],
  task?: string
): Promise<RenameResult> {
  return invoke("rename_roms", { systemPath, renames, task });
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

export async function findDamagedMedia(systemPath: string, gamePath?: string, task?: string): Promise<DamagedMedia[]> {
  return invoke("find_damaged_media", { systemPath, gamePath, task });
}

export async function findPlaceholderMedia(systemPath: string, task?: string): Promise<PlaceholderGroup[]> {
  return invoke("find_placeholder_media", { systemPath, task });
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

/** "Export to device": what is exported and where */
export interface ExportOptions {
  dest: string;
  /** ROM folders of the systems */
  systems: string[];
  games: "all" | "favorites" | "visible";
  /** Media type folders to copy (covers, videos...) */
  media: string[];
  remove_extra: boolean;
}

export interface ExportPreview {
  copy_files: number;
  copy_bytes: number;
  up_to_date: number;
  remove_files: number;
  remove_sample: string[];
  games: number;
}

export interface DeviceChange {
  system_path: string;
  system: string;
  game_path: string;
  name: string;
  field: "favorite" | "hidden" | "playcount" | "lastplayed" | "gametime";
  from: string | null;
  to: string | null;
}

export async function planExport(options: ExportOptions): Promise<ExportPreview> {
  return invoke("plan_export", { options });
}

/** Copies the selection (emits export-progress); cancelExport stops it between files */
export async function runExport(options: ExportOptions): Promise<{ copied: number; removed: number; failed: string[]; stopped: boolean }> {
  return invoke("run_export", { options });
}

export async function cancelExport(): Promise<void> {
  return invoke("cancel_export");
}

/** What changed in the destination (favourite, hidden, play stats) */
export async function pullChanges(dest: string, systems: string[]): Promise<DeviceChange[]> {
  return invoke("pull_changes", { dest, systems });
}

export async function applyPull(changes: DeviceChange[]): Promise<number> {
  return invoke("apply_pull", { changes });
}

/** "Compare with another folder": one side of a game (size of all its files, newest date) */
export interface CompareSide {
  size: number;
  modified: number;
  files: number;
}

export interface CompareGame {
  path: string;
  name: string;
  here: CompareSide | null;
  there: CompareSide | null;
}

export interface SystemCompare {
  system: string;
  only_here: CompareGame[];
  only_there: CompareGame[];
  different: CompareGame[];
  same: number;
}

/** A game copied to the other folder ("there") or into the library ("here") */
export interface CompareAction {
  system: string;
  game_path: string;
  to: "here" | "there";
}

/** Compares the library with `dest` (emits compare-progress; cancelCompare stops it) */
export async function compareFolders(dest: string, checkCrc: boolean): Promise<{ systems: SystemCompare[] }> {
  return invoke("compare_folders", { dest, checkCrc });
}

export async function applyCompare(
  dest: string,
  actions: CompareAction[],
): Promise<{ to_there: number; to_here: number; replaced: number; failed: string[]; stopped: boolean }> {
  return invoke("apply_compare", { dest, actions });
}

export async function cancelCompare(): Promise<void> {
  return invoke("cancel_compare");
}

/** "Convert to CHD": where chdman is (null = not found) */
export async function chdStatus(): Promise<{ path: string | null; version: string | null }> {
  return invoke("chd_status");
}

export interface ChdCandidate {
  path: string;
  name: string;
  /** Discs converted (relative to the system folder) */
  discs: string[];
  size: number;
  /** "exists" = there is already a .chd with that name */
  blocked: "exists" | null;
}

export async function listChdCandidates(systemPath: string, task?: string): Promise<{ supported: boolean; games: ChdCandidate[] }> {
  return invoke("list_chd_candidates", { systemPath, task });
}

export interface ChdResult {
  converted: number;
  skipped: string[];
  failed: string[];
  originals_failed: string[];
  bytes_before: number;
  bytes_after: number;
  stopped: boolean;
}

/** Converts the games (emits chd-progress; cancelChd stops the running chdman) */
export async function convertToChd(
  systemPath: string,
  gamePaths: string[],
  originals: "trash" | "keep" | "subfolder",
  subfolder?: string,
): Promise<ChdResult> {
  return invoke("convert_to_chd", { systemPath, gamePaths, originals, subfolder });
}

export async function cancelChd(): Promise<void> {
  return invoke("cancel_chd");
}

/** BIOS list (libretro System.dat) downloaded in Settings */
export interface BiosListStatus {
  downloaded: boolean;
  downloaded_at: number | null;
  files: number;
}

export interface BiosCheck {
  name: string;
  status: "ok" | "missing" | "mismatch";
  expected_md5: string | null;
  found: string | null;
}

export interface SystemBios {
  system: string;
  section: string;
  ok: number;
  total: number;
  files: BiosCheck[];
}

export async function biosStatus(): Promise<BiosListStatus> {
  return invoke("bios_status");
}

export async function biosDownload(): Promise<BiosListStatus> {
  return invoke("bios_download");
}

export async function biosDelete(): Promise<void> {
  return invoke("bios_delete");
}

/** BIOS folder proposed for a profile without one */
export async function suggestedBiosDir(profile: Profile): Promise<string | null> {
  return invoke("suggested_bios_dir", { profile });
}

/** BIOS of these systems (folder names) in the profile's BIOS folder; null = list not downloaded */
export async function checkBios(systems: string[]): Promise<{ bios_dir: string | null; systems: SystemBios[] } | null> {
  return invoke("check_bios", { systems });
}

/** The last file operation that can be undone (rename | move | normalize | swap), or null */
export interface UndoInfo {
  kind: "rename" | "move" | "normalize" | "swap";
  system: string;
  files: number;
}

export async function undoStatus(): Promise<UndoInfo | null> {
  return invoke("undo_status");
}

/** Undoes the last rename / move / normalize / swap (files, gamelists, collections) */
export async function undoLast(): Promise<{ restored: number; skipped: string[] }> {
  return invoke("undo_last");
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
export async function bestVersions(systemPath: string, task?: string): Promise<VersionGroup[]> {
  return invoke("best_versions", { systemPath, task });
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
  destSubfolder?: string,
  task?: string
): Promise<{ moved: number; skipped: string[] }> {
  return invoke("move_games", { systemPath, gamePaths, destSystemPath, destSubfolder, task });
}

export async function datReport(systemPath: string, task?: string): Promise<DatReport | null> {
  return invoke("dat_report", { systemPath, task });
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

export async function findDuplicates(systemPath: string, task?: string): Promise<DuplicateGroup[]> {
  return invoke("find_duplicates", { systemPath, task });
}

export interface RemoveResult {
  removed: number;
  files: string[];
  failed: string[];
}

/** Remove games from the gamelist and send their ROM + media to the trash (dryRun: only list) */
export async function removeGamesWithFiles(systemPath: string, gamePaths: string[], dryRun: boolean, task?: string): Promise<RemoveResult> {
  return invoke("remove_games_with_files", { systemPath, gamePaths, dryRun, task });
}

// Collections
export interface DiscGroup {
  base_name: string;
  files: string[];
}

export async function detectMultidiscGames(systemPath: string, task?: string): Promise<DiscGroup[]> {
  return invoke("detect_multidisc_games", { systemPath, task });
}

export async function generateM3uPlaylists(systemPath: string, groups: DiscGroup[]): Promise<string[]> {
  return invoke("generate_m3u_playlists", { systemPath, groups });
}

/** Writes the collection where the active profile keeps them (system folder or ES-DE collections/). */
export async function generateCollection(systemPath: string, name: string, gamePaths: string[]): Promise<string> {
  return invoke("generate_collection", { systemPath, name, gamePaths });
}

// Media normalizer (Classic profiles)
export async function planMediaNormalization(systemPath: string, gamePath?: string, task?: string): Promise<NormalizeItem[]> {
  return invoke("plan_media_normalization", { systemPath, gamePath, task });
}

export async function applyMediaNormalization(systemPath: string, items: NormalizeItem[], task?: string): Promise<NormalizeResult> {
  return invoke("apply_media_normalization", { systemPath, items, task });
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

// Game statuses (game_status.rs): kept by meROMS; "completed" also goes to the gamelist
export type PlayStatus = "incomplete" | "finished" | "completed" | "abandoned" | "never";
export const PLAY_STATUSES: PlayStatus[] = ["incomplete", "finished", "completed", "abandoned", "never"];

export interface GameStatus {
  status?: PlayStatus;
  playing?: boolean;
  backlog?: boolean;
}

/** What to change; a missing field is left as it is. `status: null` clears it */
export interface StatusChange {
  status?: PlayStatus | null;
  playing?: boolean;
  backlog?: boolean;
}

export async function getGameStatuses(systemPath: string): Promise<Record<string, GameStatus>> {
  return invoke("get_game_statuses", { systemPath });
}

export async function setGameStatuses(systemPath: string, gamePaths: string[], change: StatusChange): Promise<Record<string, GameStatus>> {
  return invoke("set_game_statuses", { systemPath, gamePaths, change });
}

// Saves (saves.rs): save games and states of a game, in the profile's saves folder
export interface SaveFile {
  path: string;
  name: string;
  kind: "save" | "state";
  /** State slot: 0 = .state, N = .stateN, -1 = .state.auto */
  slot: number | null;
  size: number;
  /** Unix seconds */
  modified: number;
  thumbnail: string | null;
}

export async function suggestedSavesDir(profile: Profile): Promise<string | null> {
  return invoke("suggested_saves_dir", { profile });
}

export async function listSaves(systemPath: string, gamePath: string): Promise<{ dir: string | null; files: SaveFile[] }> {
  return invoke("list_saves", { systemPath, gamePath });
}

export async function trashSave(file: string): Promise<void> {
  return invoke("trash_save", { file });
}

export async function backupSaves(systemPath: string, gamePath: string, dest: string): Promise<string> {
  return invoke("backup_saves", { systemPath, gamePath, dest });
}

export async function importSave(systemPath: string, gamePath: string, file: string): Promise<string> {
  return invoke("import_save", { systemPath, gamePath, file });
}

// Metadata tab (game_meta.rs)
export interface GameFiles {
  name: string;
  exists: boolean;
  is_dir: boolean;
  size: number;
  files: { path: string; size: number }[];
  archive: { name: string; size: number; crc: string }[] | null;
}

export interface GameHashes {
  target: string;
  inside_archive: boolean;
  size: number;
  crc: string;
  md5: string;
  sha1: string;
}

export interface DatMatch {
  dat: string;
  by: "content" | "name" | null;
  name: string | null;
  description: string | null;
  cloneof: string | null;
  bios: boolean;
}

export async function gameFiles(systemPath: string, gamePath: string): Promise<GameFiles> {
  return invoke("game_files", { systemPath, gamePath });
}

export async function gameHashes(systemPath: string, gamePath: string): Promise<GameHashes | null> {
  return invoke("game_hashes", { systemPath, gamePath });
}

export async function datGameInfo(systemPath: string, gamePath: string): Promise<DatMatch | null> {
  return invoke("dat_game_info", { systemPath, gamePath });
}

export async function rawGameFields(systemPath: string, gamePath: string): Promise<[string, string][]> {
  return invoke("raw_game_fields", { systemPath, gamePath });
}

export async function openManual(path: string): Promise<void> {
  return invoke("open_manual", { path });
}

// Global search (search.rs): games of every system of the active profile
export interface SearchHit {
  system_path: string;
  system_name: string;
  system_full_name: string;
  game_path: string;
  name: string;
  cover: string | null;
}

export async function searchGames(systemPaths: string[], query: string): Promise<SearchHit[]> {
  return invoke("search_games", { systemPaths, query });
}

/** Asks a long process to stop (between one game and the next); see `useTask` */
export async function cancelTask(task: string): Promise<void> {
  return invoke("cancel_task", { task });
}
