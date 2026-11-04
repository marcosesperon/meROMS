# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

meROMS is a desktop ROM manager for EmulationStation gamelist.xml files. Built with Tauri v2, React 19, TypeScript, and Tailwind CSS. It lets users browse ROM systems, view/edit game metadata, manage media, and add/remove game entries. It supports two folder structures through **profiles** — Classic EmulationStation and ES-DE — and can migrate a library between them. It also scrapes metadata and media from online databases and offers maintenance tools (duplicates, unused media, media normalizer, ROM renaming with cleanup rules, backups, multi-disc playlists, collections). The UI is available in English and Spanish.

## Features

- **Profiles** — several saved libraries (ROM folder + structure: Classic or ES-DE), switchable from the sidebar; ES-DE paths read from `es_settings.xml`
- **Browse & edit** — systems sidebar with logos, table/grid views, search + filters, slide-over metadata editor
- **Media** — unified naming `<ES-DE folder>/<ROM name>.<ext>` in both structures; preview and set images/videos per game
- **Scraping** — ScreenScraper with TheGamesDB fallback (batch), single game or whole system with progress
- **Gamelist generation** — build a gamelist.xml from the ROM files in a system folder; manage `<folder>` entries
- **Cleanup** — find unused media files and duplicate games, then delete them
- **Media normalizer** — Classic: move legacy-named media (suffixes, `images/`…) to the unified naming (optional before migrating: the migrator does it on the fly); ES-DE: recover Classic-structure media left in the ROM folder into the ES-DE media folder
- **Migration** — Classic ↔ ES-DE, copy or move; gamelists, media and collections (ROMs never touched); systems found in the other structure are offered for migration when entering a profile and per system
- **Per-game tools** — Rename, Normalize media and Clean up leftover media from the game detail panel
- **Rename** — batch-rename ROM files from the gamelist title or with regex cleanup rules (ES-DE-Roms-Renamer style); updates media, gamelist, collections and `.m3u`
- **Backups** — automatic gamelist.xml backup before every save; list/restore/delete
- **Collections** — detect multi-disc games and generate `.m3u` playlists; generate custom collections
- **Export** — export a gamelist; export/import app settings

## Development Commands

- `pnpm tauri dev` — Start full development mode (Vite + Rust backend + Tauri window)
- `pnpm tauri build` — Production build (creates platform-specific installers in `src-tauri/target/release/bundle/`)
- `pnpm dev` — Frontend-only dev server (Vite on port 1420, no Tauri window)
- `pnpm build` — Frontend-only build (`tsc && vite build`, outputs to `dist/`)
- `cargo test --manifest-path src-tauri/Cargo.toml` — Rust unit tests (use temp dirs via `tempfile`; no real ROMs needed)

## Architecture

### Structures and the layout layer

Every path decision goes through `layout::Layout` (`Classic` | `Esde`). Never build gamelist/media/backup/collection paths by hand in commands:

| | Classic | ES-DE |
|---|---|---|
| gamelist | `<ROMs>/<sys>/gamelist.xml` | `<ES-DE>/gamelists/<sys>/gamelist.xml` |
| media | `<ROMs>/<sys>/<folder>/<ROM stem>.<ext>` | `<media>/<sys>/<folder>/<ROM stem>.<ext>` |
| media paths in XML | yes | no (found by ROM name) |
| backups | next to the gamelist in `.backups/` (max 20) | same |
| collections | `<ROMs>/<sys>/custom-*.cfg`, `./x` entries | `<ES-DE>/collections/custom-*.cfg`, `%ROMPATH%/<sys>/x` entries (merged) |

- Commands still receive the system's ROM folder (`system_path`); `commands::system_context(&app, &system_path)` returns the active profile's `Layout` + `SystemRef`. No active profile ⇒ Classic.
- Always read/write gamelists through `commands::gamelist::read_gamelist` / `write_gamelist`. In ES-DE, `read_gamelist` fills media fields in memory with absolute paths (`media::hydrate_gamelist`) and `write_gamelist` strips them, so the UI works the same for both structures.
- The parser preserves unknown `<game>` children/attributes and unknown root elements (e.g. ES-DE `<alternativeEmulator>`).

### Rust Backend (`src-tauri/src/`)

