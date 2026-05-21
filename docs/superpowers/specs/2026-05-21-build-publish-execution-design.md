# Build And Publish Execution Design

## Goal

OMD Release Console should own the full release lane inside the tool: run deterministic build and collection steps, inspect outputs, generate manifests, then gate publishing through explicit version confirmation. AI is reserved for diagnosing unexpected logs, not for driving the normal path.

## Scope

This pass adds the missing execution spine:

- `build-windows`: create a clean remote 4090 worktree at the target commit and run the Windows build command through SSH.
- `collect-windows-artifacts`: copy configured Windows artifacts and signatures from the remote worktree into `releaseDir/windows`.
- `build-mac`: run the configured Mac build command locally and copy configured Mac artifacts and signatures into `releaseDir/mac`.
- The release sequence becomes `preflight -> build-windows -> collect-windows-artifacts -> build-mac -> check-artifacts -> manifest -> publish-github -> publish-gitee -> verify -> report`.

Publishing remains guarded by `confirmVersion`. GitHub/Gitee publishing should stay routed through fixed Rust actions; no UI or API accepts arbitrary shell commands.

## Configuration

Each required artifact keeps its release-facing `file` and `signature`, and gains optional source locations:

- `sourceFile`: source path for the artifact before it is copied into the release directory.
- `sourceSignature`: source path for the signature before it is copied into the release directory.

For Windows artifacts, source paths are interpreted relative to the remote worktree recorded by `build-windows`. For Mac artifacts, source paths are interpreted relative to `sourceRepo`. Absolute paths remain allowed for local Mac sources only. Supported template variables are `{version}` and `{windowsTarget}`.

## State

New step keys are added:

- `buildWindows`
- `collectWindowsArtifacts`
- `buildMac`

Each build/collect action writes start, completion, failure, duration, summary, and history events into `state.json`. `buildWindows` records the remote worktree so later collection can use the same build output.

## UI

The main rail should show build and collection as first-class stages instead of hiding them behind dry-run. The primary button follows state and runs the next safe action. Publish buttons remain separate and require the confirmation field.

## Safety

The Rust core remains the only execution layer. It builds commands from config and fixed templates, validates release directory paths, and records stdout/stderr. When a command fails, the tool stores the failure and suggestions. AI diagnosis can later consume the recorded logs, but it does not bypass the Rust action whitelist.

