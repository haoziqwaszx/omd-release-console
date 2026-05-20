# Phase 1.5 Productized Release Wizard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rework the Phase 1 frontend into a single-screen Stepper release wizard that focuses the user on one current step and one primary action.

**Architecture:** Keep the existing Node server and Phase 1 APIs unchanged. Replace the flat multi-panel frontend with a productized vanilla HTML/CSS/JS view model layer, a unified release header, a compact stepper, a current-step focus card, contextual details, and secondary collapsed details.

**Tech Stack:** Vanilla HTML, CSS, JavaScript modules, existing Node.js built-in server.

---

## File Structure

- Modify `public/index.html`: replace the flat dashboard layout with release wizard regions: header, stepper, focus card, details, secondary details.
- Modify `public/app.js`: add a small view model layer that derives current step, primary action, stepper state, suggestions, and contextual details from existing API data.
- Modify `public/styles.css`: replace scattered panel styling with a unified product visual system for a professional release wizard.
- No backend files should be changed for Phase 1.5 unless verification reveals a frontend-blocking bug.

## Task 1: Replace Flat Markup with Wizard Structure

**Files:**
- Modify: `public/index.html`
- Test: `node --check public/app.js` after Task 2

- [ ] **Step 1: Replace the body layout with wizard regions**

Replace the contents of `<body>` in `public/index.html` with this structure:

```html
<body>
  <main class="wizard-shell">
    <section class="release-hero" aria-labelledby="pageTitle">
      <div class="release-title-block">
        <p class="eyebrow">OMD Release Console</p>
        <h1 id="pageTitle">安全可控的 OMD 双端发布向导</h1>
        <p class="hero-copy">按步骤完成预检、产物确认、Manifest、线上验证和 dry-run。真实发布将在 Phase 2 启用。</p>
      </div>
      <div class="release-status-card">
        <span class="status-label">整体状态</span>
        <strong id="overallStatusText">未开始</strong>
        <span id="overallStatusBadge" class="status-pill neutral">未开始</span>
      </div>
    </section>

    <section class="release-context" aria-label="发布上下文">
      <label class="field-group" for="versionInput">
        <span>目标版本</span>
        <input id="versionInput" value="0.0.7" placeholder="例如 0.0.7" />
      </label>
      <label class="field-group release-dir-field" for="releaseDirInput">
        <span>Release 目录</span>
        <input id="releaseDirInput" placeholder="默认读取桌面最新 omd-*-release-* 目录" />
      </label>
      <div class="context-actions">
        <button id="loadButton" class="secondary-button" type="button">读取目录</button>
        <button id="refreshButton" class="secondary-button" type="button">刷新</button>
      </div>
      <p id="releaseDirText" class="context-hint">正在读取...</p>
    </section>

    <section class="wizard-card" aria-label="发布步骤向导">
      <nav id="stepperList" class="stepper-list" aria-label="发布步骤"></nav>

      <article id="currentStepCard" class="current-step-card">
        <div>
          <p id="currentStepEyebrow" class="eyebrow">当前步骤</p>
          <h2 id="currentStepTitle">准备发布信息</h2>
          <p id="currentStepDescription" class="step-description">填写目标版本和 release 目录后开始发布准备。</p>
        </div>
        <div class="step-status-row">
          <span id="currentStepStatus" class="status-pill neutral">未开始</span>
          <button id="primaryActionButton" class="primary-button" type="button">确认发布信息</button>
        </div>
        <div id="focusSuggestions" class="focus-suggestions"></div>
      </article>

      <section class="step-details" aria-labelledby="detailsTitle">
        <div class="details-heading">
          <div>
            <p class="eyebrow">Step Details</p>
            <h3 id="detailsTitle">当前步骤详情</h3>
          </div>
          <button id="toggleLogsButton" class="ghost-button" type="button">显示日志</button>
        </div>
        <div id="stepDetailContent" class="detail-content"></div>
        <pre id="actionLogOutput" class="action-log hidden">暂无动作日志</pre>
      </section>
    </section>

    <section class="secondary-details" aria-label="辅助信息">
      <details>
        <summary>辅助信息：配置、Manifest、产物和命令</summary>
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
  <script type="module" src="/app.js"></script>
</body>
```

- [ ] **Step 2: Remove obsolete IDs from markup**

Ensure these old regions no longer exist in `public/index.html`:

```html
<nav class="nav-list">
<section id="actions">
<section id="flow">
<div id="checkpointList">
<div id="suggestionList">
```

