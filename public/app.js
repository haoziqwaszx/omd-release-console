const flowSteps = [
  'Mac 预检源码状态、凭据、updater 端点和 4090 SSH。',
  '产品源码完成版本更新、验证、提交和推送。',
  '4090 从远端 commit 创建干净 Windows worktree。',
  '4090 构建并签名 NSIS、MSI、绿色 zip。',
  'Mac 用 scp 拉回 Windows 产物和签名。',
  'Mac 本机构建 dmg、app.tar.gz 和签名。',
  'Mac 生成 checksums、GitHub latest.json、Gitee latest.json。',
  'Mac 发布 GitHub Release 并标记 latest。',
  'Mac 发布 Gitee Release 并更新根 latest.json。',
  'Mac 验证两个 updater 端点和 manifest 里的每个 URL。',
];

const releaseActions = [
  { key: 'preflight', label: '预检', description: '检查源码版本、凭据、SSH 和 updater 端点。', risk: '低风险', runnable: true },
  { key: 'manifest', label: '生成 Manifest', description: '检查产物，生成 checksums、GitHub/Gitee latest.json。', risk: '低风险', runnable: true },
  { key: 'verify', label: '验证端点', description: '验证线上 updater endpoint 和下载 URL。', risk: '低风险', runnable: true },
  { key: 'dry-run-release', stepKey: 'dryRunRelease', label: 'Dry-run 发布', description: '输出真实发布命令但不执行上传。', risk: '低风险', runnable: true },
  { key: 'publish-github', stepKey: 'publishGithub', label: '发布 GitHub', description: '真实发布在 Phase 1 禁用。', risk: '高风险', runnable: false },
  { key: 'publish-gitee', stepKey: 'publishGitee', label: '发布 Gitee', description: '真实发布在 Phase 1 禁用。', risk: '高风险', runnable: false },
];

const elements = {
  releaseDirInput: document.querySelector('#releaseDirInput'),
  releaseDirText: document.querySelector('#releaseDirText'),
  githubVersion: document.querySelector('#githubVersion'),
  giteeVersion: document.querySelector('#giteeVersion'),
  artifactCount: document.querySelector('#artifactCount'),
  checkpointCount: document.querySelector('#checkpointCount'),
  flowList: document.querySelector('#flowList'),
  manifestList: document.querySelector('#manifestList'),
  checkpointList: document.querySelector('#checkpointList'),
  artifactList: document.querySelector('#artifactList'),
  versionInput: document.querySelector('#versionInput'),
  commandOutput: document.querySelector('#commandOutput'),
  refreshButton: document.querySelector('#refreshButton'),
  loadButton: document.querySelector('#loadButton'),
  commandButton: document.querySelector('#commandButton'),
  configSummary: document.querySelector('#configSummary'),
  actionList: document.querySelector('#actionList'),
  suggestionList: document.querySelector('#suggestionList'),
  actionLogOutput: document.querySelector('#actionLogOutput'),
};

renderFlow();
loadConfig();
renderActions({});
loadSummary();
loadCommands();

elements.refreshButton.addEventListener('click', () => loadSummary(elements.releaseDirInput.value.trim()));
elements.loadButton.addEventListener('click', () => loadSummary(elements.releaseDirInput.value.trim()));
elements.commandButton.addEventListener('click', () => loadCommands(elements.versionInput.value.trim()));
elements.actionList.addEventListener('click', async (event) => {
  const button = event.target.closest('button[data-action]');
  if (!button) return;
  await runAction(button.dataset.action);
});
elements.commandOutput.addEventListener('click', async (event) => {
  const button = event.target.closest('button[data-copy-raw]');
  if (!button) return;
  await navigator.clipboard.writeText(decodeURIComponent(button.dataset.copyRaw));
  button.textContent = '已复制';
});

async function loadSummary(releaseDir = '') {
  const query = releaseDir ? `?releaseDir=${encodeURIComponent(releaseDir)}` : '';
  const data = await fetchJson(`/api/summary${query}`);
  renderSummary(data);
}

async function loadCommands(version = '0.0.7') {
  const data = await fetchJson(`/api/commands?version=${encodeURIComponent(version || '0.0.7')}`);
  elements.commandOutput.innerHTML = data.commands.map((command) => {
    const highRisk = / release \d+\.\d+\.\d+(?!.*--dry-run)/.test(command);
    return `
      <article class="command-card">
        <header>
          <strong>${commandName(command)}</strong>
          <span class="badge ${highRisk ? 'danger' : 'success'}">${highRisk ? '高风险' : '低风险'}</span>
        </header>
        <code>${escapeHtml(command)}</code>
        <button type="button" data-copy-raw="${encodeURIComponent(command)}">复制</button>
      </article>
    `;
  }).join('');
}

