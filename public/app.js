const invoke = window.__TAURI_INTERNALS__?.invoke;

const journeyStages = [
  { key: 'prepare', label: '准备发版', steps: ['preflight'] },
  { key: 'build', label: '打包双端', steps: ['buildWindows', 'buildMac'] },
  { key: 'collect', label: '收集检查', steps: ['collectWindowsArtifacts', 'artifactCheck', 'manifest'] },
  { key: 'publish', label: '发布分发', steps: ['publishGithub', 'publishGitee'] },
  { key: 'verify', label: '验证报告', steps: ['verify', 'report'] },
];

const elements = {
  releaseDirInput: document.querySelector('#releaseDirInput'),
  releaseDirText: document.querySelector('#releaseDirText'),
  versionInput: document.querySelector('#versionInput'),
  refreshButton: document.querySelector('#refreshButton'),
  latestDirButton: document.querySelector('#latestDirButton'),
  openDirButton: document.querySelector('#openDirButton'),
  loadButton: document.querySelector('#loadButton'),
  historyButton: document.querySelector('#historyButton'),
  diagnosticsButton: document.querySelector('#diagnosticsButton'),
  overallStatusText: document.querySelector('#overallStatusText'),
  overallStatusBadge: document.querySelector('#overallStatusBadge'),
  progressRail: document.querySelector('#progressRail'),
  currentStepNumber: document.querySelector('#currentStepNumber'),
  currentStepEyebrow: document.querySelector('#currentStepEyebrow'),
  currentStepTitle: document.querySelector('#currentStepTitle'),
  currentStepDescription: document.querySelector('#currentStepDescription'),
  currentStepStatus: document.querySelector('#currentStepStatus'),
  primaryActionButton: document.querySelector('#primaryActionButton'),
  detailButton: document.querySelector('#detailButton'),
  focusSuggestions: document.querySelector('#focusSuggestions'),
  confirmSlot: document.querySelector('#confirmSlot'),
  logTitle: document.querySelector('#logTitle'),
  toggleLogsButton: document.querySelector('#toggleLogsButton'),
  actionLogOutput: document.querySelector('#actionLogOutput'),
  detailPanel: document.querySelector('#detailPanel'),
  closeDetailButton: document.querySelector('#closeDetailButton'),
  detailsSummaryText: document.querySelector('#detailsSummaryText'),
  stepDetailContent: document.querySelector('#stepDetailContent'),
  diagnosticsPanel: document.querySelector('#diagnosticsPanel'),
  closeDiagnosticsButton: document.querySelector('#closeDiagnosticsButton'),
  diagnosticsContent: document.querySelector('#diagnosticsContent'),
};

const appState = {
  summary: null,
  config: null,
  commands: [],
  history: [],
  lastAction: null,
  loadingAction: '',
  logPoller: null,
  detailOpen: false,
  diagnosticsOpen: false,
  diagnosticsTab: 'config',
};

await loadConfig();
await loadSummary();
await loadCommands();
await loadHistory();

