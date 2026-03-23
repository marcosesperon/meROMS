# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

meROMS is a desktop application built with Tauri v2, React 19, and TypeScript. The Rust backend communicates with the React frontend via Tauri's IPC system.

## Development Commands

- `pnpm tauri dev` — Start full development mode (Vite + Rust backend + Tauri window)
- `pnpm tauri build` — Production build (creates platform-specific installers in `src-tauri/target/release/bundle/`)
- `pnpm dev` — Frontend-only dev server (Vite on port 1420, no Tauri window)
- `pnpm build` — Frontend-only build (`tsc && vite build`, outputs to `dist/`)

## Architecture

- **`src/`** — React frontend (TypeScript). Entry: `main.tsx` → `App.tsx`
- **`src-tauri/`** — Rust backend (Tauri core)
  - `src/lib.rs` — Tauri command handlers and app builder. New commands go here with `#[tauri::command]` and must be registered in `invoke_handler`
  - `src/main.rs` — Desktop entry point (calls `lib.rs::run()`)
  - `tauri.conf.json` — App config: window settings, build commands, bundle options, identifier (`com.meroms.app`)
  - `capabilities/default.json` — Security permissions for Tauri API access from frontend
  - `Cargo.toml` — Rust dependencies (add Tauri plugins here)

## Frontend ↔ Backend IPC

- Frontend calls Rust: `import { invoke } from "@tauri-apps/api/core"` → `invoke("command_name", { args })`
- Rust defines commands: `#[tauri::command] fn command_name(arg: Type) -> ReturnType`
- Commands must be registered: `.invoke_handler(tauri::generate_handler![command_name])`
- New Tauri plugin permissions must be added to `src-tauri/capabilities/default.json`

## Adding Tauri Plugins

Plugins require both JS and Rust packages:
```bash
pnpm add @tauri-apps/plugin-<name>       # JS bindings
cargo add tauri-plugin-<name> --manifest-path src-tauri/Cargo.toml  # Rust crate
```
Then register in `lib.rs` with `.plugin(tauri_plugin_<name>::init())` and add permissions to `capabilities/default.json`.
