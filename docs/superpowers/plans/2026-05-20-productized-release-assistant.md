# Productized Release Assistant Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Turn the existing Phase 1.5 release wizard into a calmer Apple-style release assistant with one current step, one primary action, quiet progress, and details on demand.

**Architecture:** Keep the current Node server and Phase 1 APIs unchanged. Replace the frontend layout with a compact release bar, centered current-step card, quiet progress rail, and collapsed detail drawer. Add a small presentation view model in `public/app.js` to derive the primary suggestion and product progress items from existing `state.json` data.

**Tech Stack:** Vanilla HTML, CSS, JavaScript modules, existing Node.js built-in server.

---

## File Structure

- Modify `public/index.html`: replace hero/stepper/dashboard markup with release assistant regions.
- Modify `public/app.js`: keep API calls, update view-model derivation and render functions for one primary suggestion, progress rail, and details drawer.
- Modify `public/styles.css`: replace dashboard-card styling with restrained release assistant styling.
- Test with `node --check public/app.js`, `npm run check`, and browser screenshots.

## Task 1: Replace Dashboard Markup With Release Assistant Regions

**Files:**
- Modify: `public/index.html`
- Test: `node --check public/app.js`

- [x] **Step 1: Replace the body layout with assistant regions**

Replace the current `<main class="wizard-shell">...</main>` body content with:

```html
<main class="assistant-shell">
  <section class="release-bar" aria-labelledby="pageTitle">
    <div class="release-identity">
      <p class="eyebrow">OMD Release</p>
      <h1 id="pageTitle">发布助理</h1>
    </div>
    <div class="release-meta" aria-label="发布上下文">
      <label class="context-field" for="versionInput">
        <span>版本</span>
        <input id="versionInput" value="0.0.7" placeholder="0.0.7" />
      </label>
      <label class="context-field release-dir-field" for="releaseDirInput">
        <span>目录</span>
        <input id="releaseDirInput" placeholder="默认读取最新 release 目录" />
      </label>
      <button id="loadButton" class="quiet-button" type="button">读取</button>
      <button id="refreshButton" class="quiet-button" type="button">刷新</button>
    </div>
    <div class="release-state">
      <span class="status-label">整体状态</span>
      <strong id="overallStatusText">未开始</strong>
      <span id="overallStatusBadge" class="status-pill neutral">未开始</span>
    </div>
  </section>

  <p id="releaseDirText" class="release-path">正在读取...</p>

  <section class="assistant-stage" aria-label="发布助理">
    <article id="currentStepCard" class="assistant-card">
      <div id="currentStepNumber" class="step-orb">1</div>
      <p id="currentStepEyebrow" class="eyebrow">当前步骤</p>
      <h2 id="currentStepTitle">准备发布信息</h2>
      <p id="currentStepDescription" class="step-description">确认目标版本和 release 目录后开始发布准备。</p>
      <span id="currentStepStatus" class="status-pill neutral">未开始</span>
      <button id="primaryActionButton" class="primary-button" type="button">确认发布信息</button>
      <div id="focusSuggestions" class="focus-suggestions"></div>
    </article>

    <nav id="progressRail" class="progress-rail" aria-label="发布准备进度"></nav>
  </section>

  <section class="assistant-details" aria-label="检查详情">
    <details id="detailDrawer">
      <summary>
        <span>查看检查详情</span>
        <strong id="detailsSummaryText">当前步骤详情</strong>
      </summary>
      <div id="stepDetailContent" class="detail-content"></div>
      <button id="toggleLogsButton" class="ghost-button" type="button">显示日志</button>
      <pre id="actionLogOutput" class="action-log hidden">暂无动作日志</pre>
      <div class="secondary-grid">
        <section>
          <h3>配置摘要</h3>
          <div id="configSummary" class="compact-list"></div>
        </section>
        <section>
          <h3>Manifest 摘要</h3>
          <div id="manifestList" class="compact-list"></div>
        </section>
        <section>
          <h3>完整产物</h3>
          <div id="artifactList" class="compact-list"></div>
        </section>
        <section>
          <h3>命令</h3>
          <div id="commandOutput" class="compact-list"></div>
        </section>
      </div>
    </details>
  </section>
</main>
```

- [x] **Step 2: Ensure all existing DOM IDs remain available**

Confirm these IDs still exist in `public/index.html`: `releaseDirInput`, `releaseDirText`, `versionInput`, `refreshButton`, `loadButton`, `overallStatusText`, `overallStatusBadge`, `currentStepEyebrow`, `currentStepTitle`, `currentStepDescription`, `currentStepStatus`, `primaryActionButton`, `focusSuggestions`, `stepDetailContent`, `toggleLogsButton`, `actionLogOutput`, `configSummary`, `manifestList`, `artifactList`, `commandOutput`.

## Task 2: Update the Frontend View Model for Assistant Presentation

**Files:**
- Modify: `public/app.js`
- Test: `node --check public/app.js`

- [x] **Step 1: Replace `wizardSteps` with product progress items**

