# Real Publish Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn GitHub/Gitee publish from manual-required command output into real Rust whitelist actions.

**Architecture:** Keep the Rust monolith for this pass. Add pure helpers for asset discovery and command/API payload construction, then wire `publish_step` to real GitHub and Gitee execution while preserving explicit version confirmation.

**Tech Stack:** Rust 2021, serde_json, `gh`, `curl`, git credential storage, static HTML/CSS/JavaScript.

---

## File Structure

- Modify `src/main.rs`: add publish helper tests, enable publish steps, add GitHub/Gitee execution helpers, and remove the Phase 1 disabled/manual-required path.
- Modify `public/app.js`: route the primary action through GitHub publish, Gitee publish, verify, and report.
- Modify `public/index.html`: update publish confirmation copy and buttons.
- Modify `README.md`: document real publish behavior and safety gates.

## Task 1: Lock Publish Behavior With Tests

- [x] Add tests proving publish steps start as `not_started`.
- [x] Add tests proving publish assets come only from `{host}/latest.json`, `windows/`, and `mac/`.
- [x] Add tests proving GitHub release args contain fixed paths and no shell globs.
- [x] Add tests proving Gitee release payload uses API fields.
- [x] Run the targeted tests and confirm they fail before implementation.

## Task 2: Implement Real Publish Actions

- [x] Remove Phase 1 disabled state for publish steps.
- [x] Route `publish_step` to `publish_github` and `publish_gitee` after `confirmVersion` matches.
- [x] Implement GitHub publishing through `gh release create`.
- [x] Implement Gitee publishing through Release, attachment upload, and contents update API calls.
- [x] Keep Gitee token out of state/log files by passing token-bearing payloads through stdin.

## Task 3: Productize UI And Docs

- [x] Update the wizard step order to run GitHub, then Gitee, then verify.
- [x] Update publish labels from command generation to real publish actions.
- [x] Add publish result rendering.
- [x] Update README with real publish behavior and confirmation requirements.

## Task 4: Verify

- [x] Run targeted publish tests.
- [x] Run `npm run check`.
