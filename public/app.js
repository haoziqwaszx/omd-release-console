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
  commands: [],
  lastAction: null,
  loadingAction: '',
};

await loadConfig();
await loadSummary();
await loadCommands();

if (elements.refreshButton) elements.refreshButton.addEventListener('click', () => loadSummary(elements.releaseDirInput.value.trim()));
if (elements.loadButton) elements.loadButton.addEventListener('click', () => loadSummary(elements.releaseDirInput.value.trim()));
if (elements.primaryActionButton) elements.primaryActionButton.addEventListener('click', () => runPrimaryAction());
if (elements.versionInput) elements.versionInput.addEventListener('change', () => loadCommands(elements.versionInput.value.trim()));
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
if (elements.stepDetailContent) {
  elements.stepDetailContent.addEventListener('click', async (event) => {
    const button = event.target.closest('button[data-copy-raw]');
    if (!button) return;
    await navigator.clipboard.writeText(decodeURIComponent(button.dataset.copyRaw));
    button.textContent = '已复制';
  });
}

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

function buildReleaseViewModel(summary, config, lastAction) {
  const state = summary?.state || {};
  const currentStep = resolveCurrentStep(state, summary);
  const stepStatuses = mapStepStatus(state.steps || {}, currentStep.key);
  const suggestions = topSuggestions(lastAction?.suggestions || state.suggestions || [], 3);
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
  if (currentStep.key === 'context') return { label: '未开始', tone: 'neutral' };
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

function renderApp() {
  if (!elements.overallStatusText) return;
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

    const data = await fetchAction(`/api/actions/${action}`, {
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

async function fetchAction(url, options) {
  const response = await fetch(url, options);
  const data = await response.json();
  if (!response.ok) {
    throw new Error(data.message || data.error || `请求失败：${response.status}`);
  }
  return data;
}

async function fetchJson(url, options) {
  const response = await fetch(url, options);
  const data = await response.json();
  if (!response.ok || data.ok === false) {
    throw new Error(data.message || data.error || `请求失败：${response.status}`);
  }
  return data;
}
