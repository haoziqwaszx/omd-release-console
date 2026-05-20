const invoke = window.__TAURI_INTERNALS__?.invoke;

const progressSteps = [
  { key: 'preflight', label: '预检' },
  { key: 'artifactCheck', label: '产物' },
  { key: 'manifest', label: 'Manifest' },
  { key: 'verify', label: '验证' },
  { key: 'dryRunRelease', label: 'Dry-run' },
  { key: 'report', label: '报告' },
];

const elements = {
  releaseDirInput: document.querySelector('#releaseDirInput'),
  releaseDirText: document.querySelector('#releaseDirText'),
  versionInput: document.querySelector('#versionInput'),
  refreshButton: document.querySelector('#refreshButton'),
  latestDirButton: document.querySelector('#latestDirButton'),
  openDirButton: document.querySelector('#openDirButton'),
  loadButton: document.querySelector('#loadButton'),
  overallStatusText: document.querySelector('#overallStatusText'),
  overallStatusBadge: document.querySelector('#overallStatusBadge'),
  progressRail: document.querySelector('#progressRail'),
  currentStepNumber: document.querySelector('#currentStepNumber'),
  currentStepEyebrow: document.querySelector('#currentStepEyebrow'),
  currentStepTitle: document.querySelector('#currentStepTitle'),
  currentStepDescription: document.querySelector('#currentStepDescription'),
  currentStepStatus: document.querySelector('#currentStepStatus'),
  primaryActionButton: document.querySelector('#primaryActionButton'),
  focusSuggestions: document.querySelector('#focusSuggestions'),
  detailsSummaryText: document.querySelector('#detailsSummaryText'),
  stepDetailContent: document.querySelector('#stepDetailContent'),
  toggleLogsButton: document.querySelector('#toggleLogsButton'),
  actionLogOutput: document.querySelector('#actionLogOutput'),
  configSummary: document.querySelector('#configSummary'),
  manifestList: document.querySelector('#manifestList'),
  artifactList: document.querySelector('#artifactList'),
  commandOutput: document.querySelector('#commandOutput'),
  confirmVersionInput: document.querySelector('#confirmVersionInput'),
  publishGithubButton: document.querySelector('#publishGithubButton'),
  publishGiteeButton: document.querySelector('#publishGiteeButton'),
  historyList: document.querySelector('#historyList'),
};

const appState = {
  summary: null,
  config: null,
  commands: [],
  history: [],
  lastAction: null,
  loadingAction: '',
};

await loadConfig();
await loadSummary();
await loadCommands();
await loadHistory();

if (elements.refreshButton) elements.refreshButton.addEventListener('click', () => loadSummary(elements.releaseDirInput.value.trim()));
if (elements.loadButton) elements.loadButton.addEventListener('click', () => loadSummary(elements.releaseDirInput.value.trim()));
if (elements.latestDirButton) elements.latestDirButton.addEventListener('click', () => loadLatestReleaseDir());
if (elements.openDirButton) elements.openDirButton.addEventListener('click', () => openReleaseDir());
if (!invoke) document.querySelectorAll('.desktop-only').forEach((element) => element.hidden = true);
if (elements.primaryActionButton) elements.primaryActionButton.addEventListener('click', () => runPrimaryAction());
if (elements.publishGithubButton) elements.publishGithubButton.addEventListener('click', () => runAction('publish-github'));
if (elements.publishGiteeButton) elements.publishGiteeButton.addEventListener('click', () => runAction('publish-gitee'));
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
  appState.summary = invoke
    ? await invoke('tauri_summary', { releaseDir: releaseDir || null })
    : await fetchJson(releaseDir ? `/api/summary?releaseDir=${encodeURIComponent(releaseDir)}` : '/api/summary');
  if (appState.summary.releaseDir && !elements.releaseDirInput.value) {
    elements.releaseDirInput.value = appState.summary.releaseDir;
  }
  renderApp();
}

async function loadConfig() {
  const data = invoke ? await invoke('tauri_config') : await fetchJson('/api/config');
  appState.config = data.config;
  renderApp();
}

