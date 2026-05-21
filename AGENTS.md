# PROJECT KNOWLEDGE BASE

**Generated:** 2026-05-22
**Commit:** c6e0be2
**Branch:** codex/release-build-publish-actions

## OVERVIEW

OMD Release Console is a standalone Rust/Tauri release cockpit for the OMD product repo at `/Users/glame/Desktop/dix-extension-ui`. The Mac controller owns artifact collection, manifest generation, GitHub/Gitee publish, endpoint verification, and reporting; the Windows 4090 machine only builds/signs Windows artifacts.

## STRUCTURE

```text
omd-release-console/
├── src/main.rs                  # monolithic Rust release core: CLI, Tauri, HTTP, state, publish, tests
├── public/                      # static HTML/CSS/JS cockpit, no bundler/framework
├── scripts/windows/             # 4090 PowerShell build/export script
├── docs/superpowers/            # historical plans/specs; use for intent, not current truth
├── release.config.json          # non-secret release configuration embedded into desktop binary
├── tauri.conf.json              # Tauri v2 config; frontendDist points at public/
├── package.json                 # npm aliases for cargo commands
└── CLAUDE.md                    # previous agent guide; root source for safety rules
```

Exclude runtime/generated/tool output from source reasoning: `.omx/`, `.superpowers/`, `.playwright-cli/`, `output/`, `gen/`, `target/`, `release-history.json`.

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| CLI routing / modes | `src/main.rs` 120-147, 352-405 | `desktop`, `serve`, `plan`, release subcommands |
| Tauri desktop commands | `src/main.rs` 166-275 | Mirrors HTTP concepts for desktop mode |
| Browser/debug HTTP API | `src/main.rs` 2191-2275 | Hand-written `TcpListener`; no web framework |
| Release `state.json` | `src/main.rs` 407-583, 3360-3434 | Step status, checks, artifacts, manifests, history, suggestions |
| UI/API action whitelist | `src/main.rs` 2277-2571 | Fixed action-to-current-binary args; no arbitrary shell |
| Release confirmation gate | `src/main.rs` 836-931, 1319-1339, 2291-2311 | Real publish requires exact `confirmVersion == version` |
| Windows 4090 build | `src/main.rs` 933-1230, 1742-1827; `scripts/windows/omd-release-build.ps1` | Upload script, SSH run, pull `windows-artifacts.zip` |
| Mac build | `src/main.rs` 1081-1102, 1232-1317 | Pull source repo, run `tauri:build:mac`, copy artifacts |
| Artifact/checksum/manifest | `src/main.rs` 1838-2116 | Required artifacts come from `release.config.json` templates |
| GitHub/Gitee publish | `src/main.rs` 1319-1706 | GitHub via `gh`; Gitee via API/curl + git credential token |
| Online verification | `src/main.rs` 2118-2189, 2978-3027 | Fetch updater manifests and check download URLs |
| Report/history | `src/main.rs` 1881-1937 | `release-report.md` and Application Support history |
| Frontend state machine | `public/app.js` | Tauri `invoke` with HTTP fallback |
| UI shell/styles | `public/index.html`, `public/styles.css` | Static macOS-style cockpit |
| Tests | `src/main.rs` 3582-4252 | Inline Rust unit tests; no separate `tests/` dir |

## CODE MAP

| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `run_main` | fn | `src/main.rs` 120 | Selects desktop/server/CLI entry style |
| `AppContext::load` | method | `src/main.rs` 150 | Resolves root/public/Desktop/data dirs and config |
| `StateManager` | struct/impl | `src/main.rs` 74, 407 | Owns release-dir `state.json` lifecycle |
| `run_action` | fn | `src/main.rs` 2291 | Shared Tauri/HTTP action executor |
| `action_command_args` | fn | `src/main.rs` 2466 | Whitelist mapping from action names to fixed CLI args |
| `preflight` | fn | `src/main.rs` 585 | Version, credentials, SSH, endpoints, tool checks |
| `release` | fn | `src/main.rs` 836 | Full/dry-run/resume/single-publish release entry |
| `build_windows` | fn | `src/main.rs` 1104 | Uploads and runs 4090 build script |
| `collect_windows_artifacts` | fn | `src/main.rs` 1172 | Pulls and safely extracts Windows artifact zip |
| `build_mac` | fn | `src/main.rs` 1232 | Builds Mac artifacts from source repo |
| `publish_github` / `publish_gitee` | fn | `src/main.rs` 1341, 1383 | Real distribution publish steps |
| `serve` / `handle_connection` | fn | `src/main.rs` 2191, 2211 | Browser debug HTTP server/router |
| `journeyStages` | const | `public/app.js` 3 | User-facing release journey grouping |
| `resolveCurrentStep` | fn | `public/app.js` 168 | Maps backend state to next UI step/action |
| `runAction` | fn | `public/app.js` 596 | Calls Tauri/HTTP action and polls logs |

## CONVENTIONS

- Canonical local validation: `npm run check`.
- Rust tests are inline in `src/main.rs`; run all with `cargo test`, one with `cargo test <test_name>`.
- Frontend validation is syntax-only: `node --check public/app.js`.
- HTTP smoke uses fixed port when testing: `PORT=4188 cargo run -- serve`.
- `release.config.json` is non-secret and embedded via `include_str!`; do not put tokens/passwords/private keys there.
- Release output dirs must stay under Desktop and match `omd-<version>-release-*`.
- UI/API action names are kebab-case; state step keys are camelCase.
- Prefer incremental extraction only when a touched task needs a boundary; do not refactor the whole monolith just because it is large.

## ANTI-PATTERNS (THIS PROJECT)

- Accepting arbitrary shell/command input from UI, HTTP, or Tauri.
- Weakening or bypassing exact `confirmVersion == version` for `publish-github` / `publish-gitee`.
- Running real GitHub/Gitee publish or 4090 SSH build during normal validation without explicit user request.
- Storing credentials in config, state, logs, reports, or generated AGENTS files.
- Broadening action `releaseDir` to arbitrary paths.
- Shell globbing publish assets; assets must be structurally discovered from manifest + `windows/` + `mac/`.
- Treating `docs/superpowers/plans` checklists as current work items without verifying code/README.
- Editing generated/runtime dirs: `.omx/`, `.superpowers/`, `gen/`, `output/`, `.playwright-cli/`, `target/`.

## UNIQUE STYLES

- One Rust binary has three faces: Tauri desktop, browser/debug HTTP server, direct CLI.
- Frontend is plain module JS with top-level await; no React/Vite/bundler.
- Desktop mode uses Tauri `invoke`; browser mode falls back to mirrored HTTP endpoints.
- UI is a macOS-style release wizard: one current step, one primary CTA, logs/details/diagnostics secondary.
- Windows build is delegated to a long-lived PowerShell script uploaded to 4090 before execution.

## COMMANDS

```bash
# Desktop app
npm run dev

# Browser/debug HTTP mode
npm run serve

# Full local validation
npm run check

# Rust tests
cargo test
cargo test <test_name>

# Frontend syntax only
node --check public/app.js

# Build release binary
npm run build

# HTTP smoke
PORT=4188 cargo run -- serve
curl -fsS http://127.0.0.1:4188/api/config
curl -fsS http://127.0.0.1:4188/api/history
curl -fsS 'http://127.0.0.1:4188/api/commands?version=0.0.7'
```

## NOTES

- `rust-analyzer` is not installed in this environment; use structural search/outline and tests as fallback.
- `cargo test` may require `python3` for zip fixture generation.
- Run tests from repo root because tests read `scripts/windows/omd-release-build.ps1` by relative path.
- `release-history.json` is generated runtime state and ignored by git.