The new UI should use `stepperList`, `currentStepCard`, `focusSuggestions`, and `stepDetailContent` instead.

## Task 2: Add Release Wizard View Model

**Files:**
- Modify: `public/app.js`
- Test: `node --check public/app.js`

- [ ] **Step 1: Replace global metadata and elements**

Replace the top of `public/app.js` through the `elements` object with:

```js
const wizardSteps = [
  { key: 'preflight', label: '预检', action: 'preflight', description: '确认源码版本、凭据、SSH 和 updater 端点是否满足发版条件。' },
  { key: 'artifactCheck', label: '产物检查', action: 'manifest', description: '确认 Windows 与 Mac 必需产物和签名文件是否齐全。' },
  { key: 'manifest', label: 'Manifest', action: 'manifest', description: '生成 checksums、GitHub latest.json 和 Gitee latest.json。' },
  { key: 'verify', label: '线上验证', action: 'verify', description: '验证 updater endpoint、manifest 版本和下载 URL。' },
  { key: 'dryRunRelease', label: 'Dry-run', action: 'dry-run-release', description: '生成真实发布命令但不执行上传。' },
  { key: 'publish', label: '发布', action: '', description: '真实 GitHub/Gitee 发布将在 Phase 2 启用。', disabled: true },
];

const elements = {
  releaseDirInput: document.querySelector('#releaseDirInput'),
  releaseDirText: document.querySelector('#releaseDirText'),
  versionInput: document.querySelector('#versionInput'),
  refreshButton: document.querySelector('#refreshButton'),
  loadButton: document.querySelector('#loadButton'),
  overallStatusText: document.querySelector('#overallStatusText'),
  overallStatusBadge: document.querySelector('#overallStatusBadge'),
  stepperList: document.querySelector('#stepperList'),
  currentStepEyebrow: document.querySelector('#currentStepEyebrow'),
  currentStepTitle: document.querySelector('#currentStepTitle'),
  currentStepDescription: document.querySelector('#currentStepDescription'),
  currentStepStatus: document.querySelector('#currentStepStatus'),
  primaryActionButton: document.querySelector('#primaryActionButton'),
  focusSuggestions: document.querySelector('#focusSuggestions'),
  stepDetailContent: document.querySelector('#stepDetailContent'),
  toggleLogsButton: document.querySelector('#toggleLogsButton'),
  actionLogOutput: document.querySelector('#actionLogOutput'),
  configSummary: document.querySelector('#configSummary'),
  manifestList: document.querySelector('#manifestList'),
  artifactList: document.querySelector('#artifactList'),
  commandOutput: document.querySelector('#commandOutput'),
};

const appState = {
  summary: null,
  config: null,
  lastAction: null,
  loadingAction: '',
};
```

- [ ] **Step 2: Replace startup and event listeners**

Replace the initial render and event listener block with:

```js
await loadConfig();
await loadSummary();
await loadCommands();

if (elements.refreshButton) elements.refreshButton.addEventListener('click', () => loadSummary(elements.releaseDirInput.value.trim()));
if (elements.loadButton) elements.loadButton.addEventListener('click', () => loadSummary(elements.releaseDirInput.value.trim()));
if (elements.primaryActionButton) elements.primaryActionButton.addEventListener('click', () => runPrimaryAction());
if (elements.toggleLogsButton) {
  elements.toggleLogsButton.addEventListener('click', () => {
    elements.actionLogOutput.classList.toggle('hidden');
    elements.toggleLogsButton.textContent = elements.actionLogOutput.classList.contains('hidden') ? '显示日志' : '隐藏日志';
  });
}
if (elements.commandOutput) {
  elements.commandOutput.addEventListener('click', async (event) => {
    const button = event.target.closest('button[data-copy-raw]');
    if (!button) return;
    await navigator.clipboard.writeText(decodeURIComponent(button.dataset.copyRaw));
    button.textContent = '已复制';
  });
}
```

- [ ] **Step 3: Add view model functions**

Add these functions after API loading functions:

