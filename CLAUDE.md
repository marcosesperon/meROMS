# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

meROMS is a desktop ROM manager for EmulationStation gamelist.xml files. Built with Tauri v2, React 19, TypeScript, and Tailwind CSS. It lets users browse ROM systems, view/edit game metadata, manage media, and add/remove game entries. It also scrapes metadata and media from online databases and offers maintenance tools (duplicates, unused media, ROM renaming, backups, multi-disc playlists, collections). The UI is available in English and Spanish.

## Features

- **Browse & edit** — systems sidebar with logos, table/grid views, search + filters, slide-over metadata editor
- **Media** — preview and copy images/videos per game; configurable media subfolders/suffixes per media type
- **Scraping** — ScreenScraper (primary) with TheGamesDB fallback, single game or whole system with progress
- **Gamelist generation** — build a gamelist.xml from the ROM files in a system folder; manage `<folder>` entries
- **Cleanup** — find unused media files and duplicate games, then delete them
- **Rename** — batch-rename ROM files and update their references in gamelist.xml (backs up first)
- **Backups** — automatic gamelist.xml backup before every save; list/restore/delete
- **Collections** — detect multi-disc games and generate `.m3u` playlists; generate custom collections
- **Export** — export a gamelist; export/import app settings

## Development Commands

- `pnpm tauri dev` — Start full development mode (Vite + Rust backend + Tauri window)
- `pnpm tauri build` — Production build (creates platform-specific installers in `src-tauri/target/release/bundle/`)
- `pnpm dev` — Frontend-only dev server (Vite on port 1420, no Tauri window)
- `pnpm build` — Frontend-only build (`tsc && vite build`, outputs to `dist/`)

## Architecture

### Rust Backend (`src-tauri/src/`)

- `lib.rs` — App builder: registers plugins (dialog, fs, opener), the custom `media://` URI protocol (serves local images/videos from absolute paths with the right MIME type) and all Tauri commands
- `main.rs` — Desktop entry point
- `models/` — Data structs (all serde Serialize/Deserialize):
  - `game.rs`, `folder.rs`, `gamelist.rs`, `provider.rs` — `Game`, `Folder`, `GameList`, `Provider` (gamelist.xml content)
  - `settings.rs` — `AppSettings` (ROM root, scraper credentials, media config), `MediaPathConfig`, `SystemConfig`
  - `system.rs` — `SystemInfo` plus static per-system tables: ROM extensions, full names, ScreenScraper/scraping platform IDs
  - `scraper.rs` — `ScrapeResult`, `ScrapeProgress`
- `parser/mod.rs` — XML parsing via `quick-xml`: `parse_gamelist()` and `serialize_gamelist()`
- `commands/` — Tauri command handlers:
  - `settings.rs` — `get_settings`, `save_settings` (persists to app data dir as JSON), `export_settings`, `import_settings`
  - `systems.rs` — `scan_systems` (reads ROM root subdirs, checks for gamelist.xml)
  - `gamelist.rs` — `load_gamelist`, `save_gamelist` (backs up first), `add_game`, `remove_game`, `generate_gamelist`, `add_folder`, `remove_folder`
  - `images.rs` — `copy_image_to_system`, `scan_game_media`, `switch_image_type`
  - `scraper.rs` — `scrape_game`, `scrape_system` (ScreenScraper first, falls back to TheGamesDB; batch emits `scrape-progress` events)
  - `scraper_tgdb.rs` — TheGamesDB client helpers (no commands of its own)
  - `cleanup.rs` — `find_unused_media`, `delete_files`
  - `duplicates.rs` — `find_duplicates`
  - `rename.rs` — `list_rename_candidates`, `rename_roms`
  - `backup.rs` — `list_backups`, `restore_backup`, `delete_backup` (backups live in `{system}/.backups/`, max 20)
  - `export.rs` — `export_gamelist`
  - `collections.rs` — `detect_multidisc_games`, `generate_m3u_playlists`, `generate_collection`

