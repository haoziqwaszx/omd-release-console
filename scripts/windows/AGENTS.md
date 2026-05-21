# WINDOWS SCRIPT KNOWLEDGE BASE

## OVERVIEW

`omd-release-build.ps1` is the long-lived Windows 4090 build/export script. The Rust controller uploads it before `build-windows`, invokes it over SSH, then `collect-windows-artifacts` pulls back `windows-artifacts.zip`.

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Parameters | `omd-release-build.ps1` 1-14 | `-Version` required; repo/build/export paths optional |
| Step wrapper | `omd-release-build.ps1` 21-71 | `Fail`, `Run-Step`, native command handling |
| Safe path checks | `omd-release-build.ps1` 73-135 | Refuses drive roots and overlapping repo/build/export dirs |
| Signing | `omd-release-build.ps1` 155-205 | Tauri signer job with timeout and explicit empty password arg |
| Sync/build | `omd-release-build.ps1` 233-311 | clone/fetch/reset/pull/clean, npm cache, lint, Tauri builds |
| Green zip | `omd-release-build.ps1` 314-324 | Create and sign green artifact |
| Export zip | `omd-release-build.ps1` 326-375 | Copy fixed artifacts, write `build-summary.json`, zip export |

## CONVENTIONS

- Script is version-agnostic; target version comes from `-Version`.
- It verifies product `package.json` and `src-tauri/tauri.conf.json` match `-Version`.
- Default build dir is cached: `C:\Users\4090\Desktop\dix-extension-ui-release-build`.
- `git clean -fdx` preserves `node_modules`, `src-tauri/target`, and `.omd-release-cache`.
- Export dir defaults to `C:\Users\4090\AppData\Local\Temp\omd-release-export-<version>`.
- Export contract is `windows-artifacts.zip` plus `build-summary.json`; Rust collection should pull the zip, not individual files.

## ANTI-PATTERNS

- Do not hardcode a specific release version into the script.
- Do not remove path-overlap/drive-root safety checks.
- Do not make the script publish anything; Windows 4090 only builds/signs/exports.
- Do not change artifact names without updating `release.config.json` and Rust tests.
- Do not store signing secrets in this repository or generated summaries.

## VERIFY

Rust tests cover this script through `cargo test`, especially Windows path, invocation, signing, export, and wrapper behavior.