```js
function buildReleaseViewModel(summary, config, lastAction) {
  const state = summary?.state || {};
  const currentStep = resolveCurrentStep(state, summary);
  const stepStatuses = mapStepStatus(state.steps || {}, currentStep.key);
  const suggestions = topSuggestions(state.suggestions || [], 3);
  return {
    summary,
    config,
    lastAction,
    currentStep,
    primaryAction: resolvePrimaryAction(currentStep),
    stepStatuses,
    suggestions,
    overall: resolveOverallStatus(currentStep, state),
  };
}

function resolveCurrentStep(state, summary) {
  const steps = state?.steps || {};
  const hasContext = Boolean(elements.versionInput.value.trim() && (elements.releaseDirInput.value.trim() || summary?.releaseDir));
  if (!hasContext) {
    return { key: 'context', label: '准备发布信息', status: 'not_started', description: '填写目标版本和 release 目录后开始发布准备。', action: '' };
  }
  if (steps.preflight?.status !== 'success') {
    return { ...wizardSteps[0], status: steps.preflight?.status || 'not_started' };
  }
  if (steps.artifactCheck?.status !== 'success' || steps.manifestGithub?.status !== 'success' || steps.manifestGitee?.status !== 'success') {
    return { key: 'artifactCheck', label: '产物与 Manifest', action: 'manifest', status: firstBlockingStatus([steps.artifactCheck, steps.manifestGithub, steps.manifestGitee]), description: '确认产物完整后生成 GitHub/Gitee manifest。' };
  }
  if (steps.verify?.status !== 'success') {
    return { ...wizardSteps[3], status: steps.verify?.status || 'not_started' };
  }
  if (steps.dryRunRelease?.status !== 'success') {
    return { ...wizardSteps[4], status: steps.dryRunRelease?.status || 'not_started' };
  }
  return { key: 'complete', label: '准备完成', status: 'success', action: 'copyCommands', description: '发布准备已完成，可以复制 dry-run 输出的发布命令。' };
}

function resolvePrimaryAction(currentStep) {
  if (currentStep.key === 'context') return { label: '确认发布信息', action: 'refresh', disabled: false };
  if (currentStep.key === 'complete') return { label: '复制发布命令', action: 'copyCommands', disabled: false };
  if (!currentStep.action) return { label: 'Phase 2 启用', action: '', disabled: true };
  const retry = currentStep.status === 'failed' ? '重新运行' : '运行';
  return { label: `${retry}${currentStep.label}`, action: currentStep.action, disabled: false };
}

function mapStepStatus(steps, currentKey) {
  return wizardSteps.map((step) => {
    if (step.disabled) return { ...step, status: 'disabled', active: currentKey === step.key };
    if (step.key === 'manifest') {
      const manifestDone = steps.manifestGithub?.status === 'success' && steps.manifestGitee?.status === 'success';
      const manifestFailed = steps.manifestGithub?.status === 'failed' || steps.manifestGitee?.status === 'failed';
      return { ...step, status: manifestDone ? 'success' : manifestFailed ? 'failed' : 'not_started', active: currentKey === step.key || currentKey === 'artifactCheck' };
    }
    return { ...step, status: steps[step.key]?.status || 'not_started', active: currentKey === step.key };
  });
}

function resolveOverallStatus(currentStep, state) {
  if (currentStep.status === 'failed' || state?.overallStatus === 'failed') return { label: '有阻塞', tone: 'danger' };
  if (currentStep.key === 'complete') return { label: '已完成', tone: 'success' };
  if (currentStep.status === 'running') return { label: '正在执行', tone: 'warning' };
  return { label: '可继续', tone: 'success' };
}

function firstBlockingStatus(items) {
  if (items.some((item) => item?.status === 'failed')) return 'failed';
  if (items.some((item) => item?.status === 'running')) return 'running';
  return 'not_started';
}

function topSuggestions(suggestions, max) {
  return suggestions.slice(0, max);
}
```

## Task 3: Render Wizard UI from View Model

**Files:**
- Modify: `public/app.js`
- Test: `node --check public/app.js`

- [ ] **Step 1: Replace API loading functions**

Use these implementations:

```js
async function loadSummary(releaseDir = '') {
  const query = releaseDir ? `?releaseDir=${encodeURIComponent(releaseDir)}` : '';
  appState.summary = await fetchJson(`/api/summary${query}`);
  if (appState.summary.releaseDir && !elements.releaseDirInput.value) {
    elements.releaseDirInput.value = appState.summary.releaseDir;
  }
  renderApp();
}

async function loadConfig() {
  const data = await fetchJson('/api/config');
  appState.config = data.config;
  renderApp();
}

async function loadCommands(version = '0.0.7') {
  const data = await fetchJson(`/api/commands?version=${encodeURIComponent(version || '0.0.7')}`);
  appState.commands = data.commands;
  renderApp();
}
```

