# Build And Publish Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the missing build and artifact-collection execution spine so OMD Release Console can drive Mac/Windows packaging from the tool before manifest and publish steps.

**Architecture:** Keep the current monolithic Rust core for this pass and add narrow helper functions with tests before production code. Execution remains whitelist-only: Tauri/HTTP actions map to CLI subcommands, CLI subcommands call fixed command builders, and every step writes structured state. The static UI is updated to expose build/collect as first-class release stages.

**Tech Stack:** Rust 2021, Tauri v2 command bridge, serde/serde_json, static HTML/CSS/JavaScript, SSH/SCP/GitHub CLI as fixed command targets.

---

## File Structure

- Modify `src/main.rs`: add step keys, command routing, source artifact config fields, build/collect command builders, build/collect actions, safe release order, and unit tests.
- Modify `release.config.json`: add artifact source paths used by collect actions.
- Modify `public/app.js`: add build/collect steps and primary-action routing.
- Modify `public/styles.css`: make the progress rail responsive to the expanded step count.
- Modify `README.md`: document desktop-first full-flow commands.

## Task 1: Add Build/Collect State And Routing Tests

- [ ] Add failing Rust tests for `release_step_order`, `action_command_args`, remote worktree generation, and artifact source path rendering.
- [ ] Run the targeted tests and confirm they fail because new actions do not exist yet.
- [ ] Add `buildWindows`, `collectWindowsArtifacts`, and `buildMac` to `STEP_KEYS`.
- [ ] Add CLI command routing for `build-windows`, `collect-windows-artifacts`, and `build-mac`.
- [ ] Add action mapping for the three new HTTP/Tauri actions.
- [ ] Rerun targeted tests until they pass.

## Task 2: Implement Windows Build And Collection

- [ ] Add helpers to build the remote worktree path from `runId`.
- [ ] Implement `build_windows(ctx)` to run fixed SSH commands and record the remote worktree in state.
- [ ] Add optional `sourceFile` and `sourceSignature` fields to `RequiredArtifact`.
- [ ] Implement `collect_windows_artifacts(ctx)` to copy Windows artifacts with fixed `scp` commands into `releaseDir/windows`.
- [ ] Add tests for source rendering and command generation.

## Task 3: Implement Mac Build And Collection

- [ ] Implement `build_mac(ctx)` to run `npm run tauri:build:mac` in `sourceRepo`.
- [ ] Copy Mac artifacts and signatures from configured source paths into `releaseDir/mac`.
- [ ] Record missing source artifacts as structured failures and suggestions.
- [ ] Add tests for Mac source rendering and missing-artifact messages.

## Task 4: Wire The UI Full Flow

- [ ] Expand `progressSteps` to include Windows build, Windows collection, Mac build, publish GitHub, and publish Gitee.
- [ ] Update `resolveCurrentStep` and `resolvePrimaryAction` so the main button advances through the full release sequence.
- [ ] Keep publish buttons gated by the confirmation input.
- [ ] Update CSS rail to use responsive columns instead of a hard-coded five-column grid.

## Task 5: Verify And Document

- [ ] Run `cargo fmt --check`.
- [ ] Run targeted Rust tests for the new action builders.
- [ ] Run `npm run check`.
- [ ] Update README with the new full-flow command list.
- [ ] Report implemented scope and explicitly call out which actions execute real commands.