- `lib.rs` — App builder: registers plugins (dialog, fs, opener), the custom `media://` URI protocol (serves local images/videos from absolute paths with the right MIME type) and all Tauri commands
- `main.rs` — Desktop entry point
- `layout/` — `Layout`, `SystemRef`, `EsdePaths`, media stem rules, `is_own_folder` (ES-DE's own folders are never systems when ES-DE lives inside the ROM folder); `esde_settings.rs` reads `ROMDirectory`/`MediaDirectory`/`LegacyGamelistFileLocation` from `es_settings.xml`
- `media.rs` — Single table of media kinds (ES-DE folder ↔ gamelist field ↔ scraper types), `MediaIndex` (one directory walk per system), hydration, scraped-media targets, rename/move/copy helpers (never overwrite)
- `rename_rules/` — Regex cleanup rules engine (`CompiledRules`) and built-in `default_rules.json` (from ES-DE-Roms-Renamer, MIT; "(Disc x) → .partx" disabled)
- `models/` — Data structs (all serde Serialize/Deserialize):
  - `game.rs`, `folder.rs`, `gamelist.rs`, `provider.rs` — `Game`, `Folder`, `GameList`, `Provider` (gamelist.xml content)
  - `profile.rs` — `Profile`, `StructureKind` (`classic` | `esde`)
  - `settings.rs` — `AppSettings` (profiles + active profile, scraper credentials, media field mapping, rename rules…), `MediaPathConfig` (camelCase on the wire), `SystemConfig`; `migrate_legacy()` turns the old `rom_root_path` into a default Classic profile
  - `system.rs` — `SystemInfo` plus static per-system tables: ROM extensions, full names, ScreenScraper/scraping platform IDs
  - `scraper.rs` — `ScrapeResult`, `ScrapeProgress`
- `parser/mod.rs` — XML parsing via `quick-xml`: `parse_gamelist()` and `serialize_gamelist()`
- `commands/` — Tauri command handlers (most take `app: AppHandle`, injected by Tauri):
  - `settings.rs` — `get_settings` (migrates legacy settings), `save_settings`, `export_settings`, `import_settings`
  - `profiles.rs` — `detect_esde_paths`, `detect_pending_structure` (systems of the active profile whose data is only in the other structure + the source profile to migrate from: same-folder Classic, saved ES-DE profiles or `~/ES-DE`)
  - `systems.rs` — `scan_systems` (reads ROM root subdirs, checks for the gamelist where the layout keeps it)
  - `gamelist.rs` — `load_gamelist`, `save_gamelist`, `add_game`, `remove_game`, `generate_gamelist`, `add_folder`, `remove_folder`; `read_gamelist`/`write_gamelist`/`write_gamelist_with_original` helpers
  - `images.rs` — `set_game_media` (copies a picked file with unified naming), `scan_game_media`, `switch_image_type` (Classic only)
  - `scraper.rs` — `scrape_game` (ScreenScraper if configured, else TheGamesDB), `scrape_system` (per game: ScreenScraper, then TheGamesDB; emits `scrape-progress`)
  - `scraper_tgdb.rs` — TheGamesDB client helpers (no commands of its own)
  - `cleanup.rs` — `find_unused_media` (Classic: not referenced by any media field; ES-DE: no ROM with that name), `find_game_leftover_media` (one game: files named after its ROM it doesn't use), `delete_files`
  - `duplicates.rs` — `find_duplicates`
  - `normalize.rs` — `plan_media_normalization` (optional `game_path`), `apply_media_normalization` (both structures; ES-DE moves Classic leftovers from the ROM folder into the media folder and doesn't rewrite the gamelist; a file found by name goes to one destination only)
  - `migrate.rs` — `plan_migration`, `run_migration` (take the source `Profile` itself — it may be a detected, unsaved one — and an optional `systems` subset; Classic → ES-DE renames legacy-named media on the fly using `normalize::plan`; same code path with `dry_run`; async + `spawn_blocking`; emits `migration-progress`)
  - `rename.rs` — `list_rename_candidates` (source `title` | `rules`, optional `game_path`, flags `duplicate`/`exists` conflicts), `check_rename_target` (a hand-typed name: `invalid`/`exists`), `rename_roms` (returns renamed/skipped; updates collections and `.m3u`)
  - `rules.rs` — `get_default_rename_rules`, `test_rename_rules`, `import_rename_rules`, `export_rename_rules`
  - `backup.rs` — `list_backups`, `restore_backup`, `delete_backup`
  - `export.rs` — `export_gamelist`
  - `collections.rs` — `detect_multidisc_games`, `generate_m3u_playlists`, `generate_collection`

### React Frontend (`src/`)

- `store/useAppStore.ts` — Zustand store: settings (+ `setActiveProfile`), systems, games, selection, view mode, filters; switching profile resets selection and rescans; every scan runs `checkPendingStructure` (`pendingStructure`, prompt once per profile activation unless `skip_structure_prompt`, `migrationPreset` for the pre-filled migration window)
- `commands/index.ts` — Typed wrappers around `invoke()` for all Rust commands
- `types/index.ts` — TypeScript interfaces mirroring Rust structs; `MEDIA_TYPE_FOLDERS` mirrors `media::kind_for_scraper_type`
- `i18n/` — `en.ts` (keep it ASCII-only), `es.ts` translation dictionaries; `index.ts` exposes `useI18nStore` (locale) and `useTranslation()`
- `App.tsx` — Root: renders the per-system tool toolbar and owns the open/close state of every tool modal and the settings view
- `components/layout/` — `MainLayout`, `Sidebar` (profile selector, "Migrate structure…", system list with an amber dot on systems pending migration), `Header` (search + filters toggle + view toggle)
- `components/games/` — `GameListView` (switches table/grid), `GameTable` (@tanstack/react-table), `GameGrid`/`GameCard`
- `components/detail/` — `GameDetailPanel` (slide-over editor; per-game tools Rename/Normalize/Clean up behind a "save first" guard), `GameForm` ("Rename..." next to the ROM path), `ImageManager`/`ImagePreview` (Normalize/Clean up in the media header)
- `components/settings/` — `SettingsView` with tabs: `ProfilesTab`, General (scraper credentials, media to download, grid media), `MediaConfigTab` (fixed folders, gamelist field per media type), `RenameRulesTab` (rules + live test + rules.json import/export), `SystemsConfigTab`
- `components/common/` — `ViewToggle`, `SearchBar`, `FilterBar`, `ConfirmDialog`, `EmptyState`, `Icons`, `SystemLogo` (SVGs in `assets/logos/`)
  - `StructurePromptModal` — "systems in the other structure, migrate?" prompt shown when entering a profile
  - Tool modals: `ScrapeProgressModal`, `DuplicatesModal`, `UnusedMediaModal`, `RenameModal`, `RenameGameModal` (one ROM), `NormalizeMediaModal`, `MigrationModal` (normal, or pre-filled with `preset` = detected source + systems), `BackupsModal`, `CollectionsModal`, `ExportModal`; `UnusedMediaModal` and `NormalizeMediaModal` take an optional `gamePath` to work on one game
- `hooks/` — `useGames` (loads games on system selection), `useFilteredGames` (search + filters applied to the store's games), `useDebounce`
- `utils/profiles.ts` — `getActiveProfile`, `isProfileComplete`, `profileLocationKey`, `createProfile`
- `utils/images.ts` — Resolves gamelist media paths (relative or absolute) to `media://localhost/...` URLs
- `utils/format.ts` — EmulationStation date format (YYYYMMDDTHHMMSS) ↔ readable date
- `utils/cartridgeSizes.ts` — Box art / cartridge aspect ratios per system
- `utils/flags.ts` — Language/region codes → flag emoji

### Data Flow

1. User creates/selects a profile in Settings → Profiles (ES-DE folder auto-fills ROM/media folders) → saved to app data dir
2. `scan_systems` reads the active profile's ROM folder subdirectories → Sidebar populates
3. User clicks system → `load_gamelist` reads the gamelist where the layout keeps it (ES-DE: media filled by ROM name) → table/grid renders (filtered via `useFilteredGames`)
4. User clicks game → detail panel opens with form + media previews (served through `media://`)
5. Edits save via `save_gamelist` → previous gamelist.xml copied to `.backups/` → XML written back (ES-DE: without media paths)
6. Scraping: media saved with unified naming; `scrape_system` emits `scrape-progress` events consumed by `ScrapeProgressModal`
7. Migration: `plan_migration` (preview) → `run_migration` (progress events) → optionally create & activate the destination profile

## Frontend ↔ Backend IPC

- Frontend calls Rust: `import { invoke } from "@tauri-apps/api/core"` → `invoke("command_name", { args })` (camelCase args map to snake_case Rust params)
- Rust defines commands: `#[tauri::command] fn command_name(arg: Type) -> ReturnType`; long-running work should be `async` + `tauri::async_runtime::spawn_blocking`
- Commands must be registered in `lib.rs`: `.invoke_handler(tauri::generate_handler![...])`
- Tauri plugin permissions: `src-tauri/capabilities/default.json`

## Key Dependencies

- **Rust**: quick-xml (XML serde), regex (rename rules), reqwest + tokio (scraper HTTP), md-5 (ROM hashing for ScreenScraper), chrono (backup timestamps), percent-encoding/urlencoding, tauri-plugin-dialog, tauri-plugin-fs, tauri-plugin-opener; dev: tempfile
- **JS**: zustand (state + i18n), @tanstack/react-table (table view), tailwindcss, @tauri-apps/plugin-dialog, @tauri-apps/plugin-fs, @tauri-apps/plugin-opener

## Adding Tauri Plugins

```bash
pnpm add @tauri-apps/plugin-<name>
cargo add tauri-plugin-<name> --manifest-path src-tauri/Cargo.toml
```
Register in `lib.rs` with `.plugin(tauri_plugin_<name>::init())` and add permissions to `capabilities/default.json`.