async function loadCommands(version = '0.0.7') {
  const data = invoke
    ? await invoke('tauri_commands', { version: version || '0.0.7' })
    : await fetchJson(`/api/commands?version=${encodeURIComponent(version || '0.0.7')}`);
  appState.commands = data.commands;
  renderApp();
}

async function loadHistory() {
  const data = invoke ? await invoke('tauri_history') : await fetchJson('/api/history');
  appState.history = data.runs || [];
  renderApp();
}

async function loadLatestReleaseDir() {
  if (!invoke) return;
  const data = await invoke('tauri_latest_release_dir');
  if (data.releaseDir) {
    elements.releaseDirInput.value = data.releaseDir;
    await loadSummary(data.releaseDir);
  }
}

async function openReleaseDir() {
  if (!invoke) return;
  const releaseDir = elements.releaseDirInput.value.trim() || appState.summary?.releaseDir || '';
  const data = await invoke('tauri_open_release_dir', { releaseDir });
  appState.lastAction = data;
  renderApp();
}

function buildReleaseViewModel(summary, config, lastAction) {
  const state = summary?.state || {};
  const currentStep = resolveCurrentStep(state, summary);
  const progressItems = mapProgressItems(state.steps || {}, currentStep.key);
  const primarySuggestion = primarySuggestionFor(currentStep, state, lastAction);
  return {
    summary,
    config,
    lastAction,
    currentStep,
    primaryAction: resolvePrimaryAction(currentStep),
    primarySuggestion,
    progressItems,
    overall: resolveOverallStatus(currentStep, state),
  };
}

function resolveCurrentStep(state, summary) {
  const steps = state?.steps || {};
  const hasContext = Boolean(elements.versionInput.value.trim() && (elements.releaseDirInput.value.trim() || summary?.releaseDir));
  if (!hasContext) {
    return { key: 'context', label: '准备发布信息', status: 'not_started', description: '确认目标版本和 release 目录后开始发布准备。', action: '' };
  }
  if (steps.preflight?.status !== 'success') {
    return {
      key: 'preflight',
      label: '先确认发布环境',
      status: steps.preflight?.status || 'not_started',
      description: '检查源码版本、凭据、4090 连接和 updater 端点。',
      action: 'preflight',
    };
  }
  if (steps.artifactCheck?.status !== 'success') {
    return {
      key: 'artifactCheck',
      label: '确认发布产物',
      action: 'check-artifacts',
      status: steps.artifactCheck?.status || 'not_started',
      description: '检查 Windows 与 Mac 产物及签名。',
    };
  }
  if (steps.manifestGithub?.status !== 'success' || steps.manifestGitee?.status !== 'success') {
    return {
      key: 'manifest',
      label: '生成 Manifest',
      action: 'manifest',
      status: firstBlockingStatus([steps.manifestGithub, steps.manifestGitee]),
      description: '生成 checksums 和 GitHub/Gitee manifest。',
    };
  }
  if (steps.verify?.status !== 'success') {
    return {
      key: 'verify',
      label: '验证线上端点',
      status: steps.verify?.status || 'not_started',
      description: '确认 updater manifest 与下载 URL 可访问且版本匹配。',
      action: 'verify',
    };
  }
  if (steps.dryRunRelease?.status !== 'success') {
    return {
      key: 'dryRunRelease',
      label: '生成发布命令',
      status: steps.dryRunRelease?.status || 'not_started',
      description: '生成真实发布前的 dry-run 命令，不执行上传。',
      action: 'dry-run-release',
    };
  }
  if (steps.report?.status !== 'success') {
    return {
      key: 'report',
      label: '生成发布报告',
      status: steps.report?.status || 'not_started',
      description: '沉淀本次发布的步骤、状态、产物和 manifest 摘要。',
      action: 'report',
    };
  }
  return {
    key: 'complete',
    label: '发布准备完成',
    status: 'success',
    action: 'copyCommands',
    description: '预检、产物、manifest、验证和 dry-run 已完成。',
  };
}