- [ ] **Step 2: Add `renderApp` and header rendering**

Add:

```js
function renderApp() {
  const viewModel = buildReleaseViewModel(appState.summary, appState.config, appState.lastAction);
  renderHeader(viewModel);
  renderStepper(viewModel.stepStatuses);
  renderCurrentStep(viewModel);
  renderStepDetails(viewModel);
  renderSecondaryDetails(viewModel);
}

function renderHeader(viewModel) {
  elements.releaseDirText.textContent = viewModel.summary?.releaseDir || viewModel.summary?.message || '没有发布目录';
  elements.overallStatusText.textContent = viewModel.overall.label;
  elements.overallStatusBadge.textContent = viewModel.overall.label;
  elements.overallStatusBadge.className = `status-pill ${viewModel.overall.tone}`;
}
```

- [ ] **Step 3: Add stepper and focus card rendering**

Add:

```js
function renderStepper(steps) {
  elements.stepperList.innerHTML = steps.map((step, index) => `
    <button class="stepper-item ${step.active ? 'active' : ''}" type="button" disabled>
      <span class="step-number">${index + 1}</span>
      <span>
        <strong>${step.label}</strong>
        <small>${statusLabel(step.status)}</small>
      </span>
      <span class="step-dot ${toneForStatus(step.status)}"></span>
    </button>
  `).join('');
}

function renderCurrentStep(viewModel) {
  const { currentStep, primaryAction, suggestions } = viewModel;
  elements.currentStepEyebrow.textContent = currentStep.key === 'complete' ? '准备完成' : '当前步骤';
  elements.currentStepTitle.textContent = currentStep.label;
  elements.currentStepDescription.textContent = currentStep.description;
  elements.currentStepStatus.textContent = statusLabel(currentStep.status);
  elements.currentStepStatus.className = `status-pill ${toneForStatus(currentStep.status)}`;
  elements.primaryActionButton.textContent = appState.loadingAction ? '执行中...' : primaryAction.label;
  elements.primaryActionButton.disabled = primaryAction.disabled || Boolean(appState.loadingAction);
  elements.primaryActionButton.dataset.action = primaryAction.action;
  elements.focusSuggestions.innerHTML = suggestions.length
    ? suggestions.map((item) => `<article class="focus-suggestion ${item.severity === 'error' ? 'danger' : 'warning'}"><strong>${item.severity === 'error' ? '阻塞' : '建议'}</strong><span>${item.message}</span></article>`).join('')
    : `<p class="empty-state">当前没有阻塞建议。按主按钮继续。</p>`;
}
```

- [ ] **Step 4: Add step detail rendering**

Add:

```js
function renderStepDetails(viewModel) {
  const state = viewModel.summary?.state || {};
  const currentKey = viewModel.currentStep.key;
  if (currentKey === 'preflight') {
    renderChecks(state.checks || []);
    return;
  }
  if (currentKey === 'artifactCheck' || currentKey === 'manifest') {
    renderArtifacts(viewModel.summary?.requiredArtifacts || [], viewModel.summary?.artifacts || []);
    return;
  }
  if (currentKey === 'verify') {
    renderVerifyResults(state.manifests?.onlineVerify?.results || []);
    return;
  }
  if (currentKey === 'dryRunRelease' || currentKey === 'complete') {
    renderCommands(appState.commands || []);
    return;
  }
  elements.stepDetailContent.innerHTML = `<p class="empty-state">填写发布信息后开始。</p>`;
}

function renderChecks(checks) {
  elements.stepDetailContent.innerHTML = checks.length ? checks.map((check) => `
    <article class="detail-row">
      <span class="status-dot ${toneForStatus(check.status)}"></span>
      <div>
        <strong>${check.label}</strong>
        <p>${check.message}</p>
      </div>
    </article>
  `).join('') : `<p class="empty-state">还没有检查结果。</p>`;
}

function renderArtifacts(requiredArtifacts, actualArtifacts) {
  const requiredKeys = new Set(requiredArtifacts.map((artifact) => `${artifact.scope}/${artifact.name}`));
  const rows = requiredArtifacts.length
    ? [...requiredArtifacts, ...actualArtifacts.filter((artifact) => !requiredKeys.has(`${artifact.scope}/${artifact.name}`))]
    : actualArtifacts;
  elements.stepDetailContent.innerHTML = rows.length ? rows.map((artifact) => `
    <article class="detail-row">
      <span class="status-dot ${artifact.exists === false ? 'danger' : 'success'}"></span>
      <div>
        <strong>${artifact.name}</strong>
        <p>${artifact.scope}${artifact.signatureFor ? ` · 签名：${artifact.signatureFor}` : ''} · ${artifact.exists === false ? '缺失' : formatSize(artifact.size)}</p>
      </div>
    </article>
  `).join('') : `<p class="empty-state">还没有产物信息。</p>`;
}

function renderVerifyResults(results) {
  elements.stepDetailContent.innerHTML = results.length ? results.map((result) => `
    <article class="detail-row">
      <span class="status-dot ${toneForStatus(result.status)}"></span>
      <div>
        <strong>${result.endpoint}</strong>
        <p>${result.status === 'success' ? `版本 ${result.version} · ${result.platforms.length} 个平台` : result.error}</p>
      </div>
    </article>
  `).join('') : `<p class="empty-state">还没有线上验证结果。</p>`;
}

function renderCommands(commands) {
  elements.stepDetailContent.innerHTML = commands.length ? commands.map((command) => commandCard(command)).join('') : `<p class="empty-state">还没有命令。运行 dry-run 后查看命令。</p>`;
}
```

