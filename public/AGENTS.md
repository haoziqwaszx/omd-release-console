# PUBLIC KNOWLEDGE BASE

## OVERVIEW

`public/` is a static, no-bundler release cockpit frontend. `index.html` provides fixed DOM anchors, `app.js` is the UI state machine, and `styles.css` is the macOS-style visual system.

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| DOM shell | `index.html` | Toolbar, context fields, progress rail, primary card, logs, side sheets |
| Backend bridge | `app.js` 1, 104-149, 596-656 | Tauri `invoke` in desktop, HTTP fallback in browser |
| Journey model | `app.js` 3-9, 168-227 | Five user-facing stages mapped from backend steps |
| Action execution | `app.js` 580-656 | Primary action, publish confirmation, log polling |
| Details/diagnostics | `app.js` 364-575 | Current step details and config/manifest/artifacts/history/commands tabs |
| Escaping/render helpers | `app.js` 417-754 | Manual `escapeHtml`; many renderers use `innerHTML` |
| Visual system | `styles.css` | CSS variables, cards, status pills, side sheets, responsive layout |

## CONVENTIONS

- `index.html` must keep `<script type="module" src="/app.js">`; `app.js` uses top-level await.
- DOM `id`s are hard dependencies through the `elements` map. Rename only with matching JS updates.
- Desktop-only controls are hidden when `window.__TAURI_INTERNALS__?.invoke` is absent.
- Tauri command and HTTP endpoint behavior must remain mirrored.
- Publish confirmation UI is only UX; Rust remains the safety boundary.
- Validate frontend changes with `node --check public/app.js` or `npm run check`.

## STEP MAPPING GOTCHAS

- UI state keys: `buildWindows`, `collectWindowsArtifacts`, `publishGithub`.
- Action names: `build-windows`, `collect-windows-artifacts`, `publish-github`.
- `manifest` is a UI aggregate over backend/state `manifestGithub` and `manifestGitee`.
- Adding a step usually touches `journeyStages`, `resolveCurrentStep`, `resolvePrimaryAction`, `statusForStep`, `renderStepDetails`, and backend JSON shape.

## ANTI-PATTERNS

- Do not introduce arbitrary command input or a terminal-style command runner.
- Do not treat `commandCard` high-risk regex as a security mechanism.
- Do not add framework/bundler dependencies unless explicitly scoped.
- Do not put full config, raw logs, long manifests, or complete artifact lists into the primary view by default.
- Do not insert backend/user strings into `innerHTML` without `escapeHtml`.
- Do not rely on `navigator.clipboard` always working; it has no current fallback.

## STYLE

- Apple-like utility UI: one current step, one primary CTA, quiet status indicators.
- CSS classes are semantic (`release-*`, `toolbar-*`, `journey-*`, `focus-*`, `status-pill`).
- `body` is desktop-first (`100vh`, hidden overflow); put overflow inside panels/sheets.