async function loadConfig() {
  const data = await fetchJson('/api/config');
  renderConfig(data.config);
}

async function runAction(action) {
  try {
    const version = elements.versionInput.value.trim();
    const releaseDir = elements.releaseDirInput.value.trim();
    elements.actionLogOutput.textContent = `正在执行 ${action}...`;

    const data = await fetchJson(`/api/actions/${action}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ version, releaseDir }),
    });

    elements.actionLogOutput.textContent = [data.logs?.stdout, data.logs?.stderr].filter(Boolean).join('\n') || data.message;
    if (data.state) {
      renderState(data.state);
    }
    renderSuggestions(data.suggestions || data.state?.suggestions || []);
    await loadSummary(releaseDir);
  } catch (error) {
    elements.actionLogOutput.textContent = error.message;
    renderSuggestions([{ severity: 'error', message: error.message, action }]);
  }
}

function renderSummary(data) {
  elements.releaseDirText.textContent = data.releaseDir || data.message || '没有发布目录';
  if (data.releaseDir && !elements.releaseDirInput.value) {
    elements.releaseDirInput.value = data.releaseDir;
  }

  elements.githubVersion.textContent = data.manifests.github.exists ? data.manifests.github.version : '缺失';
  elements.giteeVersion.textContent = data.manifests.gitee.exists ? data.manifests.gitee.version : '缺失';
  elements.artifactCount.textContent = String((data.requiredArtifacts || data.artifacts).length);
  elements.checkpointCount.textContent = String(Object.values(data.state?.steps || {}).filter((step) => step.startedAt || step.endedAt).length);

  renderManifests(data.manifests);
  renderState(data.state || {});
  renderRequiredArtifacts(data.requiredArtifacts || [], data.artifacts || []);
  renderSuggestions(data.state?.suggestions || []);
}

function renderFlow() {
  elements.flowList.innerHTML = flowSteps.map((step) => `<li>${step}</li>`).join('');
}

function renderManifests(manifests) {
  const items = ['github', 'gitee'].flatMap((host) => {
    const manifest = manifests[host];
    if (!manifest.exists) {
      return `<article class="target-item"><header><strong>${hostLabel(host)}</strong><span class="badge danger">缺失</span></header></article>`;
    }

    return manifest.platforms.map((target) => `
      <article class="target-item">
        <header>
          <strong>${hostLabel(host)} · ${target.key}</strong>
          <span class="badge ${target.hasSignature ? 'success' : 'warning'}">${target.hasSignature ? '已签名' : '缺签名'}</span>
        </header>
        <p class="target-url">${target.url}</p>
      </article>
    `);
  });

  elements.manifestList.innerHTML = items.join('') || emptyText('暂无 manifest');
}

function renderConfig(config) {
  const rows = [
    ['Source Repo', config.sourceRepo],
    ['GitHub Repo', config.githubRepo],
    ['Gitee Repo', `${config.giteeOwner}/${config.giteeRepo}`],
    ['GitHub Latest', config.githubLatest],
    ['Gitee Latest', config.giteeLatest],
    ['Windows SSH', config.windowsSsh],
    ['Release Dir', config.releaseDirPattern],
    ['Proxy', config.githubProxy],
  ];
  elements.configSummary.innerHTML = rows.map(([label, value]) => `
    <article class="config-item">
      <span>${label}</span>
      <strong>${value || '-'}</strong>
    </article>
  `).join('');
}

function renderState(state) {
  renderActions(state.steps || {});
  renderCheckpointsFromSteps(state.steps || {});
}

function renderActions(steps) {
  elements.actionList.innerHTML = releaseActions.map((action) => {
    const step = steps[action.stepKey || action.key] || {};
    const status = step.status || (action.runnable ? 'not_started' : 'disabled');
    const disabled = !action.runnable;
    return `
      <article class="action-card">
        <header>
          <div>
            <strong>${action.label}</strong>
            <p class="muted">${action.description}</p>
          </div>
          <span class="badge ${badgeClass(status)}">${statusLabel(status)}</span>
        </header>
        <p class="muted">风险：${action.risk}${step.durationMs ? ` · 耗时 ${formatDuration(step.durationMs)}` : ''}</p>
        ${step.error ? `<p class="danger-text">${step.error}</p>` : ''}
        ${step.message ? `<p class="muted">${step.message}</p>` : ''}
        <button type="button" data-action="${action.key}" ${disabled ? 'disabled' : ''}>${disabled ? '后续阶段启用' : '运行'}</button>
      </article>
    `;
  }).join('');
}

function renderCheckpointsFromSteps(steps) {
  const recent = Object.values(steps)
    .filter((step) => step.startedAt || step.endedAt)
    .sort((left, right) => String(right.endedAt || right.startedAt).localeCompare(String(left.endedAt || left.startedAt)))
    .slice(0, 8);
  elements.checkpointList.innerHTML = recent.map((step) => `
    <article class="checkpoint-item">
      <strong>${checkpointLabel(step.key)}</strong>
      <p class="muted">${statusLabel(step.status)} · ${formatDate(step.endedAt || step.startedAt)}</p>
    </article>
  `).join('') || emptyText('暂无状态文件');
}

function renderRequiredArtifacts(requiredArtifacts, actualArtifacts) {
  const requiredKeys = new Set(requiredArtifacts.map((artifact) => `${artifact.scope}/${artifact.name}`));
  const extraRows = actualArtifacts.filter((artifact) => !requiredKeys.has(`${artifact.scope}/${artifact.name}`));
  const rows = requiredArtifacts.length ? [...requiredArtifacts, ...extraRows] : actualArtifacts;

  elements.artifactList.innerHTML = rows.map((artifact) => `
    <article class="artifact-row ${artifact.exists === false ? 'missing' : ''}">
      <div>
        <strong>${artifact.name}</strong>
        <p class="muted">${artifact.scope}${artifact.signatureFor ? ` · 签名：${artifact.signatureFor}` : ''} · ${artifact.exists === false ? '缺失' : formatDate(artifact.modifiedAt)}</p>
      </div>
      <span class="badge ${artifact.exists === false ? 'danger' : 'success'}">${artifact.exists === false ? '缺失' : formatSize(artifact.size)}</span>
    </article>
  `).join('') || emptyText('暂无产物');
}

function renderSuggestions(suggestions) {
  elements.suggestionList.innerHTML = suggestions.map((item) => `
    <article class="suggestion-item">
      <span class="badge ${item.severity === 'error' ? 'danger' : 'warning'}">${item.severity || 'info'}</span>
      <p>${item.message}</p>
    </article>
  `).join('') || emptyText('暂无建议');
}

function hostLabel(host) {
  return host === 'github' ? 'GitHub' : 'Gitee';
}

function checkpointLabel(name) {
  return {
    preflight: '预检',
    artifactCheck: '产物检查',
    checksums: 'Checksums',
    manifestGithub: 'GitHub Manifest',
    manifestGitee: 'Gitee Manifest',
    dryRunRelease: 'Dry-run 发布',
    publishGithub: '发布 GitHub',
    publishGitee: '发布 Gitee',
    verify: '线上验证',
  }[name] || name;
}

function commandName(command) {
  if (command.includes(' preflight ')) return '预检';
  if (command.includes(' --dry-run')) return 'Dry-run 发布';
  if (command.includes(' verify ')) return '验证';
  if (command.includes(' release ')) return '真实发布';
  if (command.includes(' manifest ')) return '生成 Manifest';
  return '计划';
}

function badgeClass(status) {
  return {
    success: 'success',
    running: 'warning',
    failed: 'danger',
    disabled: 'warning',
    manual_required: 'warning',
  }[status] || 'warning';
}

function statusLabel(status) {
  return {
    not_started: '未开始',
    running: '执行中',
    success: '成功',
    failed: '失败',
    skipped: '已跳过',
    manual_required: '需人工处理',
    disabled: '未启用',
  }[status] || status || '未开始';
}

function formatDate(value) {
  if (!value) return '-';
  return new Intl.DateTimeFormat('zh-CN', {
    dateStyle: 'short',
    timeStyle: 'medium',
  }).format(new Date(value));
}

function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatDuration(ms) {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function escapeHtml(value) {
  return value.replace(/[&<>"]/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[char]));
}

function emptyText(text) {
  return `<p class="muted">${text}</p>`;
}

async function fetchJson(url, options) {
  const response = await fetch(url, options);
  const data = await response.json();
  if (!response.ok || data.ok === false) {
    throw new Error(data.message || data.error || `请求失败：${response.status}`);
  }
  return data;
}