function resolvePrimaryAction(currentStep) {
  if (currentStep.key === 'context') return { label: '确认发布信息', action: 'refresh', disabled: false };
  if (currentStep.key === 'complete') return { label: '复制发布命令', action: 'copyCommands', disabled: false };
  if (!currentStep.action) return { label: 'Phase 2 启用', action: '', disabled: true };
  const retry = currentStep.status === 'failed' ? '重新' : '';
  const labels = {
    preflight: `${retry}运行预检`,
    artifactCheck: `${retry}检查产物`,
    manifest: `${retry}生成 Manifest`,
    verify: `${retry}验证端点`,
    dryRunRelease: `${retry}生成 dry-run`,
    report: `${retry}生成报告`,
  };
  return { label: labels[currentStep.key] || currentStep.label, action: currentStep.action, disabled: false };
}

function mapProgressItems(steps, currentKey) {
  return progressSteps.map((step) => {
    if (step.key === 'manifest') {
      const manifestDone = steps.manifestGithub?.status === 'success' && steps.manifestGitee?.status === 'success';
      const manifestFailed = steps.manifestGithub?.status === 'failed' || steps.manifestGitee?.status === 'failed';
      return { ...step, status: manifestDone ? 'success' : manifestFailed ? 'failed' : 'not_started', active: currentKey === step.key || currentKey === 'artifactCheck' };
    }
    return { ...step, status: steps[step.key]?.status || 'not_started', active: currentKey === step.key };
  });
}

function primarySuggestionFor(currentStep, state, lastAction) {
  if (currentStep.key === 'context') {
    if (!elements.versionInput.value.trim()) return { severity: 'warning', message: '先填写目标版本。' };
    if (!elements.releaseDirInput.value.trim() && !appState.summary?.releaseDir) return { severity: 'warning', message: '先选择或读取 release 目录。' };
  }

  if (currentStep.key === 'complete') {
    return { severity: 'info', message: '发布准备、dry-run 和报告已完成；真实发布仍需版本确认。' };
  }

  if (Object.values(appState.summary?.manifests || {}).some((manifest) => manifest.versionStatus === 'same_version')) {
    return { severity: 'warning', message: '线上 manifest 已是目标版本，重复发布前请确认。' };
  }

  if (currentStep.key === 'dryRunRelease') {
    return { severity: 'info', message: 'dry-run 只生成发布命令，不会执行上传。' };
  }

  const suggestions = [...(lastAction?.suggestions || []), ...(state.suggestions || [])];
  return suggestions.find((item) => item.severity === 'error')
    || suggestions[0]
    || { severity: 'info', message: '当前没有阻塞。按主按钮继续。' };
}