- [ ] **Step 5: Add secondary detail rendering**

Add:

```js
function renderSecondaryDetails(viewModel) {
  renderConfig(viewModel.config || {});
  renderManifestSummary(viewModel.summary?.manifests || {});
  renderFullArtifacts(viewModel.summary?.requiredArtifacts || [], viewModel.summary?.artifacts || []);
  elements.commandOutput.innerHTML = (appState.commands || []).map((command) => commandCard(command)).join('') || `<p class="empty-state">暂无命令</p>`;
}

function renderConfig(config) {
  const rows = [
    ['Source Repo', config.sourceRepo],
    ['GitHub Repo', config.githubRepo],
    ['Gitee Repo', config.giteeOwner && config.giteeRepo ? `${config.giteeOwner}/${config.giteeRepo}` : ''],
    ['Windows SSH', config.windowsSsh],
    ['Proxy', config.githubProxy],
  ];
  elements.configSummary.innerHTML = rows.map(([label, value]) => `<article class="compact-row"><span>${label}</span><strong>${value || '-'}</strong></article>`).join('');
}

function renderManifestSummary(manifests) {
  elements.manifestList.innerHTML = ['github', 'gitee'].map((host) => {
    const manifest = manifests[host] || {};
    return `<article class="compact-row"><span>${hostLabel(host)}</span><strong>${manifest.exists ? manifest.version : '缺失'}</strong></article>`;
  }).join('');
}

function renderFullArtifacts(requiredArtifacts, actualArtifacts) {
  const rows = requiredArtifacts.length ? requiredArtifacts : actualArtifacts;
  elements.artifactList.innerHTML = rows.map((artifact) => `<article class="compact-row"><span>${artifact.scope}</span><strong>${artifact.name}</strong></article>`).join('') || `<p class="empty-state">暂无产物</p>`;
}
```

## Task 4: Wire Primary Action and Shared Helpers

**Files:**
- Modify: `public/app.js`
- Test: `node --check public/app.js`

- [ ] **Step 1: Add primary action runner**

Add:

```js
async function runPrimaryAction() {
  const action = elements.primaryActionButton.dataset.action;
  if (action === 'refresh') {
    await loadSummary(elements.releaseDirInput.value.trim());
    return;
  }
  if (action === 'copyCommands') {
    const command = (appState.commands || []).join('\n');
    await navigator.clipboard.writeText(command);
    elements.primaryActionButton.textContent = '已复制命令';
    return;
  }
  if (!action) return;
  await runAction(action);
}

async function runAction(action) {
  try {
    const version = elements.versionInput.value.trim();
    const releaseDir = elements.releaseDirInput.value.trim();
    appState.loadingAction = action;
    renderApp();

    const data = await fetchJson(`/api/actions/${action}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ version, releaseDir }),
    });

    appState.lastAction = data;
    elements.actionLogOutput.textContent = [data.logs?.stdout, data.logs?.stderr].filter(Boolean).join('\n') || data.message;
    await loadSummary(releaseDir);
  } catch (error) {
    appState.lastAction = { ok: false, suggestions: [{ severity: 'error', message: error.message, action }] };
    elements.actionLogOutput.textContent = error.message;
    renderApp();
  } finally {
    appState.loadingAction = '';
    renderApp();
  }
}
```

- [ ] **Step 2: Add shared display helpers**

Add or keep these helpers at the bottom of `public/app.js`:

```js
function commandCard(command) {
  const highRisk = / release \d+\.\d+\.\d+(?!.*--dry-run)/.test(command);
  return `
    <article class="command-card">
      <header>
        <strong>${commandName(command)}</strong>
        <span class="status-pill ${highRisk ? 'danger' : 'success'}">${highRisk ? '高风险' : '低风险'}</span>
      </header>
      <code>${escapeHtml(command)}</code>
      <button class="secondary-button" type="button" data-copy-raw="${encodeURIComponent(command)}">复制</button>
    </article>
  `;
}