if (elements.refreshButton) elements.refreshButton.addEventListener('click', () => loadSummary(elements.releaseDirInput.value.trim()));
if (elements.loadButton) elements.loadButton.addEventListener('click', () => loadSummary(elements.releaseDirInput.value.trim()));
if (elements.latestDirButton) elements.latestDirButton.addEventListener('click', () => loadLatestReleaseDir());
if (elements.openDirButton) elements.openDirButton.addEventListener('click', () => openReleaseDir());
if (!invoke) document.querySelectorAll('.desktop-only').forEach((element) => { element.hidden = true; });
if (elements.primaryActionButton) elements.primaryActionButton.addEventListener('click', () => runPrimaryAction());
if (elements.detailButton) elements.detailButton.addEventListener('click', () => toggleDetailPanel());
if (elements.historyButton) elements.historyButton.addEventListener('click', () => openDiagnostics('history'));
if (elements.diagnosticsButton) elements.diagnosticsButton.addEventListener('click', () => openDiagnostics('config'));
if (elements.closeDetailButton) elements.closeDetailButton.addEventListener('click', () => closeDetailPanel());
if (elements.closeDiagnosticsButton) elements.closeDiagnosticsButton.addEventListener('click', () => closeDiagnostics());
if (elements.versionInput) elements.versionInput.addEventListener('change', () => loadCommands(elements.versionInput.value.trim()));
if (elements.toggleLogsButton) {
  elements.toggleLogsButton.addEventListener('click', () => {
    elements.actionLogOutput.classList.toggle('hidden');
    elements.toggleLogsButton.textContent = elements.actionLogOutput.classList.contains('hidden') ? '展开' : '折叠';
  });
}
if (elements.diagnosticsPanel) {
  elements.diagnosticsPanel.addEventListener('click', async (event) => {
    const tab = event.target.closest('[data-diagnostics-tab]');
    if (tab) {
      appState.diagnosticsTab = tab.dataset.diagnosticsTab;
      renderApp();
      return;
    }
    const copyButton = event.target.closest('button[data-copy-raw]');
    if (!copyButton) return;
    await navigator.clipboard.writeText(decodeURIComponent(copyButton.dataset.copyRaw));
    copyButton.textContent = '已复制';
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
    return { key: 'context', label: '准备发布信息', status: 'not_started', description: '确认目标版本和 release 目录后开始。', action: '' };
  }
  if (steps.preflight?.status !== 'success') {
    return { key: 'preflight', label: '准备发版', status: steps.preflight?.status || 'not_started', description: '检查版本、凭据、4090 连接和 updater 端点。', action: 'preflight' };
  }
  if (steps.buildWindows?.status !== 'success') {
    return { key: 'buildWindows', label: '打包 Windows', status: steps.buildWindows?.status || 'not_started', description: '在 4090 上创建干净 worktree 并打包 Windows 应用。', action: 'build-windows' };
  }
  if (steps.buildMac?.status !== 'success') {
    return { key: 'buildMac', label: '打包 Mac', status: steps.buildMac?.status || 'not_started', description: '在本机从源码仓库打包 Mac 应用。', action: 'build-mac' };
  }
  if (steps.collectWindowsArtifacts?.status !== 'success') {
    return { key: 'collectWindowsArtifacts', label: '收集 Windows 产物', status: steps.collectWindowsArtifacts?.status || 'not_started', description: '从 4090 拉回 Windows 产物和签名。', action: 'collect-windows-artifacts' };
  }
  if (steps.artifactCheck?.status !== 'success') {
    return { key: 'artifactCheck', label: '检查产物', action: 'check-artifacts', status: steps.artifactCheck?.status || 'not_started', description: '确认 Windows 与 Mac 产物、签名齐全。' };
  }
  if (steps.manifestGithub?.status !== 'success' || steps.manifestGitee?.status !== 'success') {
    return { key: 'manifest', label: '生成 Manifest', action: 'manifest', status: firstBlockingStatus([steps.manifestGithub, steps.manifestGitee]), description: '生成 checksums 和 GitHub/Gitee manifest。' };
  }
  if (steps.publishGithub?.status !== 'success') {
    return { key: 'publishGithub', label: '发布 GitHub', status: steps.publishGithub?.status || 'not_started', description: '确认版本后创建 GitHub Release 并上传产物。', action: 'publish-github' };
  }
  if (steps.publishGitee?.status !== 'success') {
    return { key: 'publishGitee', label: '发布 Gitee', status: steps.publishGitee?.status || 'not_started', description: '确认版本后创建 Gitee Release 并更新 latest.json。', action: 'publish-gitee' };
  }
  if (steps.verify?.status !== 'success') {
    return { key: 'verify', label: '验证线上更新', status: steps.verify?.status || 'not_started', description: '确认 updater manifest 与下载 URL 可访问。', action: 'verify' };
  }
  if (steps.report?.status !== 'success') {
    return { key: 'report', label: '生成发布报告', status: steps.report?.status || 'not_started', description: '生成本次发布的报告摘要。', action: 'report' };
  }
  return { key: 'complete', label: '发布完成', status: 'success', action: 'copyCommands', description: '打包、发布、验证和报告已完成。' };
}

function resolvePrimaryAction(currentStep) {
  if (currentStep.key === 'context') return { label: '确认发布信息', action: 'refresh', disabled: false };
  if (currentStep.key === 'complete') return { label: '复制发布命令', action: 'copyCommands', disabled: false };
  if (!currentStep.action) return { label: '等待上下文', action: '', disabled: true };
  const retry = currentStep.status === 'failed' ? '重新' : '';
  const labels = {
    preflight: `${retry}开始预检`,
    buildWindows: `${retry}打包 Windows`,
    buildMac: `${retry}打包 Mac`,
    collectWindowsArtifacts: `${retry}收集 Windows 产物`,
    artifactCheck: `${retry}检查产物`,
    manifest: `${retry}生成 Manifest`,
    publishGithub: `${retry}发布 GitHub`,
    publishGitee: `${retry}发布 Gitee`,
    verify: `${retry}验证线上更新`,
    report: `${retry}生成报告`,
  };
  return { label: labels[currentStep.key] || currentStep.label, action: currentStep.action, disabled: false };
}

function mapProgressItems(steps, currentKey) {
  const activeStage = stageForStep(currentKey);
  return journeyStages.map((stage, index) => {
    const statuses = stage.steps.map((step) => statusForStep(steps, step));
    const done = statuses.filter((status) => status === 'success').length;
    return {
      ...stage,
      index: index + 1,
      status: stageStatus(statuses),
      active: stage.key === activeStage,
      done,
      total: stage.steps.length,
    };
  });
}

function statusForStep(steps, key) {
  if (key === 'manifest') {
    const statuses = [steps.manifestGithub?.status, steps.manifestGitee?.status].filter(Boolean);
    if (statuses.includes('failed')) return 'failed';
    if (statuses.includes('running')) return 'running';
    if (statuses.length === 2 && statuses.every((status) => status === 'success')) return 'success';
    if (statuses.some((status) => status === 'success')) return 'running';
    return 'not_started';
  }
  return steps[key]?.status || 'not_started';
}

function stageStatus(statuses) {
  if (statuses.includes('failed')) return 'failed';
  if (statuses.includes('running')) return 'running';
  if (statuses.every((status) => status === 'success')) return 'success';
  if (statuses.some((status) => status === 'success')) return 'running';
  return 'not_started';
}

function stageForStep(key) {
  if (key === 'context') return 'prepare';
  if (key === 'complete') return 'verify';
  return journeyStages.find((stage) => stage.steps.includes(key))?.key || 'prepare';
}

function primarySuggestionFor(currentStep, state, lastAction) {
  if (currentStep.key === 'context') {
    if (!elements.versionInput.value.trim()) return { severity: 'warning', message: '先填写目标版本。' };
    if (!elements.releaseDirInput.value.trim() && !appState.summary?.releaseDir) return { severity: 'warning', message: '先选择或读取 release 目录。' };
  }
  if (currentStep.key === 'complete') return { severity: 'info', message: '发版流程已走完。' };
  if (currentStep.key === 'publishGithub' || currentStep.key === 'publishGitee') {
    return { severity: 'warning', message: '发布前必须输入与目标一致的版本号确认。' };
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
  renderConfirmSlot(viewModel.currentStep);
  renderDetailPanel(viewModel);
  renderDiagnosticsPanel(viewModel);
}

function renderHeader(viewModel) {
  elements.releaseDirText.textContent = viewModel.summary?.releaseDir || viewModel.summary?.message || '没有发布目录';
  elements.overallStatusText.textContent = viewModel.overall.label;
  elements.overallStatusBadge.textContent = viewModel.overall.label;
  elements.overallStatusBadge.className = `status-pill ${viewModel.overall.tone}`;
}

function renderProgressRail(items) {
  elements.progressRail.innerHTML = items.map((item) => `
    <div class="journey-item ${item.active ? 'active' : ''} ${item.status}">
      <div class="journey-index">${item.status === 'success' ? '✓' : item.index}</div>
      <div class="journey-copy">
        <strong>${escapeHtml(String(item.label))}</strong>
        <span>${statusLabel(item.status)} · ${item.done}/${item.total}</span>
      </div>
    </div>
  `).join('');
}

function renderCurrentStep(viewModel) {
  const { currentStep, primaryAction, primarySuggestion, progressItems } = viewModel;
  const activeIndex = Math.max(0, progressItems.findIndex((item) => item.active));
  elements.currentStepNumber.textContent = currentStep.key === 'complete' ? '✓' : String(activeIndex + 1);
  elements.currentStepEyebrow.textContent = currentStep.key === 'complete' ? '发布完成' : progressItems[activeIndex]?.label || '当前步骤';
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
      <span>${escapeHtml(String(primarySuggestion.message))}</span>
    </article>
  `;
  elements.logTitle.textContent = appState.loadingAction ? `正在执行 ${appState.loadingAction}` : '等待执行';
}

function renderConfirmSlot(currentStep) {
  const needsConfirm = currentStep.key === 'publishGithub' || currentStep.key === 'publishGitee';
  elements.confirmSlot.hidden = !needsConfirm;
  elements.confirmSlot.innerHTML = needsConfirm
    ? `
      <label for="confirmVersionInput">
        <span>版本确认</span>
        <input id="confirmVersionInput" placeholder="输入 ${escapeHtml(elements.versionInput.value.trim() || '目标版本')} 后发布" />
      </label>
    `
    : '';
}

function renderDetailPanel(viewModel) {
  elements.detailPanel.hidden = !appState.detailOpen;
  elements.detailsSummaryText.textContent = detailsSummaryFor(viewModel.currentStep);
  if (!appState.detailOpen) return;
  renderStepDetails(viewModel);
}

function renderDiagnosticsPanel(viewModel) {
  elements.diagnosticsPanel.hidden = !appState.diagnosticsOpen;
  if (!appState.diagnosticsOpen) return;
  document.querySelectorAll('.diagnostics-tab').forEach((tab) => {
    tab.classList.toggle('active', tab.dataset.diagnosticsTab === appState.diagnosticsTab);
  });
  const renderers = {
    config: () => renderConfig(viewModel.config || {}),
    manifest: () => renderManifestSummary(viewModel.summary?.manifests || {}),
    artifacts: () => renderFullArtifacts(viewModel.summary?.requiredArtifacts || [], viewModel.summary?.artifacts || []),
    history: () => renderHistory(),
    commands: () => renderCommands(appState.commands || []),
  };
  elements.diagnosticsContent.innerHTML = renderers[appState.diagnosticsTab]?.() || '<p class="empty-state">暂无信息</p>';
}

function renderStepDetails(viewModel) {
  const state = viewModel.summary?.state || {};
  const currentKey = viewModel.currentStep.key;
  if (currentKey === 'preflight') {
    elements.stepDetailContent.innerHTML = checksHtml(state.checks || []);
    return;
  }
  if (currentKey === 'artifactCheck' || currentKey === 'manifest') {
    elements.stepDetailContent.innerHTML = artifactsHtml(viewModel.summary?.requiredArtifacts || [], viewModel.summary?.artifacts || []);
    return;
  }
  if (currentKey === 'buildWindows' || currentKey === 'collectWindowsArtifacts' || currentKey === 'buildMac') {
    elements.stepDetailContent.innerHTML = buildSummaryHtml(state.steps?.[currentKey]);
    return;
  }
  if (currentKey === 'publishGithub' || currentKey === 'publishGitee') {
    elements.stepDetailContent.innerHTML = publishSummaryHtml(state.steps?.[currentKey]);
    return;
  }
  if (currentKey === 'verify') {
    elements.stepDetailContent.innerHTML = verifyResultsHtml(state.manifests?.onlineVerify?.results || []);
    return;
  }
  if (currentKey === 'report') {
    elements.stepDetailContent.innerHTML = reportHtml(state.steps?.report);
    return;
  }
  elements.stepDetailContent.innerHTML = '<p class="empty-state">当前步骤暂无详情。</p>';
}

function detailsSummaryFor(currentStep) {
  return {
    context: '发布信息',
    preflight: '预检结果',
    artifactCheck: '产物检查',
    manifest: 'Manifest 详情',
    buildWindows: 'Windows 打包',
    collectWindowsArtifacts: '产物收集',
    buildMac: 'Mac 打包',
    publishGithub: 'GitHub 发布',
    publishGitee: 'Gitee 发布',
    verify: '验证结果',
    report: '发布报告',
    complete: '完成摘要',
  }[currentStep.key] || '当前步骤详情';
}

function checksHtml(checks) {
  return checks.length ? checks.map((check) => `
    <article class="detail-row">
      <span class="status-dot ${toneForStatus(check.status)}"></span>
      <div>
        <strong>${escapeHtml(String(check.label || '-'))}</strong>
        <p>${escapeHtml(String(check.message || '-'))}</p>
      </div>
    </article>
  `).join('') : '<p class="empty-state">还没有检查结果。</p>';
}

function publishSummaryHtml(step) {
  const summary = step?.summary || {};
  const assets = summary.assets || [];
  return assets.length ? assets.map((asset) => {
    const label = typeof asset === 'string' ? asset : asset.name || asset.id || '-';
    const detail = typeof asset === 'string' ? summary.repo || '' : `id: ${asset.id || '-'} size: ${asset.size || '-'}`;
    return `
      <article class="detail-row">
        <span class="status-dot success"></span>
        <div>
          <strong>${escapeHtml(String(label))}</strong>
          <p>${escapeHtml(String(detail))}</p>
        </div>
      </article>
    `;
  }).join('') : '<p class="empty-state">输入确认版本后执行发布。</p>';
}

function buildSummaryHtml(step) {
  const summary = step?.summary || {};
  const files = summary.files || [];
  if (files.length) {
    return files.map((file) => `
      <article class="detail-row">
        <span class="status-dot success"></span>
        <div>
          <strong>${escapeHtml(file.target || '-')}</strong>
          <p>${escapeHtml(file.source || '-')}</p>
        </div>
      </article>
    `).join('');
  }
  return '<p class="empty-state">还没有打包或收集结果。</p>';
}

function artifactsHtml(requiredArtifacts, actualArtifacts) {
  const requiredKeys = new Set(requiredArtifacts.map((artifact) => `${artifact.scope}/${artifact.name}`));
  const rows = requiredArtifacts.length
    ? [...requiredArtifacts, ...actualArtifacts.filter((artifact) => !requiredKeys.has(`${artifact.scope}/${artifact.name}`))]
    : actualArtifacts;
  rows.sort((a, b) => Number(b.exists === false) - Number(a.exists === false));
  return rows.length ? rows.map((artifact) => `
    <article class="detail-row">
      <span class="status-dot ${artifact.exists === false ? 'danger' : 'success'}"></span>
      <div>
        <strong>${escapeHtml(String(artifact.name || '-'))}</strong>
        <p>${escapeHtml(`${artifact.scope || '-'}${artifact.signatureFor ? ` · 签名：${artifact.signatureFor}` : ''} · ${artifact.exists === false ? '缺失' : formatSize(artifact.size)}`)}</p>
      </div>
    </article>
  `).join('') : '<p class="empty-state">还没有产物信息。</p>';
}

function verifyResultsHtml(results) {
  return results.length ? results.map((result) => `
    <article class="detail-row">
      <span class="status-dot ${toneForStatus(result.status)}"></span>
      <div>
        <strong>${escapeHtml(String(result.endpoint || '-'))}</strong>
        <p>${escapeHtml(result.status === 'success' ? `版本 ${result.version} · ${result.platforms.length} 个平台` : String(result.error || '-'))}</p>
      </div>
    </article>
  `).join('') : '<p class="empty-state">还没有线上验证结果。</p>';
}

function reportHtml(step) {
  const file = step?.summary?.file;
  return file
    ? `<article class="detail-row"><span class="status-dot success"></span><div><strong>发布报告</strong><p>${escapeHtml(file)}</p></div></article>`
    : '<p class="empty-state">还没有发布报告。</p>';
}

function renderConfig(config) {
  const rows = [
    ['Source Repo', config.sourceRepo],
    ['GitHub Repo', config.githubRepo],
    ['Gitee Repo', config.giteeOwner && config.giteeRepo ? `${config.giteeOwner}/${config.giteeRepo}` : ''],
    ['Windows SSH', config.windowsSsh],
    ['Proxy', config.githubProxy],
  ];
  return rows.map(([label, value]) => `<article class="compact-row"><span>${escapeHtml(label)}</span><strong>${escapeHtml(String(value || '-'))}</strong></article>`).join('');
}

function renderManifestSummary(manifests) {
  const baseRows = ['github', 'gitee'].map((host) => {
    const manifest = manifests[host] || {};
    const status = manifest.versionStatus === 'same_version' ? ' · 已是目标版本' : '';
    return `<article class="compact-row"><span>${hostLabel(host)}</span><strong>${escapeHtml(manifest.exists ? `${manifest.version}${status}` : '缺失')}</strong></article>`;
  });
  const diff = appState.summary?.manifestDiff || {};
  const warnings = [
    ...(diff.missingOnGithub || []).map((item) => `GitHub 缺 ${item}`),
    ...(diff.missingOnGitee || []).map((item) => `Gitee 缺 ${item}`),
  ];
  return [...baseRows, ...warnings.map((warning) => `<article class="compact-row"><span>差异</span><strong>${escapeHtml(warning)}</strong></article>`)].join('');
}

function renderHistory() {
  return appState.history.length
    ? appState.history.map((run) => `<article class="compact-row"><span>${escapeHtml(run.version || '-')}</span><strong>${escapeHtml(run.overallStatus || '-')}</strong></article>`).join('')
    : '<p class="empty-state">暂无发布历史</p>';
}

function renderFullArtifacts(requiredArtifacts, actualArtifacts) {
  const rows = requiredArtifacts.length ? requiredArtifacts : actualArtifacts;
  return rows.map((artifact) => `<article class="compact-row"><span>${escapeHtml(String(artifact.scope || '-'))}</span><strong>${escapeHtml(String(artifact.name || '-'))}</strong></article>`).join('') || '<p class="empty-state">暂无产物</p>';
}

function renderCommands(commands) {
  return commands.length ? commands.map((command) => commandCard(command)).join('') : '<p class="empty-state">暂无命令</p>';
}

function toggleDetailPanel() {
  appState.detailOpen = !appState.detailOpen;
  if (appState.detailOpen) appState.diagnosticsOpen = false;
  renderApp();
}

function closeDetailPanel() {
  appState.detailOpen = false;
  renderApp();
}

function openDiagnostics(tab) {
  appState.diagnosticsOpen = true;
  appState.detailOpen = false;
  appState.diagnosticsTab = tab;
  renderApp();
}

function closeDiagnostics() {
  appState.diagnosticsOpen = false;
  renderApp();
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
  let version = '';
  let releaseDir = '';
  try {
    version = elements.versionInput.value.trim();
    releaseDir = elements.releaseDirInput.value.trim();
    const confirmVersion = document.querySelector('#confirmVersionInput')?.value.trim() || '';
    appState.loadingAction = action;
    elements.actionLogOutput.classList.remove('hidden');
    elements.toggleLogsButton.textContent = '折叠';
    elements.actionLogOutput.textContent = `开始执行 ${action}...\n`;
    renderApp();
    startActionLogPolling(action, version, releaseDir);

    const data = invoke
      ? await invoke('tauri_action', { action, version, releaseDir, confirmVersion })
      : await fetchAction(`/api/actions/${action}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ version, releaseDir, confirmVersion }),
      });

    appState.lastAction = data;
    await loadActionLog(action, version, releaseDir);
    if (!elements.actionLogOutput.textContent.trim()) {
      elements.actionLogOutput.textContent = [data.logs?.stdout, data.logs?.stderr].filter(Boolean).join('\n') || data.message;
    }
    await loadSummary(releaseDir);
    await loadHistory();
  } catch (error) {
    appState.lastAction = { ok: false, suggestions: [{ severity: 'error', message: error.message, action }] };
    elements.actionLogOutput.textContent = error.message;
    renderApp();
  } finally {
    stopActionLogPolling();
    if (action && version && releaseDir) await loadActionLog(action, version, releaseDir).catch(() => {});
    appState.loadingAction = '';
    renderApp();
  }
}

function startActionLogPolling(action, version, releaseDir) {
  stopActionLogPolling();
  appState.logPoller = window.setInterval(() => {
    loadActionLog(action, version, releaseDir).catch(() => {});
  }, 1000);
}

function stopActionLogPolling() {
  if (appState.logPoller) {
    window.clearInterval(appState.logPoller);
    appState.logPoller = null;
  }
}

async function loadActionLog(action, version, releaseDir) {
  if (!version || !releaseDir) return;
  const data = invoke
    ? await invoke('tauri_action_log', { action, version, releaseDir })
    : await fetchJson(`/api/action-log?action=${encodeURIComponent(action)}&version=${encodeURIComponent(version)}&releaseDir=${encodeURIComponent(releaseDir)}`);
  if (data.log) {
    elements.actionLogOutput.textContent = data.log;
    elements.actionLogOutput.scrollTop = elements.actionLogOutput.scrollHeight;
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
  return String(value).replace(/[&<>"]/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[char]));
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