function resolveOverallStatus(currentStep, state) {
  if (currentStep.status === 'failed' || state?.overallStatus === 'failed') return { label: '有阻塞', tone: 'danger' };
  if (currentStep.status === 'manual_required' || state?.overallStatus === 'manual_required') return { label: '需人工处理', tone: 'warning' };
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

function renderApp() {
  if (!elements.overallStatusText) return;
  const viewModel = buildReleaseViewModel(appState.summary, appState.config, appState.lastAction);
  renderHeader(viewModel);
  renderProgressRail(viewModel.progressItems);
  renderCurrentStep(viewModel);
  renderStepDetails(viewModel);
  renderSecondaryDetails(viewModel);
  renderHistory();
}

function renderHeader(viewModel) {
  elements.releaseDirText.textContent = viewModel.summary?.releaseDir || viewModel.summary?.message || '没有发布目录';
  elements.overallStatusText.textContent = viewModel.overall.label;
  elements.overallStatusBadge.textContent = viewModel.overall.label;
  elements.overallStatusBadge.className = `status-pill ${viewModel.overall.tone}`;
}

function renderProgressRail(items) {
  elements.progressRail.innerHTML = items.map((item) => `
    <div class="progress-item ${item.active ? 'active' : ''}">
      <span class="progress-segment ${toneForStatus(item.status)}"></span>
      <span>${item.label}</span>
    </div>
  `).join('');
}

function renderCurrentStep(viewModel) {
  const { currentStep, primaryAction, primarySuggestion, progressItems } = viewModel;
  const activeIndex = Math.max(0, progressItems.findIndex((item) => item.active));
  elements.currentStepNumber.textContent = currentStep.key === 'complete' ? '✓' : String(activeIndex + 1);
  elements.currentStepEyebrow.textContent = currentStep.key === 'complete' ? '准备完成' : '当前步骤';
  elements.currentStepTitle.textContent = currentStep.label;
  elements.currentStepDescription.textContent = currentStep.description;
  elements.currentStepStatus.textContent = statusLabel(currentStep.status);
  elements.currentStepStatus.className = `status-pill ${toneForStatus(currentStep.status)}`;
  elements.primaryActionButton.textContent = appState.loadingAction ? '执行中...' : primaryAction.label;
  elements.primaryActionButton.disabled = primaryAction.disabled || Boolean(appState.loadingAction);
  elements.primaryActionButton.dataset.action = primaryAction.action;
  elements.focusSuggestions.innerHTML = `
    <article class="focus-suggestion ${toneForSuggestion(primarySuggestion.severity)}">
      <strong>${suggestionLabel(primarySuggestion.severity)}</strong>
      <span>${primarySuggestion.message}</span>
    </article>
  `;
  elements.detailsSummaryText.textContent = detailsSummaryFor(currentStep);
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

function detailsSummaryFor(currentStep) {
  return {
    context: '发布信息与配置',
    preflight: '预检结果与建议',
    artifactCheck: '产物与 Manifest 详情',
    verify: '端点验证结果',
    dryRunRelease: '发布命令',
    complete: '准备完成详情',
  }[currentStep.key] || '当前步骤详情';
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
  rows.sort((a, b) => Number(b.exists === false) - Number(a.exists === false));
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
  const baseRows = ['github', 'gitee'].map((host) => {
    const manifest = manifests[host] || {};
    const status = manifest.versionStatus === 'same_version' ? ' · 已是目标版本' : '';
    return `<article class="compact-row"><span>${hostLabel(host)}</span><strong>${manifest.exists ? `${manifest.version}${status}` : '缺失'}</strong></article>`;
  });
  const diff = appState.summary?.manifestDiff || {};
  const warnings = [
    ...(diff.missingOnGithub || []).map((item) => `GitHub 缺 ${item}`),
    ...(diff.missingOnGitee || []).map((item) => `Gitee 缺 ${item}`),
  ];
  elements.manifestList.innerHTML = [
    ...baseRows,
    ...warnings.map((warning) => `<article class="compact-row"><span>差异</span><strong>${escapeHtml(warning)}</strong></article>`),
  ].join('');
}

function renderHistory() {
  if (!elements.historyList) return;
  elements.historyList.innerHTML = appState.history.length
    ? appState.history.map((run) => `<article class="compact-row"><span>${escapeHtml(run.version || '-')}</span><strong>${escapeHtml(run.overallStatus || '-')}</strong></article>`).join('')
    : `<p class="empty-state">暂无发布历史</p>`;
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

    const confirmVersion = elements.confirmVersionInput?.value.trim() || '';
    const data = invoke
      ? await invoke('tauri_action', { action, version, releaseDir, confirmVersion })
      : await fetchAction(`/api/actions/${action}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ version, releaseDir, confirmVersion }),
      });

    appState.lastAction = data;
    elements.actionLogOutput.textContent = [data.logs?.stdout, data.logs?.stderr].filter(Boolean).join('\n') || data.message;
    await loadSummary(releaseDir);
    await loadHistory();
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
    manual_required: 'warning',
    failed: 'danger',
    disabled: 'neutral',
    not_started: 'neutral',
  }[status] || 'neutral';
}

function toneForSuggestion(severity) {
  return {
    error: 'danger',
    warning: 'warning',
    info: 'info',
  }[severity] || 'info';
}

function suggestionLabel(severity) {
  return {
    error: '阻塞',
    warning: '建议',
    info: '提示',
  }[severity] || '提示';
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
