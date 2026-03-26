import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, BackupEntry, Folder, Game, GameList, RenameEntry, RenameRequest, ScrapeResult, SystemInfo, UnusedMediaFile } from "../types";

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

export async function deleteFiles(paths: string[]): Promise<number> {
  return invoke("delete_files", { paths });
}

export async function listRenameCandidates(
  systemPath: string
): Promise<RenameEntry[]> {
  return invoke("list_rename_candidates", { systemPath });
}

export async function renameRoms(
  systemPath: string,
  renames: RenameRequest[]
): Promise<GameList> {
  return invoke("rename_roms", { systemPath, renames });
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

export async function copyImageToSystem(
  source: string,
  systemPath: string,
  subfolder: string,
  filename: string
): Promise<string> {
  return invoke("copy_image_to_system", {
    source,
    systemPath,
    subfolder,
    filename,
  });
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

export async function generateCollection(name: string, gamePaths: string[], outputDir: string): Promise<string> {
  return invoke("generate_collection", { name, gamePaths, outputDir });
}

// Folders
export async function addFolder(systemPath: string, folder: Folder): Promise<GameList> {
  return invoke("add_folder", { systemPath, folder });
}

export async function removeFolder(systemPath: string, folderPath: string): Promise<GameList> {
  return invoke("remove_folder", { systemPath, folderPath });
}