function hostLabel(host) {
  return host === 'github' ? 'GitHub' : 'Gitee';
}

function commandName(command) {
  if (command.includes(' preflight ')) return '预检';
  if (command.includes(' --dry-run')) return 'Dry-run 发布';
  if (command.includes(' verify ')) return '验证';
  if (command.includes(' release ')) return '真实发布';
  if (command.includes(' manifest ')) return '生成 Manifest';
  return '计划';
}

function toneForStatus(status) {
  return {
    success: 'success',
    running: 'warning',
    failed: 'danger',
    disabled: 'neutral',
    not_started: 'neutral',
  }[status] || 'neutral';
}

function statusLabel(status) {
  return {
    not_started: '未开始',
    running: '正在执行',
    success: '已完成',
    failed: '有阻塞',
    skipped: '已跳过',
    manual_required: '需人工处理',
    disabled: '后续阶段',
  }[status] || status || '未开始';
}

function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function escapeHtml(value) {
  return value.replace(/[&<>"]/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[char]));
}

async function fetchJson(url, options) {
  const response = await fetch(url, options);
  const data = await response.json();
  if (!response.ok || data.ok === false) {
    throw new Error(data.message || data.error || `请求失败：${response.status}`);
  }
  return data;
}
```

- [ ] **Step 3: Run JS syntax check**

Run:

```bash
node --check public/app.js
```

Expected: exits successfully with no output.

## Task 5: Replace Visual System CSS

**Files:**
- Modify: `public/styles.css`
- Test: browser visual check

- [ ] **Step 1: Replace CSS with unified release wizard styles**

Replace `public/styles.css` with:

```css
:root {
  --bg: #eef2f7;
  --surface: #ffffff;
  --surface-soft: #f8fafc;
  --text: #0f172a;
  --muted: #64748b;
  --border: #dbe3ee;
  --primary: #16a34a;
  --primary-dark: #15803d;
  --warning: #d97706;
  --danger: #dc2626;
  --neutral: #94a3b8;
  --shadow: 0 24px 80px rgba(15, 23, 42, 0.10);
}

* { box-sizing: border-box; }

body {
  margin: 0;
  min-height: 100vh;
  background: radial-gradient(circle at top left, #dff7ea 0, transparent 30%), var(--bg);
  color: var(--text);
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
}

button,
input { font: inherit; }

button {
  min-height: 44px;
  border-radius: 12px;
  cursor: pointer;
  transition: border-color 180ms ease, background 180ms ease, color 180ms ease, box-shadow 180ms ease;
}

button:focus-visible,
input:focus-visible,
summary:focus-visible {
  outline: 3px solid rgba(22, 163, 74, 0.28);
  outline-offset: 2px;
}

button:disabled {
  cursor: not-allowed;
  opacity: 0.62;
}

input {
  width: 100%;
  min-height: 44px;
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 0.65rem 0.8rem;
  color: var(--text);
  background: white;
}

.wizard-shell {
  width: min(1180px, calc(100% - 32px));
  margin: 0 auto;
  padding: 32px 0 48px;
  display: grid;
  gap: 18px;
}

.release-hero,
.release-context,
.wizard-card,
.secondary-details {
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.92);
  border-radius: 24px;
  box-shadow: var(--shadow);
}

.release-hero {
  display: flex;
  justify-content: space-between;
  gap: 24px;
  padding: 28px;
}

.release-title-block h1 {
  margin: 0;
  font-size: clamp(2rem, 4vw, 3.3rem);
  letter-spacing: -0.04em;
}

.hero-copy {
  max-width: 720px;
  margin: 12px 0 0;
  color: var(--muted);
  font-size: 1rem;
  line-height: 1.65;
}

.eyebrow {
  margin: 0 0 8px;
  color: var(--primary-dark);
  font-size: 0.74rem;
  font-weight: 800;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.release-status-card {
  min-width: 180px;
  align-self: stretch;
  display: grid;
  align-content: center;
  justify-items: start;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: 20px;
  padding: 18px;
  background: var(--surface-soft);
}

.status-label,
.field-group span {
  color: var(--muted);
  font-size: 0.82rem;
  font-weight: 700;
}

.release-status-card strong {
  font-size: 1.45rem;
}

.release-context {
  display: grid;
  grid-template-columns: 180px minmax(0, 1fr) auto;
  gap: 14px;
  align-items: end;
  padding: 18px;
}

.field-group {
  display: grid;
  gap: 7px;
}

.context-actions {
  display: flex;
  gap: 10px;
}

.context-hint {
  grid-column: 1 / -1;
  margin: 0;
  color: var(--muted);
  overflow-wrap: anywhere;
}

.wizard-card {
  padding: 22px;
  display: grid;
  gap: 22px;
}

.stepper-list {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 10px;
}

.stepper-item {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  gap: 10px;
  align-items: center;
  border: 1px solid var(--border);
  background: var(--surface-soft);
  color: var(--text);
  padding: 12px;
  text-align: left;
}

.stepper-item.active {
  border-color: rgba(22, 163, 74, 0.45);
  background: #f0fdf4;
  box-shadow: 0 10px 30px rgba(22, 163, 74, 0.10);
}

.step-number {
  width: 28px;
  height: 28px;
  display: inline-grid;
  place-items: center;
  border-radius: 999px;
  background: white;
  color: var(--muted);
  font-weight: 800;
}

.stepper-item strong,
.stepper-item small {
  display: block;
}

.stepper-item small {
  margin-top: 3px;
  color: var(--muted);
}

.step-dot,
.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  background: var(--neutral);
}

.current-step-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 22px;
  align-items: start;
  border: 1px solid #bbf7d0;
  border-radius: 24px;
  padding: 28px;
  background: linear-gradient(135deg, #ffffff 0%, #f0fdf4 100%);
}

.current-step-card h2 {
  margin: 0;
  font-size: clamp(1.75rem, 3vw, 2.5rem);
  letter-spacing: -0.035em;
}

.step-description {
  max-width: 680px;
  margin: 12px 0 0;
  color: var(--muted);
  font-size: 1rem;
  line-height: 1.65;
}

.step-status-row {
  display: grid;
  gap: 12px;
  justify-items: end;
}

.primary-button,
.secondary-button,
.ghost-button {
  border: 1px solid transparent;
  padding: 0.7rem 1rem;
  font-weight: 800;
}

.primary-button {
  min-width: 180px;
  background: var(--primary);
  color: white;
  box-shadow: 0 14px 30px rgba(22, 163, 74, 0.22);
}

.primary-button:hover { background: var(--primary-dark); }

.secondary-button {
  border-color: var(--border);
  background: white;
  color: var(--text);
}

.secondary-button:hover,
.ghost-button:hover {
  border-color: rgba(22, 163, 74, 0.35);
  background: #f0fdf4;
}

.ghost-button {
  border-color: transparent;
  background: transparent;
  color: var(--muted);
}

.focus-suggestions {
  grid-column: 1 / -1;
  display: grid;
  gap: 10px;
}

.focus-suggestion {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  border-radius: 14px;
  padding: 12px;
}

.focus-suggestion.danger { background: #fef2f2; color: #991b1b; }
.focus-suggestion.warning { background: #fffbeb; color: #92400e; }

.step-details {
  border: 1px solid var(--border);
  border-radius: 20px;
  padding: 18px;
  background: var(--surface);
}

.details-heading {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: start;
  margin-bottom: 14px;
}

.details-heading h3,
.secondary-details h3 {
  margin: 0;
}

.detail-content,
.compact-list {
  display: grid;
  gap: 10px;
}

.detail-row,
.compact-row,
.command-card {
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--surface-soft);
  padding: 12px;
}

.detail-row {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 12px;
  align-items: start;
}

.detail-row strong,
.compact-row strong {
  overflow-wrap: anywhere;
}

.detail-row p,
.empty-state {
  margin: 4px 0 0;
  color: var(--muted);
  overflow-wrap: anywhere;
}

.action-log {
  margin: 14px 0 0;
  padding: 14px;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: #111827;
  color: #e5e7eb;
  font-family: "SF Mono", ui-monospace, monospace;
  font-size: 0.82rem;
}

.hidden { display: none; }

.secondary-details {
  padding: 18px;
}

.secondary-details summary {
  cursor: pointer;
  font-weight: 800;
}

.secondary-grid {
  margin-top: 16px;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.compact-row {
  display: grid;
  gap: 4px;
}

.compact-row span {
  color: var(--muted);
  font-size: 0.8rem;
}

.command-card {
  display: grid;
  gap: 10px;
}

.command-card header {
  display: flex;
  justify-content: space-between;
  gap: 10px;
}

.command-card code {
  display: block;
  overflow-wrap: anywhere;
  border-radius: 10px;
  background: #111827;
  color: #e5e7eb;
  padding: 0.65rem;
  font-family: "SF Mono", ui-monospace, monospace;
  font-size: 0.78rem;
}

.status-pill {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  padding: 0.25rem 0.62rem;
  font-size: 0.76rem;
  font-weight: 900;
}

.status-pill.success,
.step-dot.success,
.status-dot.success { background: #dcfce7; color: #166534; }
.status-pill.warning,
.step-dot.warning,
.status-dot.warning { background: #fef3c7; color: #92400e; }
.status-pill.danger,
.step-dot.danger,
.status-dot.danger { background: #fee2e2; color: #991b1b; }
.status-pill.neutral,
.step-dot.neutral,
.status-dot.neutral { background: #e2e8f0; color: #475569; }

@media (max-width: 980px) {
  .release-hero,
  .current-step-card {
    grid-template-columns: 1fr;
  }

  .release-context,
  .secondary-grid {
    grid-template-columns: 1fr;
  }

  .step-status-row {
    justify-items: stretch;
  }

  .stepper-list {
    grid-template-columns: 1fr 1fr;
  }
}

@media (max-width: 560px) {
  .wizard-shell {
    width: min(100% - 20px, 1180px);
    padding: 16px 0 32px;
  }

  .release-hero,
  .wizard-card,
  .current-step-card {
    padding: 18px;
    border-radius: 18px;
  }

  .stepper-list {
    grid-template-columns: 1fr;
  }

  .context-actions {
    display: grid;
  }
}
```

## Task 6: Verification

**Files:**
- Modify only if verification exposes defects.
- Test: syntax, API, and browser visual/manual checks.

- [ ] **Step 1: Run syntax checks**

Run:

```bash
npm run check && node --check public/app.js
```

Expected: exits successfully.

- [ ] **Step 2: Start or reuse local server**

Run if no server is running:

```bash
PORT=4178 npm run dev
```

Expected: server starts at `http://127.0.0.1:4178`.

- [ ] **Step 3: Verify page contains wizard regions**

Run:

```bash
curl -sS http://127.0.0.1:4178/ | grep -E '发布向导|stepperList|currentStepCard'
```

Expected: output includes the hero title and wizard element IDs.

- [ ] **Step 4: Verify APIs still work**

Run:

```bash
curl -sS http://127.0.0.1:4178/api/config | python3 -m json.tool >/tmp/omd-config-ui-check.json
curl -sS 'http://127.0.0.1:4178/api/summary?releaseDir=/Users/glame/Desktop/omd-0.0.7-release-test' | python3 -m json.tool >/tmp/omd-summary-ui-check.json
```

Expected: both commands exit successfully.

- [ ] **Step 5: Manual browser validation**

Open `http://127.0.0.1:4178` and verify:

- The first screen has one hero/header, one context form, one stepper, one current-step card, and one primary CTA.
- The old flat dashboard panels are gone from the main flow.
- Running preflight with version `0.0.7` against the test release dir shows blocking suggestions in the current-step card.
- Required artifacts appear in step details when the current step is产物/Manifest, not as a competing top-level panel.
- Config, manifest, full artifacts, and commands are inside the auxiliary details area.
- At 375px, 768px, 1024px, and desktop width there is no horizontal scroll.

---

## Self-Review

- Spec coverage: single current step, single primary CTA, stepper flow, contextual details, secondary collapsed information, visual system, and responsive requirements are covered.
- Placeholder scan: no TBD/TODO placeholders; all code blocks and commands are concrete.
- Type consistency: element IDs, view model function names, status/tone names, and action names are consistent across tasks.
