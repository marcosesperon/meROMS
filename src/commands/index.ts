import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, BackupEntry, EsdeDetectedPaths, Folder, MigrationMode, MigrationReport, NormalizeItem, NormalizeResult, PendingStructure, Profile, RenameResult, RenameRule, RenameSource, RuleTestResult, Game, GameList, RenameEntry, RenameRequest, ScrapeResult, SystemInfo, UnusedMediaFile } from "../types";

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

export async function scrapeGame(
  systemPath: string,
  gamePath: string
): Promise<ScrapeResult> {
  return invoke("scrape_game", { systemPath, gamePath });
}

export async function scrapeSystem(systemPath: string): Promise<void> {
  return invoke("scrape_system", { systemPath });
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

export async function deleteFiles(paths: string[]): Promise<number> {
  return invoke("delete_files", { paths });
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

// Duplicates
export interface DuplicateGroup {
  reason: string;
  games: Game[];
}

export async function findDuplicates(systemPath: string): Promise<DuplicateGroup[]> {
  return invoke("find_duplicates", { systemPath });
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
