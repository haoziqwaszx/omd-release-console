# SRC KNOWLEDGE BASE

## OVERVIEW

`src/` contains one file: `main.rs`. It is not just an entry point; it is the entire Rust release core, desktop bridge, browser debug server, release workflow, utilities, and unit tests.

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Process modes | `main.rs` 113-147 | `desktop` default, `serve`, `plan`, release CLI |
| Config/context | `main.rs` 149-350 | `AppContext`, embedded config fallback, validation |
| CLI release dispatch | `main.rs` 352-405 | Parses version/flags/release dir, creates `StateManager` |
| State manager | `main.rs` 407-583 | `state.json` load/save/start/complete/fail/record helpers |
| Preflight | `main.rs` 585-834 | Source version, tools, auth, SSH, updater endpoints |
| Release orchestration | `main.rs` 836-931 | dry-run, resume, full release, exact confirm gate |
| Windows 4090 | `main.rs` 933-1230, 1742-1827 | Upload/run PowerShell, retry access violation, collect zip |
| Mac build | `main.rs` 1081-1102, 1232-1317 | Pull source repo and copy Mac required artifacts |
| Publish | `main.rs` 1319-1706 | GitHub `gh release create`, Gitee API/curl |
| Artifacts/manifests | `main.rs` 1838-2116 | Required artifact scan, checksums, updater manifests |
| HTTP/Tauri action execution | `main.rs` 2277-2571 | Shared whitelist executor and action prereqs |
| Summary/config API shapes | `main.rs` 2586-2825 | Frontend-facing JSON summaries |
| Utilities | `main.rs` 2893-3580 | HTTP parsing, curl, JSON, process, path, time helpers |
| Tests | `main.rs` 3582-4252 | Inline unit tests for safety-critical behavior |

## CONVENTIONS

- Keep CLI, Tauri commands, and HTTP endpoints semantically mirrored.
- Add new UI/API executable behavior only by extending `action_command_args` with fixed args.
- State step keys are camelCase (`buildWindows`); UI/API action names are kebab-case (`build-windows`).
- Release failures should preserve `state.json`, logs, errors, and suggestions for diagnosis.
- `release.config.json` supplies release paths/artifact templates; avoid hardcoding new release constants in logic.
- Tests are behavior-named snake_case and live at the bottom of `main.rs`.

## ANTI-PATTERNS

- Do not add arbitrary command execution, even behind “advanced” UI/API fields.
- Do not weaken `confirm_version_matches`; exact version equality is deliberate.
- Do not skip `missing_action_prerequisite` checks to make the UI feel faster.
- Do not log or persist Gitee tokens, GitHub credentials, private keys, or signing passwords.
- Do not expand release-dir access outside Desktop `omd-<version>-release-*` without redesigning the safety model.
- Do not extract unrelated modules in bulk. Extract only a boundary required by the current task.

## TESTING

```bash
cargo test
cargo test confirm_version_requires_exact_target_match
npm run check
```

`cargo test` should be run from repo root; some tests read `scripts/windows/omd-release-build.ps1` relatively and zip tests use `python3`.
