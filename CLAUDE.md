# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project purpose

OMD Release Console is a standalone release cockpit for OMD, separate from the `dix-extension-ui` product repository. The intended release split is:

- Windows 4090 build machine: Windows build and signing only.
- Mac controller machine: collect artifacts, generate manifests, publish to GitHub/Gitee, verify updater endpoints, and generate the release report.

The app is intentionally whitelist-only: UI/API actions map to fixed Rust commands, not arbitrary shell input. Real GitHub/Gitee publish actions require a confirmation version matching the target version.

## Common commands

```bash
# Desktop app mode, default for local use
npm run dev
# equivalent: cargo run -- desktop

# Browser/debug HTTP mode
npm run serve
# equivalent: cargo run -- serve

# Full local validation
npm run check
# expands to: cargo fmt --check && cargo test && cargo clippy -- -D warnings && node --check public/app.js

# Build Rust release binary
npm run build
# equivalent: cargo build --release

# Run all Rust tests
cargo test

# Run one Rust test by name
cargo test <test_name>

# Check frontend JavaScript syntax only
node --check public/app.js
```

Useful CLI release commands:

```bash
cargo run -- plan
cargo run -- preflight 0.0.7
cargo run -- build-windows 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- build-mac 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- collect-windows-artifacts 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- check-artifacts 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- manifest 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --step publish-github --confirm-version 0.0.7
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --step publish-gitee --confirm-version 0.0.7
cargo run -- verify 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- report 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --dry-run
```

For HTTP smoke testing, run the server with a fixed port:

```bash
PORT=4188 cargo run -- serve
curl -fsS http://127.0.0.1:4188/api/config
curl -fsS http://127.0.0.1:4188/api/history
curl -fsS 'http://127.0.0.1:4188/api/commands?version=0.0.7'
```

## Architecture overview

- `src/main.rs` is the Rust release core and currently contains the whole backend: CLI routing, Tauri command registration, the lightweight HTTP server, release state management, action execution, artifact/manifest logic, publish logic, verification, reporting, utilities, and unit tests.
- `run_main()` selects one of three entry styles from the same binary:
  - `desktop`/`app`/`gui` starts Tauri.
  - `serve`/`server`/`dev` starts the local HTTP server for browser debugging.
  - release subcommands execute CLI actions directly.
- `AppContext` loads repository paths, desktop/data directories, and release configuration. The compiled desktop app embeds the default release config with `include_str!("../release.config.json")`, so it does not depend on being launched from the repository for default config values.
- `StateManager` owns `state.json` for a release directory. Release actions update step statuses, history, suggestions, artifacts, manifests, and report metadata there.
- The frontend is static and lives under `public/`:
  - `index.html` defines the release cockpit shell, progress rail, log panel, detail sheet, and diagnostics sheet.
  - `app.js` is the UI state machine. It uses Tauri `invoke` when running in desktop mode and falls back to HTTP endpoints when served in browser mode.
  - `styles.css` contains the productized cockpit styling.
- HTTP endpoints and Tauri commands intentionally mirror each other for the same concepts: config, summary, commands, action execution, action logs, latest release dir, open release dir, and history.
- `release.config.json` is non-secret release configuration: source repo location, GitHub/Gitee distribution repos, updater endpoints, Windows SSH/repo settings, target triple, release directory pattern, and required artifact templates.
- Release output directories are expected to look like `~/Desktop/omd-<version>-release-<timestamp>/` with `windows/`, `mac/`, `github/latest.json`, `gitee/latest.json`, `checksums.sha256`, `state.json`, and `release-report.md`.
- `release-history.json` is generated runtime state and is ignored by git.

## Safety-sensitive behavior

- Do not replace the whitelist action model with arbitrary command execution from the UI/API.
- Preserve the `confirmVersion == version` gate for true publish actions.
- Treat GitHub/Gitee publish commands and 4090 SSH build actions as real external side effects; prefer dry-run, unit tests, and local smoke tests unless the user explicitly asks to execute a real release step.