Use five visible progress items and keep publishing as complete-state copy only:

```js
const progressSteps = [
  { key: 'preflight', label: '预检' },
  { key: 'artifactCheck', label: '产物' },
  { key: 'manifest', label: 'Manifest' },
  { key: 'verify', label: '验证' },
  { key: 'dryRunRelease', label: 'Dry-run' },
];
```

- [x] **Step 2: Add new DOM references**

Add references for `progressRail`, `currentStepNumber`, and `detailsSummaryText`.

- [x] **Step 3: Update `resolveCurrentStep` copy**

Return product copy matching the spec:

```js
return {
  key: 'preflight',
  label: '先确认发布环境',
  status: steps.preflight?.status || 'not_started',
  description: '检查源码版本、凭据、4090 连接和 updater 端点。',
  action: 'preflight',
};
```

Apply equivalent copy for `context`, `artifactCheck`, `verify`, `dryRunRelease`, and `complete`.

- [x] **Step 4: Add `primarySuggestion` derivation**

Derive one main suggestion from latest action suggestions, state suggestions, missing context, and complete-state copy:

```js
function primarySuggestionFor(currentStep, state, lastAction) {
  if (currentStep.key === 'context') {
    if (!elements.versionInput.value.trim()) return { severity: 'warning', message: '先填写目标版本。' };
    if (!elements.releaseDirInput.value.trim() && !appState.summary?.releaseDir) return { severity: 'warning', message: '先选择或读取 release 目录。' };
  }
  if (currentStep.key === 'complete') {
    return { severity: 'info', message: '真实发布将在 Phase 2 启用。' };
  }
  const suggestions = [...(lastAction?.suggestions || []), ...(state.suggestions || [])];
  return suggestions.find((item) => item.severity === 'error') || suggestions[0] || { severity: 'info', message: '当前没有阻塞。按主按钮继续。' };
}
```

- [x] **Step 5: Render quiet progress rail**

Replace `renderStepper` with `renderProgressRail` that renders thin segments and labels:

```js
function renderProgressRail(items) {
  elements.progressRail.innerHTML = items.map((item) => `
    <div class="progress-item ${item.active ? 'active' : ''}">
      <span class="progress-segment ${toneForStatus(item.status)}"></span>
      <span>${item.label}</span>
    </div>
  `).join('');
}
```

- [x] **Step 6: Render only one primary suggestion in the card**

Update `renderCurrentStep` so `focusSuggestions` contains a single `.focus-suggestion` article.

- [x] **Step 7: Preserve detail rendering and secondary summaries**

Keep `renderChecks`, `renderArtifacts`, `renderVerifyResults`, `renderCommands`, `renderConfig`, `renderManifestSummary`, and `renderFullArtifacts`, but call them inside the collapsed details drawer.

## Task 3: Replace Dashboard CSS With Assistant Styling

**Files:**
- Modify: `public/styles.css`
- Test: browser screenshot review

- [x] **Step 1: Remove the decorative radial background and heavy card system**

Use a quiet neutral page:

```css
body {
  margin: 0;
  min-height: 100vh;
  background: #f4f6f9;
  color: var(--text);
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
}
```

- [x] **Step 2: Add compact release bar styling**

Create `.assistant-shell`, `.release-bar`, `.release-meta`, `.release-state`, `.context-field`, and `.release-path` styles with restrained borders and no large hero typography.

- [x] **Step 3: Add centered assistant card styling**

Create `.assistant-stage`, `.assistant-card`, and `.step-orb` styles. The card should be centered, white, lightly bordered, and no larger than 680px.

- [x] **Step 4: Add quiet progress rail styling**

Create `.progress-rail`, `.progress-item`, and `.progress-segment` styles. The rail should use thin 6-8px segments rather than button cards.

- [x] **Step 5: Add collapsed details styling**

Create `.assistant-details details`, `.assistant-details summary`, and preserve detail row, command card, compact list, and action log styling in a lower visual weight.

- [x] **Step 6: Add responsive rules**

At widths below 900px, stack the release bar and context controls. At widths below 560px, reduce shell padding, card padding, and make buttons full width where needed.

## Task 4: Verify Behavior and Visual Acceptance

**Files:**
- Test only

- [x] **Step 1: Run syntax checks**

Run:

```bash
node --check public/app.js
npm run check
```

Expected: both commands exit 0.

- [x] **Step 2: Start the local server**

Run:

```bash
npm run dev
```

Expected: server prints `OMD 发布控制台已启动：http://127.0.0.1:4177`.

- [x] **Step 3: Inspect desktop and mobile screenshots**

Open `http://127.0.0.1:4177` in a browser and capture:

- desktop around 1440px width
- mobile around 390px width

Acceptance:

- one dominant current step card
- one primary CTA
- no six-card stepper
- details are collapsed or visually secondary
- no visible text overlap
- Phase 2 publishing remains disabled/future copy only

- [x] **Step 4: Stop the local server**

Stop the dev server after verification.
