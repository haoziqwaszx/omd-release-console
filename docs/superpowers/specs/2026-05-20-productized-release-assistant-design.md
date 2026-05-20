# Productized Release Assistant Design

## Background

Phase 1 made OMD Release Console useful as a release preparation center. Phase 1.5 moved the frontend into a stepper-style wizard, but the page still feels closer to an engineering dashboard than a polished product: too many panels compete for attention, the stepper is visually heavy, and diagnostic material remains too visible on the first screen.

The next frontend pass should keep all existing Phase 1 capabilities while changing the product experience into an Apple-style release assistant: calm, focused, and decisive. The page should answer one question at a time instead of showing every release fact at once.

## Goal

Build a more refined single-screen release assistant that emphasizes:

- one current step
- one primary action
- one most important suggestion
- quiet progress context
- details available on demand

This is a productization pass over the current frontend. It does not enable real GitHub or Gitee publishing.

## Non-Goals

- Do not add Phase 2 publishing endpoints.
- Do not change the backend API contract unless a frontend blocker is found.
- Do not introduce a frontend framework or new dependency.
- Do not add release history, run IDs, or archive views.
- Do not expose raw logs, commands, full config, and full artifact lists by default.

## Design Direction

The selected direction is **Apple-style Release Assistant**.

The interface should feel like a guided product flow rather than a dashboard. The release operator should see a clear centered task, an obvious CTA, and restrained surrounding context. Full diagnostics remain available, but they move behind a details affordance.

### First-Screen Hierarchy

The first viewport should be organized as:

1. **Compact Release Bar**
   - Shows product name, target version, release directory, and overall state.
   - Keeps edit/change controls secondary.
   - Does not show full configuration or manifest data.

2. **Centered Current Step Card**
   - The visual center of the product.
   - Shows current step number, current step title, one plain-language description, current status, one primary CTA, and one highlighted suggestion.
   - Uses generous spacing, controlled type sizes, and a calm surface.

3. **Quiet Progress Rail**
   - A thin progress indicator with labels for `预检`, `产物`, `Manifest`, `验证`, and `Dry-run`.
   - Replaces the current row of large stepper buttons.
   - Shows progress without competing with the current step card.

4. **Collapsed Detail Drawer**
   - Holds checks, artifact rows, manifest summaries, commands, and logs.
   - Closed or visually subordinate by default.
   - Opens for troubleshooting, not as the default reading path.

## Information Rules

The assistant should aggressively reduce visible information:

- Show at most one primary CTA.
- Show at most one highlighted suggestion in the main card.
- Show no command cards on the first screen unless the current step is `Dry-run` or `准备完成`.
- Show no full config summary on the first screen.
- Show no full artifact list on the first screen.
- Show no raw stdout/stderr unless the user opens details.
- Keep Phase 2 publishing visible only as disabled future context.

When there are multiple errors or suggestions, the main card should pick the first highest-severity item. The remaining items can appear in the detail drawer.

## Step Model

Keep the current step resolution logic, but change presentation:

- `context`: release information is missing.
- `preflight`: preflight has not succeeded.
- `artifactCheck`: artifact check or manifest generation has not succeeded.
- `verify`: online verification has not succeeded.
- `dryRunRelease`: dry-run command generation has not succeeded.
- `complete`: release preparation is complete.

The visible progress rail should group backend steps into five product steps:

1. `预检`
2. `产物`
3. `Manifest`
4. `验证`
5. `Dry-run`

The disabled publishing step should not be part of the main progress rail for this pass. It can appear as a small note in the complete state: `真实发布将在 Phase 2 启用`.

## Current Step Card States

### Context Missing

- Title: `准备发布信息`
- Description: `确认目标版本和 release 目录后开始发布准备。`
- CTA: `确认发布信息`
- Suggestion: tell the user which field is missing.

### Preflight

- Title: `先确认发布环境`
- Description: `检查源码版本、凭据、4090 连接和 updater 端点。`
- CTA: `运行预检` or `重新运行预检`
- Suggestion: highest-severity preflight suggestion.

