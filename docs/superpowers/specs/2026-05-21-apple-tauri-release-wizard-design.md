# Apple Tauri Release Wizard Design

## Goal

Rework OMD Release Console into a native-feeling Apple-style Tauri desktop app that guides a user from "code is ready to release" through build, artifact collection, publishing, verification, and report generation.

The app should feel like a focused release assistant, not a browser dashboard or command console.

## Product Positioning

The primary product is a Rust + Tauri desktop application.

- Rust owns all release execution.
- Tauri is the native shell and bridge.
- The web UI is the desktop app's view layer, not the business runtime.
- `serve` remains useful for development and debugging, but the user-facing product should not depend on opening a browser.

The expected user moment is:

1. The user has finished code changes in `dix-extension-ui`.
2. The user decides this version should be released.
3. The user opens OMD Release Console.
4. The app guides the user through every release step until verification and report output are complete.

## Visual Direction

Use an Apple/macOS utility style:

- Light neutral background: `#f5f5f7` family.
- White or translucent panels with subtle borders.
- SF system font stack.
- Calm blue primary action, not green.
- Sidebar navigation with a single focused main panel.
- Minimal chrome, low visual noise, generous spacing.
- Status indicators should be small and quiet.
- Logs, raw commands, and detailed artifacts are secondary disclosure, not first-screen content.

Avoid:

- Dashboard card grids as the primary screen.
- Multiple equally prominent action buttons.
- Marketing hero layout.
- Dense terminal-like output as the default view.
- Browser-centric framing or wording.

## Information Architecture

The app shell has three major zones:

1. Window header
   - App name: `OMD Release`.
   - Current version.
   - Overall status.
   - Optional compact release directory path.

2. Left release journey sidebar
   - Stage 1: Prepare Release.
   - Stage 2: Package Apps.
   - Stage 3: Collect And Check.
   - Stage 4: Publish Distribution.
   - Stage 5: Verify And Report.

3. Main step panel
   - Current step title.
   - Plain-language explanation of what will happen.
   - One primary action.
   - Compact status summary.
   - Expandable details for logs, artifacts, commands, and troubleshooting.

## Release Journey

### Stage 1: Prepare Release

Purpose: confirm the release context before any build or publish action.

User-facing actions:

- Confirm target version.
- Confirm source repository.
- Confirm release directory.
- Run preflight.

Rust actions:

- `preflight`.

The user should understand whether the environment is ready before seeing build buttons.

### Stage 2: Package Apps

Purpose: package both platform outputs without making the user think in commands.

User-facing actions:

- Package Windows.
- Package Mac.

Rust actions:

- `build-windows`.
- `build-mac`.

Windows packaging still runs through the configured 4090 SSH target. Mac packaging runs locally through Rust from the configured source repo.

### Stage 3: Collect And Check

Purpose: bring all artifacts into the release directory and prove required files exist.

User-facing actions:

- Collect Windows artifacts.
- Check artifacts.

Rust actions:

- `collect-windows-artifacts`.
- `check-artifacts`.
- `manifest` may begin only after artifacts pass.

### Stage 4: Publish Distribution

Purpose: generate distribution metadata and publish through fixed actions.

User-facing actions:

- Generate manifest.
- Confirm version for publishing.
- Publish GitHub.
- Publish Gitee.

Rust actions:

- `manifest`.
- `publish-github`.
- `publish-gitee`.

Publishing must remain gated by exact version confirmation. The UI should make this feel like a deliberate release confirmation, not a command prompt.

### Stage 5: Verify And Report

Purpose: prove the release is usable and preserve release evidence.

User-facing actions:

- Verify online endpoints.
- Generate report.
- Open release folder.

Rust actions:

- `verify`.
- `report`.
- `open-release-dir`.

## Component Model

### `ReleaseShell`

Owns the desktop layout: header, sidebar, and main panel.

### `ReleaseSidebar`

Maps low-level Rust step state into five user-facing stages.

Each stage has:

- label
- short description
- status
- active marker
- optional progress count

### `StepFocusPanel`

Displays only the current next action.

It should include:

- stage eyebrow
- step title
- short explanation
- primary button
- status pill
- one concise suggestion or blocker

### `StepDetails`

Expandable supporting panel.

Contains:

- checks
- logs
- artifact list
- manifest summary
- publish results
- command references for debugging only

### `ReleaseContextBar`

Contains version, release directory, and native desktop helpers.

This should be visually compact and should not dominate the workflow.

## Data Flow

The frontend derives all product state from Rust summaries and action responses.

1. UI loads config and summary.
2. UI maps Rust step keys into journey stages.
3. UI resolves the next required action.
4. User clicks the single primary action.
5. UI calls Tauri command `tauri_action` or development HTTP action.
6. Rust validates input and executes a fixed whitelist action.
7. Rust writes `state.json`.
8. UI reloads summary and advances the journey.

The browser development server can use the same HTTP action API, but production behavior should assume Tauri command invocation.

## Safety And Trust

The app must not expose arbitrary shell execution.

Safety rules:

- Every executable operation is a named Rust action.
- Release directory must remain under the allowed Desktop release pattern.
- Publish actions require exact `confirmVersion`.
- Logs must not persist tokens or credentials.
- Gitee token handling stays in Rust and is read from git credential storage.
- AI diagnosis, when added later, can read logs and suggest fixes but must not bypass action whitelists.

## Error Handling

Errors should be shown as release blockers, not raw terminal failures.

Each error state should provide:

- what failed
- why it likely failed
- what the user can do next
- expandable logs for technical detail

The main panel should always answer: "What is blocking this release right now?"

## Testing

Verification should cover:

- `npm run check`.
- Rust unit tests for action routing and publish safety.
- Frontend syntax check.
- Browser/Tauri visual smoke test that confirms:
  - sidebar stages render
  - one primary action is visible
  - publish confirmation controls are present
  - old Phase 1/Phase 2 disabled copy is gone
  - text does not overflow on desktop and narrow widths

## Acceptance Criteria

The redesign is successful when:

- The first screen looks like a focused macOS release assistant.
- The main screen shows one next action, not a list of competing actions.
- Users can understand the full path from packaging to publishing from the sidebar.
- Packaging and publishing are clearly launched from the Tauri UI.
- Browser use is not required for the product experience.
- Raw logs and commands are available but not visually primary.
- Existing Rust release functionality remains intact.
