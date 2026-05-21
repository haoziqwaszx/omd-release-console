# SUPERPOWERS DOCS KNOWLEDGE BASE

## OVERVIEW

This directory is a historical planning/spec archive. Use it to understand why the release console exists and how the design evolved; do not treat plan checkboxes as current work items.

## STRUCTURE

```text
docs/superpowers/
├── plans/    # dated implementation plans; many are already completed or superseded
└── specs/    # dated design specs; useful for durable intent and safety model
```

## WHERE TO LOOK

| Need | File pattern | Notes |
|------|--------------|-------|
| Current-ish Tauri release wizard intent | `specs/2026-05-21-apple-tauri-release-wizard-design.md` | Desktop-first Rust/Tauri, five-stage journey, UI priorities |
| Real publish constraints | `specs/2026-05-21-real-publish-execution-design.md` | Fixed actions, exact confirmation, Gitee token handling |
| Build/publish spine | `specs/2026-05-21-build-publish-execution-design.md` | Full release lane and state model |
| Windows 4090 script context | `plans/2026-05-21-4090-windows-release-build-script.md` | Cached build dir, export zip, collect behavior |
| Early state/config model | `specs/2026-05-20-phase-1-release-preparation-center-design.md` | `state.json`, non-secret config, whitelist action API |

## DURABLE DECISIONS

- Rust is the execution authority; frontend is a view/action trigger layer.
- UI/API/Tauri actions are fixed whitelist operations, not arbitrary shell.
- `state.json` is the release run source of truth for frontend display and recovery.
- Real publish is enabled now but gated by exact `confirmVersion == version`.
- GitHub/Gitee assets are fixed/discovered from manifest + `windows/` + `mac/`.
- Gitee credentials come from git credential storage and must not enter state/logs.
- Windows 4090 only builds/signs; Mac controller collects, manifests, publishes, verifies, reports.
- UI should keep one current step and one primary CTA; details/logs/config stay secondary.

## GOTCHAS

- Older 2026-05-20 docs saying “real publish disabled / Phase 2 later” are historical; verify against README and `src/main.rs`.
- Some plan checkboxes may be unchecked even when code has since implemented the behavior.
- Source of truth order: current code/config/README first, then latest docs, then older plans/specs.
- Do not copy large transient implementation steps into root docs; extract durable constraints only.