### Artifacts and Manifest

- Title: `确认发布产物`
- Description: `检查 Windows 与 Mac 产物及签名，然后生成 GitHub/Gitee manifest。`
- CTA: `生成 Manifest`
- Suggestion: first missing artifact or manifest blocker.

### Verify

- Title: `验证线上端点`
- Description: `确认 updater manifest 与下载 URL 可访问且版本匹配。`
- CTA: `验证端点`
- Suggestion: first endpoint or URL blocker.

### Dry-run

- Title: `生成发布命令`
- Description: `生成真实发布前的 dry-run 命令，不执行上传。`
- CTA: `生成 dry-run`
- Suggestion: explain that this is still safe and non-publishing.

### Complete

- Title: `发布准备完成`
- Description: `预检、产物、manifest、验证和 dry-run 已完成。`
- CTA: `复制发布命令`
- Suggestion: `真实发布将在 Phase 2 启用。`

## Visual System

The current page is too card-heavy. The redesign should use fewer framed surfaces:

- Page background: quiet neutral gray, no decorative gradient blobs.
- Main card: white surface, subtle border, 8-16px radius.
- Release bar: unframed or lightly bordered.
- Progress rail: thin segmented line, not large button cards.
- Details: simple drawer or subdued panel below the fold.

Use restrained colors:

- Green for success/continue.
- Amber for running or attention.
- Red for blockers.
- Slate/gray for neutral text and disabled states.

Avoid an all-green or one-note palette. Green is an action/status color, not the whole brand palette.

Typography should be more controlled than the current version:

- Main card title can be prominent, but not hero-scale.
- Compact labels should avoid excessive letter spacing.
- No negative letter spacing.
- Button text must fit on mobile and desktop.

## Interaction Design

### Primary Action

The primary button always maps to the current step. It should be visually dominant and stable.

When an action is running:

- Button label becomes `执行中...`
- Main card status becomes running.
- Other controls remain visually quiet.

### Details Drawer

The detail drawer should expose context without becoming the default workflow:

- Closed or compact by default.
- Label: `查看检查详情`
- Content changes based on current step.
- Logs remain a nested toggle inside details.

Suggested detail sections:

- Preflight: checks and all suggestions.
- Artifacts/Manifest: required artifacts, generated manifests, missing files.
- Verify: endpoint results.
- Dry-run/Complete: command cards.

### Release Bar Editing

The version and release directory should not dominate the page. Prefer a compact bar with a `更改` affordance that expands the fields, or keep the fields visible but visually subordinate to the main card.

If a field is missing, the current step card should guide the user instead of relying on the field styling alone.

## Data Flow

No backend data changes are required.

Frontend still consumes:

- `GET /api/summary`
- `GET /api/config`
- `GET /api/commands`
- `POST /api/actions/preflight`
- `POST /api/actions/manifest`
- `POST /api/actions/verify`
- `POST /api/actions/dry-run-release`

The frontend should add a presentation layer that derives:

- `currentStep`
- `primaryAction`
- `primarySuggestion`
- `progressItems`
- `detailMode`
- `releaseContextCollapsed`

This can live in `public/app.js` without a new dependency.

## File Scope

Expected implementation files:

- `public/index.html`
- `public/app.js`
- `public/styles.css`

Backend files should stay unchanged unless verification reveals an existing frontend-blocking issue.

## Testing

Verification should include:

- `node --check public/app.js`
- `npm run check`
- local browser smoke test at `http://127.0.0.1:4177`
- desktop screenshot review
- mobile-width screenshot review

Manual acceptance checks:

- First screen shows one dominant current step card.
- There is only one primary CTA.
- The old six-card stepper is replaced by a quiet progress rail.
- Auxiliary details are not visually dominant by default.
- The complete state still makes Phase 2 publishing status clear.
- Existing safe actions still work.

## Open Risk

This pass may hide information that was previously easy to see. The mitigation is a reliable details drawer with all current diagnostics preserved. The main view should become calmer, not less capable.
