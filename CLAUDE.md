# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

meROMS is a desktop ROM manager for EmulationStation gamelist.xml files. Built with Tauri v2, React 19, TypeScript, and Tailwind CSS. It lets users browse ROM systems, view/edit game metadata, manage images, and add/remove game entries.

## Development Commands

- `pnpm tauri dev` — Start full development mode (Vite + Rust backend + Tauri window)
- `pnpm tauri build` — Production build (creates platform-specific installers in `src-tauri/target/release/bundle/`)
- `pnpm dev` — Frontend-only dev server (Vite on port 1420, no Tauri window)
- `pnpm build` — Frontend-only build (`tsc && vite build`, outputs to `dist/`)

## Architecture

### Rust Backend (`src-tauri/src/`)

- `lib.rs` — App builder: registers plugins (dialog, fs, opener) and all Tauri commands
- `main.rs` — Desktop entry point
- `models/` — Data structs: `Game`, `GameList`, `AppSettings`, `SystemInfo` (all serde Serialize/Deserialize)
- `parser/mod.rs` — XML parsing via `quick-xml`: `parse_gamelist()` and `serialize_gamelist()`
- `commands/` — Tauri command handlers:
  - `settings.rs` — `get_settings`, `save_settings` (persists to app data dir as JSON)
  - `systems.rs` — `scan_systems` (reads ROM root subdirs, checks for gamelist.xml)
  - `gamelist.rs` — `load_gamelist`, `save_gamelist`, `add_game`, `remove_game`
  - `images.rs` — `copy_image_to_system` (copies image file into system's images dir)

### React Frontend (`src/`)

- `store/useAppStore.ts` — Zustand store: systems, games, selection, view mode, settings
- `commands/index.ts` — Typed wrappers around `invoke()` for all Rust commands
- `types/index.ts` — TypeScript interfaces mirroring Rust structs
- `components/layout/` — `MainLayout`, `Sidebar` (system list), `Header` (search + view toggle)
- `components/games/` — `GameListView` (switches table/grid), `GameTable` (@tanstack/react-table), `GameGrid`/`GameCard`
- `components/detail/` — `GameDetailPanel` (slide-over editor), `GameForm`, `ImageManager`/`ImagePreview`
- `components/settings/` — `SettingsView` (ROM root folder picker)
- `components/common/` — `ViewToggle`, `SearchBar`, `ConfirmDialog`, `EmptyState`
- `hooks/` — `useGames` (loads games on system selection), `useDebounce`
- `utils/images.ts` — Resolves relative gamelist.xml image paths via `convertFileSrc()`
- `utils/format.ts` — EmulationStation date format (YYYYMMDDTHHMMSS) ↔ readable date

### Data Flow

1. User configures ROM root folder in Settings → saved to app data dir
2. `scan_systems` reads subdirectories → Sidebar populates
3. User clicks system → `load_gamelist` parses gamelist.xml → table/grid renders
4. User clicks game → detail panel opens with form + image previews
5. Edits save via `save_gamelist` → XML written back to disk

## Frontend ↔ Backend IPC

- Frontend calls Rust: `import { invoke } from "@tauri-apps/api/core"` → `invoke("command_name", { args })`
- Rust defines commands: `#[tauri::command] fn command_name(arg: Type) -> ReturnType`
- Commands must be registered in `lib.rs`: `.invoke_handler(tauri::generate_handler![...])`
- Tauri plugin permissions: `src-tauri/capabilities/default.json`

## Key Dependencies

- **Rust**: quick-xml (XML serde), tauri-plugin-dialog, tauri-plugin-fs
- **JS**: zustand (state), @tanstack/react-table (table view), tailwindcss, @tauri-apps/plugin-dialog, @tauri-apps/plugin-fs

## Adding Tauri Plugins

```bash
pnpm add @tauri-apps/plugin-<name>
cargo add tauri-plugin-<name> --manifest-path src-tauri/Cargo.toml
```
Register in `lib.rs` with `.plugin(tauri_plugin_<name>::init())` and add permissions to `capabilities/default.json`.