### React Frontend (`src/`)

- `store/useAppStore.ts` — Zustand store: systems, games, selection, view mode, filters, settings
- `commands/index.ts` — Typed wrappers around `invoke()` for all Rust commands
- `types/index.ts` — TypeScript interfaces mirroring Rust structs
- `i18n/` — `en.ts`, `es.ts` translation dictionaries; `index.ts` exposes `useI18nStore` (locale) and `useTranslation()`
- `App.tsx` — Root: renders the per-system tool toolbar and owns the open/close state of every tool modal and the settings view
- `components/layout/` — `MainLayout`, `Sidebar` (system list), `Header` (search + filters toggle + view toggle)
- `components/games/` — `GameListView` (switches table/grid), `GameTable` (@tanstack/react-table), `GameGrid`/`GameCard`
- `components/detail/` — `GameDetailPanel` (slide-over editor), `GameForm`, `ImageManager`/`ImagePreview`
- `components/settings/` — `SettingsView` with tabs: General (ROM root, scraper credentials, import/export), `MediaConfigTab`, `SystemsConfigTab`
- `components/common/` — `ViewToggle`, `SearchBar`, `FilterBar`, `ConfirmDialog`, `EmptyState`, `Icons`, `SystemLogo` (SVGs in `assets/logos/`)
  - Tool modals: `ScrapeProgressModal`, `DuplicatesModal`, `UnusedMediaModal`, `RenameModal`, `BackupsModal`, `CollectionsModal`, `ExportModal`
- `hooks/` — `useGames` (loads games on system selection), `useFilteredGames` (search + filters applied to the store's games), `useDebounce`
- `utils/images.ts` — Resolves relative gamelist.xml media paths to `media://localhost/...` URLs
- `utils/format.ts` — EmulationStation date format (YYYYMMDDTHHMMSS) ↔ readable date
- `utils/cartridgeSizes.ts` — Box art / cartridge aspect ratios per system
- `utils/flags.ts` — Language/region codes → flag emoji

### Data Flow

1. User configures ROM root folder (and optional scraper credentials) in Settings → saved to app data dir
2. `scan_systems` reads subdirectories → Sidebar populates
3. User clicks system → `load_gamelist` parses gamelist.xml → table/grid renders (filtered via `useFilteredGames`)
4. User clicks game → detail panel opens with form + media previews (served through `media://`)
5. Edits save via `save_gamelist` → previous gamelist.xml copied to `.backups/` → XML written back to disk
6. Scraping: `scrape_game` returns the updated game; `scrape_system` emits `scrape-progress` events consumed by `ScrapeProgressModal`

## Frontend ↔ Backend IPC

- Frontend calls Rust: `import { invoke } from "@tauri-apps/api/core"` → `invoke("command_name", { args })`
- Rust defines commands: `#[tauri::command] fn command_name(arg: Type) -> ReturnType`
- Commands must be registered in `lib.rs`: `.invoke_handler(tauri::generate_handler![...])`
- Tauri plugin permissions: `src-tauri/capabilities/default.json`

## Key Dependencies

- **Rust**: quick-xml (XML serde), reqwest + tokio (scraper HTTP), md-5 (ROM hashing for ScreenScraper), chrono (backup timestamps), percent-encoding/urlencoding, tauri-plugin-dialog, tauri-plugin-fs, tauri-plugin-opener
- **JS**: zustand (state + i18n), @tanstack/react-table (table view), tailwindcss, @tauri-apps/plugin-dialog, @tauri-apps/plugin-fs, @tauri-apps/plugin-opener

## Adding Tauri Plugins

```bash
pnpm add @tauri-apps/plugin-<name>
cargo add tauri-plugin-<name> --manifest-path src-tauri/Cargo.toml
```
Register in `lib.rs` with `.plugin(tauri_plugin_<name>::init())` and add permissions to `capabilities/default.json`.
