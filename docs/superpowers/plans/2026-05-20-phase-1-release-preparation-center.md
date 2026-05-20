# Phase 1 Release Preparation Center Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Phase 1 of OMD Release Console as a low-risk release preparation center with safe action APIs, structured `state.json`, config externalization, and a frontend for running non-publishing release checks.

**Architecture:** Keep the existing Node HTTP server, static frontend, and CLI. Move release constants into `release.config.json`, make the CLI write structured state, make the server expose whitelisted action APIs that call fixed CLI commands, and make the frontend render state/action results. Real publishing endpoints are defined as disabled placeholders only.

**Tech Stack:** Node.js ES modules, built-in `node:http`, built-in `node:fs`, built-in `node:child_process`, vanilla HTML/CSS/JavaScript.

---

## File Structure

- Create `release.config.json`: non-sensitive release configuration and required artifact patterns.
- Create `scripts/release-config.mjs`: load config, expand `~`, provide default config values, validate required config shape.
- Create `scripts/release-state.mjs`: state file creation/update helpers used by CLI commands.
- Modify `scripts/omd-release.mjs`: use config module, write state for `preflight`, `manifest`, `verify`, and `release --dry-run`, and keep real release blocked.
- Modify `server.mjs`: add JSON body parsing, `GET /api/config`, action endpoints, SemVer/path validation, fixed CLI dispatch, disabled publish placeholders.
- Modify `public/index.html`: add release preparation UI sections for config summary, action cards, suggestions/logs, required artifacts, and command cards.
- Modify `public/app.js`: call new APIs, render standard state, run safe actions, render command cards and disabled publish actions.
- Modify `public/styles.css`: styles for action cards, config summary, logs, suggestions, disabled buttons, and richer artifact/manifest states.
- Modify `package.json`: keep `npm run check` and include syntax checks for any new `.mjs` files.

## Task 1: Externalize Release Configuration

**Files:**
- Create: `release.config.json`
- Create: `scripts/release-config.mjs`
- Modify: `package.json`
- Test: `npm run check`

- [ ] **Step 1: Create `release.config.json` with current non-sensitive defaults**

Create `release.config.json`:

```json
{
  "sourceRepo": "/Users/glame/Desktop/dix-extension-ui",
  "githubRepo": "haoziqwaszx/omd-distribution",
  "giteeOwner": "glame",
  "giteeRepo": "omd-distribution",
  "giteeLatest": "https://gitee.com/glame/omd-distribution/raw/master/latest.json",
  "githubLatest": "https://github.com/haoziqwaszx/omd-distribution/releases/latest/download/latest.json",
  "windowsSsh": "4090@192.168.101.9",
  "windowsRepo": "C:\\Users\\4090\\Desktop\\dix-extension-ui",
  "windowsTarget": "x86_64-pc-windows-msvc",
  "githubProxy": "http://127.0.0.1:7890",
  "releaseDirPattern": "~/Desktop/omd-*-release-*",
  "requiredArtifacts": [
    {
      "key": "windows-x86_64-nsis",
      "scope": "windows",
      "file": "OMD_{version}_x64-setup.exe",
      "signature": "OMD_{version}_x64-setup.exe.sig"
    },
    {
      "key": "windows-x86_64-msi",
      "scope": "windows",
      "file": "OMD_{version}_x64_en-US.msi",
      "signature": "OMD_{version}_x64_en-US.msi.sig"
    },
    {
      "key": "windows-x86_64-green",
      "scope": "windows",
      "file": "OMD_{version}_x64_green.zip",
      "signature": "OMD_{version}_x64_green.zip.sig"
    },
    {
      "key": "darwin-aarch64",
      "scope": "mac",
      "file": "OMD.app.tar.gz",
      "signature": "OMD.app.tar.gz.sig"
    }
  ]
}
```

- [ ] **Step 2: Create `scripts/release-config.mjs`**

Create `scripts/release-config.mjs`:

```js
import { readFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectDir = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const configFile = path.join(projectDir, 'release.config.json');

export function loadReleaseConfig() {
  const raw = JSON.parse(readFileSync(configFile, 'utf8'));
  const config = {
    ...raw,
    githubProxy: process.env.OMD_RELEASE_PROXY || raw.githubProxy || '',
  };
  validateConfig(config);
  return config;
}

export function configSummary(config) {
  return {
    sourceRepo: config.sourceRepo,
    githubRepo: config.githubRepo,
    giteeOwner: config.giteeOwner,
    giteeRepo: config.giteeRepo,
    giteeLatest: config.giteeLatest,
    githubLatest: config.githubLatest,
    windowsSsh: config.windowsSsh,
    windowsRepo: config.windowsRepo,
    windowsTarget: config.windowsTarget,
    githubProxy: config.githubProxy ? 'configured' : 'not_configured',
    releaseDirPattern: config.releaseDirPattern,
    requiredArtifacts: config.requiredArtifacts,
  };
}

export function expandHome(value) {
  return value === '~' || value.startsWith('~/')
    ? path.join(os.homedir(), value.slice(2))
    : value;
}

export function artifactName(template, version) {
  return template.replaceAll('{version}', version);
}

function validateConfig(config) {
  const requiredStrings = [
    'sourceRepo',
    'githubRepo',
    'giteeOwner',
    'giteeRepo',
    'giteeLatest',
    'githubLatest',
    'windowsSsh',
    'windowsRepo',
    'windowsTarget',
    'releaseDirPattern',
  ];

  for (const key of requiredStrings) {
    if (typeof config[key] !== 'string' || !config[key]) {
      throw new Error(`release.config.json 缺少字段：${key}`);
    }
  }

  if (!Array.isArray(config.requiredArtifacts) || config.requiredArtifacts.length === 0) {
    throw new Error('release.config.json 缺少 requiredArtifacts');
  }

  for (const artifact of config.requiredArtifacts) {
    for (const key of ['key', 'scope', 'file', 'signature']) {
      if (typeof artifact[key] !== 'string' || !artifact[key]) {
        throw new Error(`requiredArtifacts 缺少字段：${key}`);
      }
    }
  }
}
```

- [ ] **Step 3: Update `package.json` check script**

Replace the `scripts.check` value in `package.json` with:

```json
"check": "node --check server.mjs && node --check scripts/release-config.mjs && node --check scripts/release-state.mjs && node --check scripts/omd-release.mjs"
```

This will fail until Task 2 creates `scripts/release-state.mjs`; that is expected if Task 1 is run alone before Task 2.

- [ ] **Step 4: Run syntax check for the config module**

Run:

```bash
node --check scripts/release-config.mjs
```

Expected: command exits successfully with no output.

## Task 2: Add Structured Release State Helpers

**Files:**
- Create: `scripts/release-state.mjs`
- Test: `node --check scripts/release-state.mjs`

- [ ] **Step 1: Create `scripts/release-state.mjs`**

Create `scripts/release-state.mjs`:

```js
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const stepKeys = [
  'preflight',
  'artifactCheck',
  'checksums',
  'manifestGithub',
  'manifestGitee',
  'dryRunRelease',
  'verify',
  'publishGithub',
  'publishGitee',
];

export function createStateManager({ releaseDir, version, sourceRepo, commit }) {
  mkdirSync(releaseDir, { recursive: true });
  const file = path.join(releaseDir, 'state.json');

  function loadState() {
    if (!existsSync(file)) {
      const now = new Date().toISOString();
      return {
        version,
        releaseDir,
        sourceRepo,
        commit: commit || '',
        createdAt: now,
        updatedAt: now,
        currentStep: '',
        overallStatus: 'not_started',
        checks: [],
        steps: Object.fromEntries(stepKeys.map((key) => [key, emptyStep(key)])),
        artifacts: [],
        manifests: {},
        errors: [],
        suggestions: [],
      };
    }

    const state = JSON.parse(readFileSync(file, 'utf8'));
    return {
      ...state,
      version: state.version || version,
      releaseDir: state.releaseDir || releaseDir,
      sourceRepo: state.sourceRepo || sourceRepo,
      commit: state.commit || commit || '',
      checks: state.checks || [],
      steps: {
        ...Object.fromEntries(stepKeys.map((key) => [key, emptyStep(key)])),
        ...(state.steps || {}),
      },
      artifacts: state.artifacts || [],
      manifests: state.manifests || {},
      errors: state.errors || [],
      suggestions: state.suggestions || [],
    };
  }

  function saveState(state) {
    const next = {
      ...state,
      updatedAt: new Date().toISOString(),
    };
    writeFileSync(file, `${JSON.stringify(next, null, 2)}\n`, 'utf8');
    return next;
  }

  function update(updater) {
    return saveState(updater(loadState()));
  }

  function startStep(stepKey, message) {
    return update((state) => {
      const now = new Date().toISOString();
      return {
        ...state,
        currentStep: stepKey,
        overallStatus: 'running',
        steps: {
          ...state.steps,
          [stepKey]: {
            ...emptyStep(stepKey),
            ...(state.steps[stepKey] || {}),
            status: 'running',
            startedAt: now,
            endedAt: '',
            durationMs: 0,
            message: message || '',
            error: null,
          },
        },
      };
    });
  }

  function completeStep(stepKey, message, summary = {}) {
    return update((state) => {
      const now = new Date().toISOString();
      const previous = state.steps[stepKey] || emptyStep(stepKey);
      return {
        ...state,
        currentStep: stepKey,
        overallStatus: 'success',
        steps: {
          ...state.steps,
          [stepKey]: {
            ...previous,
            status: 'success',
            endedAt: now,
            durationMs: durationMs(previous.startedAt, now),
            message: message || previous.message,
            summary,
            error: null,
          },
        },
      };
    });
  }

  function failStep(stepKey, error, suggestion = '') {
    return update((state) => {
      const now = new Date().toISOString();
      const previous = state.steps[stepKey] || emptyStep(stepKey);
      const message = error instanceof Error ? error.message : String(error);
      return {
        ...state,
        currentStep: stepKey,
        overallStatus: 'failed',
        errors: [...state.errors, { step: stepKey, message, at: now }],
        suggestions: suggestion
          ? upsertSuggestion(state.suggestions, { severity: 'error', message: suggestion, action: stepKey })
          : state.suggestions,
        steps: {
          ...state.steps,
          [stepKey]: {
            ...previous,
            status: 'failed',
            endedAt: now,
            durationMs: durationMs(previous.startedAt, now),
            error: message,
            suggestions: suggestion ? [suggestion] : previous.suggestions,
          },
        },
      };
    });
  }

  function setStepDisabled(stepKey, message) {
    return update((state) => ({
      ...state,
      steps: {
        ...state.steps,
        [stepKey]: {
          ...(state.steps[stepKey] || emptyStep(stepKey)),
          status: 'disabled',
          message,
        },
      },
    }));
  }

  function recordCheck(check) {
    return update((state) => ({
      ...state,
      checks: upsertByKey(state.checks, {
        ...check,
        checkedAt: check.checkedAt || new Date().toISOString(),
      }),
    }));
  }

  function recordArtifacts(artifacts) {
    return update((state) => ({ ...state, artifacts }));
  }

  function recordManifest(host, manifest) {
    return update((state) => ({
      ...state,
      manifests: {
        ...state.manifests,
        [host]: manifest,
      },
    }));
  }

  function recordSuggestion(message, severity = 'info', action = '') {
    return update((state) => ({
      ...state,
      suggestions: upsertSuggestion(state.suggestions, { severity, message, action }),
    }));
  }

  return {
    file,
    loadState,
    saveState,
    startStep,
    completeStep,
    failStep,
    setStepDisabled,
    recordCheck,
    recordArtifacts,
    recordManifest,
    recordSuggestion,
  };
}

function emptyStep(key) {
  return {
    key,
    status: key === 'publishGithub' || key === 'publishGitee' ? 'disabled' : 'not_started',
    startedAt: '',
    endedAt: '',
    durationMs: 0,
    message: key === 'publishGithub' || key === 'publishGitee' ? '真实发布在 Phase 1 未启用' : '',
    summary: {},
    error: null,
    suggestions: [],
  };
}

function durationMs(startedAt, endedAt) {
  if (!startedAt || !endedAt) return 0;
  return Math.max(0, new Date(endedAt).getTime() - new Date(startedAt).getTime());
}

function upsertByKey(items, item) {
  return [...items.filter((existing) => existing.key !== item.key), item];
}

function upsertSuggestion(items, item) {
  if (items.some((existing) => existing.message === item.message && existing.action === item.action)) {
    return items;
  }
  return [...items, item];
}
```

- [ ] **Step 2: Run syntax check**

Run:

```bash
node --check scripts/release-state.mjs
```

Expected: command exits successfully with no output.

## Task 3: Wire CLI to Config and State

**Files:**
- Modify: `scripts/omd-release.mjs`
- Test: `node --check scripts/omd-release.mjs`

- [ ] **Step 1: Replace hardcoded config and target definitions**

At the top of `scripts/omd-release.mjs`, replace the hardcoded `config` object and `targets` array with imports and config loading:

```js
import { loadReleaseConfig, artifactName, expandHome } from './release-config.mjs';
import { createStateManager } from './release-state.mjs';

const config = loadReleaseConfig();
```

Remove the existing local `expandHome` function at the bottom of the file.

- [ ] **Step 2: Create shared state manager after `releaseDir` is computed**

After the existing `releaseDir` constant, add:

```js
const state = createStateManager({
  releaseDir,
  version: version || '',
  sourceRepo: config.sourceRepo,
  commit: command && command !== 'plan' && hasGitRepo(config.sourceRepo) ? safeExec('git', ['rev-parse', 'HEAD'], config.sourceRepo) : '',
});

state.setStepDisabled('publishGithub', '真实发布在 Phase 1 未启用');
state.setStepDisabled('publishGitee', '真实发布在 Phase 1 未启用');
```

Add helper functions near `exec`:

```js
function safeExec(command, commandArgs, cwd = process.cwd()) {
  try {
    return exec(command, commandArgs, cwd);
  } catch {
    return '';
  }
}

function hasGitRepo(dir) {
  return existsSync(path.join(dir, '.git'));
}
```

- [ ] **Step 3: Replace `targets` usage with `config.requiredArtifacts`**

In `manifestFor`, replace the target loop with:

```js
for (const target of config.requiredArtifacts) {
  const sigPath = path.join(releaseDir, target.scope, artifactName(target.signature, expectedVersion));
  assert(existsSync(sigPath), `缺少签名文件：${sigPath}`);
  platforms[target.key] = {
    signature: readFileSync(sigPath, 'utf8').replace(/\r?\n/g, ''),
    url: `${base}/${artifactName(target.file, expectedVersion)}`,
  };
}
```

In `validateManifest`, replace the target loop with:

```js
for (const target of config.requiredArtifacts) {
  assert(manifest.platforms[target.key]?.url, `${target.key} 缺少 url`);
  assert(manifest.platforms[target.key]?.signature, `${target.key} 缺少 signature`);
}
```

- [ ] **Step 4: Add artifact inspection helper**

Add this function before `manifestFor`:

```js
function inspectArtifacts(expectedVersion) {
  return config.requiredArtifacts.flatMap((target) => {
    const fileName = artifactName(target.file, expectedVersion);
    const sigName = artifactName(target.signature, expectedVersion);
    const filePath = path.join(releaseDir, target.scope, fileName);
    const sigPath = path.join(releaseDir, target.scope, sigName);
    const fileExists = existsSync(filePath);
    const signatureExists = existsSync(sigPath);
    const fileInfo = fileExists ? statSync(filePath) : null;
    const sigInfo = signatureExists ? statSync(sigPath) : null;

    return [
      {
        key: target.key,
        scope: target.scope,
        name: fileName,
        required: true,
        exists: fileExists,
        size: fileInfo?.size || 0,
        modifiedAt: fileInfo ? new Date(fileInfo.mtimeMs).toISOString() : '',
        checksum: fileExists ? sha256(filePath) : '',
        signatureFor: '',
        hasSignature: signatureExists,
      },
      {
        key: `${target.key}:signature`,
        scope: target.scope,
        name: sigName,
        required: true,
        exists: signatureExists,
        size: sigInfo?.size || 0,
        modifiedAt: sigInfo ? new Date(sigInfo.mtimeMs).toISOString() : '',
        checksum: signatureExists ? sha256(sigPath) : '',
        signatureFor: fileName,
        hasSignature: false,
      },
    ];
  });
}
```

- [ ] **Step 5: Update `preflight` to record checks and continue where safe**

Replace `preflight(expectedVersion)` with:

```js
function preflight(expectedVersion) {
  state.startStep('preflight', '开始预检');
  progress('开始预检');
  const failures = [];

  const packageJson = check('sourcePackageVersion', '源码 package.json 版本', () => {
    const packageJson = json(path.join(config.sourceRepo, 'package.json'));
    assert(packageJson.version === expectedVersion, `package.json 当前版本是 ${packageJson.version}`);
    return `package.json 版本 ${packageJson.version}`;
  }, '请先同步 package.json version');

  const tauriConfig = check('tauriVersion', 'Tauri 配置版本', () => {
    const tauriConfig = json(path.join(config.sourceRepo, 'src-tauri', 'tauri.conf.json'));
    assert(tauriConfig.version === expectedVersion, `tauri.conf.json 当前版本是 ${tauriConfig.version}`);
    return { message: `tauri.conf.json 版本 ${tauriConfig.version}`, tauriConfig };
  }, '请先同步 src-tauri/tauri.conf.json version');

  check('updaterKey', 'Updater 私钥', () => {
    assert(existsSync(path.join(config.sourceRepo, 'src-tauri', 'updater.key')), '缺少 src-tauri/updater.key');
    return 'updater.key 存在';
  }, '请确认 src-tauri/updater.key 是否在本机存在');

  check('ghCli', 'gh CLI', () => {
    assert(hasCommand('gh'), '缺少 gh CLI');
    return 'gh CLI 存在';
  }, '请安装 gh CLI 或确认 PATH');

  check('sshCli', 'ssh CLI', () => {
    assert(hasCommand('ssh'), '缺少 ssh');
    return 'ssh 存在';
  }, '请安装 ssh 或确认 PATH');

  check('scpCli', 'scp CLI', () => {
    assert(hasCommand('scp'), '缺少 scp');
    return 'scp 存在';
  }, '请安装 scp 或确认 PATH');

  check('githubAuth', 'GitHub 登录状态', () => {
    run('gh', ['auth', 'status'], config.sourceRepo);
    return 'gh auth status 通过';
  }, '请运行 gh auth login');

  check('windowsSsh', 'Windows 构建机 SSH', () => {
    run('ssh', ['-o', 'StrictHostKeyChecking=accept-new', '-o', 'BatchMode=yes', config.windowsSsh, 'powershell -NoProfile -Command "hostname; whoami"'], config.sourceRepo);
    return 'Windows SSH 连接成功';
  }, '请检查 4090 SSH 连通性和密钥配置');

  check('giteeCredential', 'Gitee 凭据', () => {
    assert(hasGiteeCredential(), '无法读取 Gitee 凭据');
    return 'Gitee 凭据可用';
  }, '请检查 git credential 中的 Gitee 凭据');

  check('giteeEndpoint', 'Gitee updater 端点', () => {
    const endpoints = tauriConfig.value?.tauriConfig?.plugins?.updater?.endpoints || [];
    assert(endpoints.includes(config.giteeLatest), '缺少 Gitee updater 端点');
    return 'Gitee updater 端点存在';
  }, '请检查 tauri updater endpoints');

  check('githubEndpoint', 'GitHub updater 端点', () => {
    const endpoints = tauriConfig.value?.tauriConfig?.plugins?.updater?.endpoints || [];
    assert(endpoints.includes(config.githubLatest), '缺少 GitHub updater 端点');
    return 'GitHub updater 端点存在';
  }, '请检查 tauri updater endpoints');

  if (failures.length > 0) {
    const message = `预检失败 ${failures.length} 项`;
    state.failStep('preflight', message, failures[0].suggestion);
    fail(message);
  }

  state.completeStep('preflight', '预检通过', { checks: state.loadState().checks.length });
  progress('预检通过');

  function check(key, label, fn, suggestion) {
    try {
      const result = fn();
      const message = typeof result === 'string' ? result : result.message;
      state.recordCheck({ key, label, status: 'success', message, suggestion: '' });
      return { ok: true, value: result };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      failures.push({ key, message, suggestion });
      state.recordCheck({ key, label, status: 'failed', message, suggestion });
      state.recordSuggestion(suggestion, 'error', key);
      return { ok: false, value: null };
    }
  }
}
```

- [ ] **Step 6: Update `checksums` and `manifests` to record state**

At the start of `checksums()`, add:

```js
state.startStep('artifactCheck', '检查发布产物');
const artifacts = inspectArtifacts(version);
state.recordArtifacts(artifacts);
const missing = artifacts.filter((artifact) => artifact.required && !artifact.exists);
if (missing.length > 0) {
  const message = `缺少 ${missing.length} 个必需产物或签名`;
  state.failStep('artifactCheck', message, '请补齐缺失产物后重新生成 manifest');
  fail(message);
}
state.completeStep('artifactCheck', '产物完整', { count: artifacts.length });
state.startStep('checksums', '生成 checksums.sha256');
```

Before `progress(`已写入 ${path.join(releaseDir, 'checksums.sha256')}`);`, add:

```js
state.completeStep('checksums', 'checksums.sha256 已生成', { count: files.length });
```

In `manifests(expectedVersion)`, before writing each host manifest, add:

```js
state.startStep(host === 'github' ? 'manifestGithub' : 'manifestGitee', `生成 ${host} manifest`);
```

After `validateManifest(file);`, add:

```js
state.recordManifest(host, manifestSummaryForState(manifest));
state.completeStep(host === 'github' ? 'manifestGithub' : 'manifestGitee', `${host} manifest 已生成`, manifestSummaryForState(manifest));
```

Add helper:

```js
function manifestSummaryForState(manifest) {
  return {
    exists: true,
    version: manifest.version || '',
    pubDate: manifest.pub_date || '',
    platforms: Object.entries(manifest.platforms || {}).map(([key, value]) => ({
      key,
      url: value.url || '',
      hasSignature: Boolean(value.signature),
    })),
  };
}
```

- [ ] **Step 7: Update `verify` to record endpoint results**

Replace `verify(expectedVersion)` with:

```js
function verify(expectedVersion) {
  state.startStep('verify', '验证 updater 端点');
  const results = [];

  for (const endpoint of [config.giteeLatest, config.githubLatest]) {
    try {
      const manifest = fetchJson(endpoint);
      assert(manifest.version === expectedVersion, `${endpoint} 返回版本 ${manifest.version}`);
      const platforms = [];
      for (const [key, value] of Object.entries(manifest.platforms || {})) {
        assert(value.signature, `${key} 缺少 signature`);
        const status = httpStatus(value.url);
        assert(status === 200, `${value.url} 无法访问`);
        platforms.push({ key, url: value.url, httpStatus: status, hasSignature: true });
      }
      results.push({ endpoint, status: 'success', version: manifest.version, platforms });
      progress(`已验证 ${endpoint}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      results.push({ endpoint, status: 'failed', error: message, platforms: [] });
      state.recordSuggestion(`请检查 updater endpoint：${endpoint}`, 'error', 'verify');
    }
  }

  if (results.some((result) => result.status === 'failed')) {
    state.failStep('verify', '线上验证失败', '请检查失败的 updater endpoint 或下载 URL');
    state.recordManifest('onlineVerify', { results });
    fail('线上验证失败');
  }

  state.recordManifest('onlineVerify', { results });
  state.completeStep('verify', '线上验证通过', { endpoints: results.length });
}
```

- [ ] **Step 8: Update `release` dry-run state and blocked real release**

In `release(expectedVersion)`, inside `if (args.dryRun)`, before the loop add:

```js
state.startStep('dryRunRelease', '生成 dry-run 发布命令');
const commands = dryRunCommands(expectedVersion);
```

Change the loop to use `commands`:

```js
for (const commandLine of commands) {
  console.log(`[试运行] ${commandLine}`);
}
state.completeStep('dryRunRelease', 'dry-run 发布命令已生成', { commands });
return;
```

Before the final `fail('当前版本为安全第一版...')`, add:

```js
state.failStep('dryRunRelease', '真实发布在 Phase 1 未启用', '请先使用 --dry-run 核对命令；真实发布将在后续阶段启用');
```

- [ ] **Step 9: Run CLI syntax check**

Run:

```bash
node --check scripts/omd-release.mjs
```

Expected: command exits successfully with no output.

## Task 4: Add Safe Action APIs to Server

**Files:**
- Modify: `server.mjs`
- Test: `node --check server.mjs`

- [ ] **Step 1: Add imports**

At the top of `server.mjs`, add:

```js
import { spawn } from 'node:child_process';
import { loadReleaseConfig, configSummary, expandHome } from './scripts/release-config.mjs';
```

Change the existing `import { existsSync } from 'node:fs';` to:

```js
import { existsSync } from 'node:fs';
```

Keep it unchanged if already identical.

Add after `const port = ...`:

```js
const releaseConfig = loadReleaseConfig();
```

Remove the local `expandHome` function at the bottom of `server.mjs` after replacing its usage with the imported function.

- [ ] **Step 2: Add routing for config and action endpoints**

Inside the request handler, after `/api/commands`, add:

```js
    if (url.pathname === '/api/config') {
      return sendJson(response, { ok: true, config: configSummary(releaseConfig) });
    }

    if (url.pathname.startsWith('/api/actions/')) {
      if (request.method !== 'POST') {
        return sendJson(response, { ok: false, error: 'Action API 只支持 POST' }, 405);
      }
      const action = url.pathname.replace('/api/actions/', '');
      const body = await readJsonBody(request);
      return sendJson(response, await runAction(action, body));
    }
```

- [ ] **Step 3: Add action execution helpers**

Add these functions before `serveStatic`:

```js
async function runAction(action, body) {
  if (action === 'publish-github' || action === 'publish-gitee') {
    return {
      ok: false,
      action,
      status: 'disabled',
      message: '真实发布在 Phase 1 未启用',
      state: null,
      logs: { stdout: '', stderr: '' },
      suggestions: [{ severity: 'info', message: '请先使用 dry-run-release 核对命令', action: 'dry-run-release' }],
    };
  }

  const commandArgsByAction = {
    preflight: (version, releaseDir) => ['scripts/omd-release.mjs', 'preflight', version, '--release-dir', releaseDir],
    manifest: (version, releaseDir) => ['scripts/omd-release.mjs', 'manifest', version, '--release-dir', releaseDir],
    verify: (version, releaseDir) => ['scripts/omd-release.mjs', 'verify', version, '--release-dir', releaseDir],
    'dry-run-release': (version, releaseDir) => ['scripts/omd-release.mjs', 'release', version, '--release-dir', releaseDir, '--dry-run'],
  };

  if (!commandArgsByAction[action]) {
    return actionError(action, 'unknown_action', `未知 action：${action}`);
  }

  const validation = validateActionInput(body);
  if (!validation.ok) {
    return actionError(action, 'invalid_input', validation.message);
  }

  const { version, releaseDir } = validation;
  const logs = await runNodeCommand(commandArgsByAction[action](version, releaseDir));
  const state = await readJsonIfExists(path.join(releaseDir, 'state.json'));
  const ok = logs.exitCode === 0;

  return {
    ok,
    action,
    status: ok ? 'success' : 'failed',
    message: ok ? `${action} 完成` : `${action} 失败`,
    state,
    logs: { stdout: logs.stdout, stderr: logs.stderr },
    suggestions: state?.suggestions || [],
  };
}

function validateActionInput(body) {
  const version = String(body?.version || '').trim();
  const releaseDirValue = String(body?.releaseDir || '').trim();

  if (!isVersion(version)) {
    return { ok: false, message: '请提供 SemVer 版本号，例如 0.0.7。' };
  }

  if (!releaseDirValue) {
    return { ok: false, message: '请提供 releaseDir。' };
  }

  const releaseDir = path.resolve(expandHome(releaseDirValue));
  if (!isAllowedReleaseDir(releaseDir, version)) {
    return { ok: false, message: 'releaseDir 必须位于桌面的 omd-<version>-release-* 目录。' };
  }

  return { ok: true, version, releaseDir };
}

function isAllowedReleaseDir(releaseDir, version) {
  const expectedParent = desktopDir;
  const relative = path.relative(expectedParent, releaseDir);
  return Boolean(relative)
    && !relative.startsWith('..')
    && !path.isAbsolute(relative)
    && new RegExp(`^omd-${escapeRegExp(version)}-release-.+`).test(path.basename(releaseDir));
}

function runNodeCommand(commandArgs) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, commandArgs, { cwd: __dirname, encoding: 'utf8' });
    let stdout = '';
    let stderr = '';

    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('close', (exitCode) => resolve({ exitCode, stdout, stderr }));
  });
}

function actionError(action, status, message) {
  return {
    ok: false,
    action,
    status,
    message,
    state: null,
    logs: { stdout: '', stderr: '' },
    suggestions: [{ severity: 'error', message, action }],
  };
}

async function readJsonBody(request) {
  let raw = '';
  for await (const chunk of request) {
    raw += chunk;
    if (raw.length > 1024 * 1024) {
      throw new Error('请求体过大');
    }
  }
  return raw ? JSON.parse(raw) : {};
}

function isVersion(value) {
  return /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(value || '');
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
```

- [ ] **Step 4: Run server syntax check**

Run:

```bash
node --check server.mjs
```

Expected: command exits successfully with no output.

## Task 5: Make Summary Understand New State and Required Artifacts

**Files:**
- Modify: `server.mjs`
- Test: manual API calls

- [ ] **Step 1: Include config required artifacts in summary**

In `summarizeRelease`, after `const normalized = ...`, compute required artifacts:

```js
  const requiredArtifacts = requiredArtifactSummaries(normalized, state?.version || versionFromReleaseDir(normalized) || '');
```

Because `state` is loaded later in the current Promise.all, first change the Promise.all block to read `state` before computing artifacts. Replace the current Promise.all with:

```js
  const state = await readJsonIfExists(path.join(normalized, 'state.json'));
  const version = state?.version || versionFromReleaseDir(normalized) || '';
  const [githubManifest, giteeManifest, checksums, artifacts] = await Promise.all([
    readJsonIfExists(path.join(normalized, 'github', 'latest.json')),
    readJsonIfExists(path.join(normalized, 'gitee', 'latest.json')),
    readLinesIfExists(path.join(normalized, 'checksums.sha256')),
    listArtifacts(normalized),
  ]);
  const requiredArtifacts = requiredArtifactSummaries(normalized, version);
```

Return `requiredArtifacts` in the summary object:

```js
    requiredArtifacts,
```

- [ ] **Step 2: Add required artifact helper functions**

Add before `listArtifacts`:

```js
function requiredArtifactSummaries(releaseDir, version) {
  if (!version) return [];
  return releaseConfig.requiredArtifacts.flatMap((target) => {
    const fileName = target.file.replaceAll('{version}', version);
    const sigName = target.signature.replaceAll('{version}', version);
    return [artifactSummary(releaseDir, target.scope, fileName, true, ''), artifactSummary(releaseDir, target.scope, sigName, true, fileName)];
  });
}

function artifactSummary(releaseDir, scope, name, required, signatureFor) {
  const file = path.join(releaseDir, scope, name);
  if (!existsSync(file)) {
    return { scope, name, required, exists: false, size: 0, modifiedAt: '', signatureFor };
  }
  const info = statSync(file);
  return {
    scope,
    name,
    required,
    exists: true,
    size: info.size,
    modifiedAt: new Date(info.mtimeMs).toISOString(),
    signatureFor,
  };
}

function versionFromReleaseDir(releaseDir) {
  const match = path.basename(releaseDir).match(/^omd-(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)-release-/);
  return match?.[1] || '';
}
```

Also update the import from `node:fs/promises` or `node:fs` so `statSync` is available. Change:

```js
import { existsSync } from 'node:fs';
```

to:

```js
import { existsSync, statSync } from 'node:fs';
```

- [ ] **Step 3: Run API syntax check**

Run:

```bash
node --check server.mjs
```

Expected: command exits successfully with no output.

## Task 6: Update Frontend Markup for Release Preparation Center

**Files:**
- Modify: `public/index.html`
- Test: browser render after Task 7

- [ ] **Step 1: Update sidebar navigation**

In `public/index.html`, replace the nav list with:

```html
<nav class="nav-list">
  <a href="#overview">概览</a>
  <a href="#actions">准备动作</a>
  <a href="#manifests">Manifest</a>
  <a href="#artifacts">产物</a>
  <a href="#commands">命令</a>
</nav>
```

- [ ] **Step 2: Add version input to overview panel**

Inside the `#overview` panel, before the existing `.release-input`, add:

```html
<div class="release-input version-row">
  <input id="versionInput" value="0.0.7" placeholder="目标版本，例如 0.0.7" />
  <button id="commandButton" type="button">生成命令</button>
</div>
```

Remove the duplicate `versionInput` and `commandButton` from the commands panel in Step 7.

- [ ] **Step 3: Add config summary panel**

After the overview panel, add:

```html
<section class="panel">
  <div class="panel-heading">
    <div>
      <p class="eyebrow">Config</p>
      <h3>发布配置摘要</h3>
    </div>
  </div>
  <div id="configSummary" class="config-grid"></div>
</section>
```

- [ ] **Step 4: Add action cards panel**

After the config panel, add:

```html
<section id="actions" class="panel">
  <div class="panel-heading">
    <div>
      <p class="eyebrow">Actions</p>
      <h3>发布准备动作</h3>
    </div>
  </div>
  <div id="actionList" class="action-grid"></div>
</section>
```

- [ ] **Step 5: Add suggestions and logs panel**

After the action cards panel, add:

```html
<section class="two-column">
  <div class="panel">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">Suggestions</p>
        <h3>下一步建议</h3>
      </div>
    </div>
    <div id="suggestionList" class="suggestion-list"></div>
  </div>

  <div class="panel">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">Logs</p>
        <h3>最近动作日志</h3>
      </div>
    </div>
    <pre id="actionLogOutput">暂无动作日志</pre>
  </div>
</section>
```

- [ ] **Step 6: Add manifest anchor**

Change the manifest panel wrapper in the existing two-column section from:

```html
<div class="panel">
```

to:

```html
<div id="manifests" class="panel">
```

- [ ] **Step 7: Simplify commands panel tools**

In the commands panel, remove this block:

```html
<div class="command-tools">
  <input id="versionInput" value="0.0.7" />
  <button id="commandButton" type="button">生成命令</button>
</div>
<pre id="commandOutput"></pre>
```

Replace it with:

```html
<div id="commandOutput" class="command-card-list"></div>
```

## Task 7: Update Frontend Behavior

**Files:**
- Modify: `public/app.js`
- Test: browser manual validation

- [ ] **Step 1: Replace `elements` with new DOM bindings**

Replace the `elements` object with:

```js
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
```

- [ ] **Step 2: Add action metadata and initial load**

After `flowSteps`, add:

```js
const releaseActions = [
  { key: 'preflight', label: '预检', description: '检查源码版本、凭据、SSH 和 updater 端点。', risk: '低风险', runnable: true },
  { key: 'manifest', label: '生成 Manifest', description: '检查产物，生成 checksums、GitHub/Gitee latest.json。', risk: '低风险', runnable: true },
  { key: 'verify', label: '验证端点', description: '验证线上 updater endpoint 和下载 URL。', risk: '低风险', runnable: true },
  { key: 'dry-run-release', stepKey: 'dryRunRelease', label: 'Dry-run 发布', description: '输出真实发布命令但不执行上传。', risk: '低风险', runnable: true },
  { key: 'publish-github', stepKey: 'publishGithub', label: '发布 GitHub', description: '真实发布在 Phase 1 禁用。', risk: '高风险', runnable: false },
  { key: 'publish-gitee', stepKey: 'publishGitee', label: '发布 Gitee', description: '真实发布在 Phase 1 禁用。', risk: '高风险', runnable: false },
];
```

After `renderFlow();`, add:

```js
loadConfig();
renderActions({});
```

- [ ] **Step 3: Update event listeners**

After existing listeners, add:

```js
elements.actionList.addEventListener('click', async (event) => {
  const button = event.target.closest('button[data-action]');
  if (!button) return;
  await runAction(button.dataset.action);
});
```

- [ ] **Step 4: Add config and action API functions**

Add after `loadCommands`:

```js
async function loadConfig() {
  const data = await fetchJson('/api/config');
  renderConfig(data.config);
}

async function runAction(action) {
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
}
```

- [ ] **Step 5: Update `loadSummary` to render new state fields**

At the end of `renderSummary(data)`, add:

```js
  renderState(data.state || {});
  renderRequiredArtifacts(data.requiredArtifacts || [], data.artifacts || []);
  renderSuggestions(data.state?.suggestions || []);
```

- [ ] **Step 6: Add render helpers**

Add these functions before `hostLabel`:

```js
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
  const recent = Object.values(steps).filter((step) => step.startedAt || step.endedAt).sort((left, right) => String(right.endedAt || right.startedAt).localeCompare(String(left.endedAt || left.startedAt))).slice(0, 8);
  elements.checkpointList.innerHTML = recent.map((step) => `
    <article class="checkpoint-item">
      <strong>${checkpointLabel(step.key)}</strong>
      <p class="muted">${statusLabel(step.status)} · ${formatDate(step.endedAt || step.startedAt)}</p>
    </article>
  `).join('') || emptyText('暂无状态文件');
}

function renderRequiredArtifacts(requiredArtifacts, actualArtifacts) {
  const actualKeys = new Set(actualArtifacts.map((artifact) => `${artifact.scope}/${artifact.name}`));
  const requiredRows = requiredArtifacts.map((artifact) => ({ ...artifact, required: true }));
  const extraRows = actualArtifacts.filter((artifact) => !actualKeys.has(`${artifact.scope}/${artifact.name}`));
  const rows = requiredRows.length ? requiredRows : actualArtifacts;

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
```

- [ ] **Step 7: Update command rendering**

Replace `loadCommands` with:

```js
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
        <code>${command}</code>
        <button type="button" data-copy="${escapeHtml(command)}">复制</button>
      </article>
    `;
  }).join('');
}
```

Add copy listener after the action listener:

```js
elements.commandOutput.addEventListener('click', async (event) => {
  const button = event.target.closest('button[data-copy]');
  if (!button) return;
  await navigator.clipboard.writeText(button.dataset.copy);
  button.textContent = '已复制';
});
```

Add helpers:

```js
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

function formatDuration(ms) {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function escapeHtml(value) {
  return value.replace(/[&<>"]/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[char]));
}
```

- [ ] **Step 8: Update `fetchJson` to accept options**

Replace `fetchJson` with:

```js
async function fetchJson(url, options) {
  const response = await fetch(url, options);
  const data = await response.json();
  if (!response.ok || data.ok === false) {
    throw new Error(data.message || data.error || `请求失败：${response.status}`);
  }
  return data;
}
```

Wrap `runAction` body in try/catch:

```js
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
```

## Task 8: Update Frontend Styles

**Files:**
- Modify: `public/styles.css`
- Test: browser visual check

- [ ] **Step 1: Add disabled button style**

After the existing `button:hover` block, add:

```css
button:disabled {
  border-color: var(--border);
  background: #e5e7eb;
  color: var(--muted);
  cursor: not-allowed;
}
```

- [ ] **Step 2: Update responsive nav columns**

In the media query, replace:

```css
.nav-list {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}
```

with:

```css
.nav-list {
  grid-template-columns: repeat(5, minmax(0, 1fr));
}
```

- [ ] **Step 3: Add card grids and state styles**

Before the `pre` block, add:

```css
.config-grid,
.action-grid,
.command-card-list,
.suggestion-list {
  display: grid;
  gap: 0.65rem;
}

.config-grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.action-grid {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.config-item,
.action-card,
.command-card,
.suggestion-item {
  border: 1px solid var(--border);
  background: var(--panel-soft);
  border-radius: 8px;
  padding: 0.75rem;
}

.config-item {
  display: grid;
  gap: 0.25rem;
}

.config-item span {
  color: var(--muted);
  font-size: 0.78rem;
}

.config-item strong {
  overflow-wrap: anywhere;
  font-size: 0.9rem;
}

.action-card,
.command-card {
  display: grid;
  gap: 0.65rem;
}

.action-card header,
.command-card header {
  display: flex;
  justify-content: space-between;
  gap: 0.75rem;
}

.command-card code {
  display: block;
  overflow-wrap: anywhere;
  border-radius: 6px;
  background: #111827;
  color: #e5e7eb;
  padding: 0.65rem;
  font-family: "SF Mono", ui-monospace, monospace;
  font-size: 0.78rem;
}

.suggestion-item {
  display: flex;
  align-items: flex-start;
  gap: 0.6rem;
}

.suggestion-item p {
  margin: 0;
}

.danger-text {
  color: var(--danger);
  margin: 0;
}

.artifact-row.missing {
  border-color: #fecaca;
  background: #fff7f7;
}
```

- [ ] **Step 4: Add responsive card styles**

Inside `@media (max-width: 980px)`, add:

```css
.config-grid,
.action-grid {
  grid-template-columns: 1fr;
}
```

## Task 9: End-to-End Verification

**Files:**
- Modify only if verification finds defects.
- Test: CLI and local browser.

- [ ] **Step 1: Run full syntax check**

Run:

```bash
npm run check
```

Expected: all `node --check` commands pass with no output except npm script headers.

- [ ] **Step 2: Start the app**

Run:

```bash
npm run dev
```

Expected: server prints:

```text
OMD 发布控制台已启动：http://127.0.0.1:4177
```

- [ ] **Step 3: Verify config API**

Run in another terminal:

```bash
curl -sS http://127.0.0.1:4177/api/config
```

Expected JSON contains `ok: true`, `config.sourceRepo`, `config.githubRepo`, `config.requiredArtifacts`, and does not contain tokens, passwords, or private key material.

- [ ] **Step 4: Verify invalid action input is rejected before CLI execution**

Run:

```bash
curl -sS -X POST http://127.0.0.1:4177/api/actions/preflight \
  -H 'Content-Type: application/json' \
  -d '{"version":"bad","releaseDir":"/tmp/not-allowed"}'
```

Expected JSON:

```json
{
  "ok": false,
  "action": "preflight",
  "status": "invalid_input"
}
```

- [ ] **Step 5: Verify disabled publish placeholder**

Run:

```bash
curl -sS -X POST http://127.0.0.1:4177/api/actions/publish-github \
  -H 'Content-Type: application/json' \
  -d '{"version":"0.0.7","releaseDir":"/Users/glame/Desktop/omd-0.0.7-release-test","confirmVersion":"0.0.7"}'
```

Expected JSON contains:

```json
{
  "ok": false,
  "action": "publish-github",
  "status": "disabled",
  "message": "真实发布在 Phase 1 未启用"
}
```

- [ ] **Step 6: Verify dry-run action writes state without real upload**

Create an allowed release directory if needed:

```bash
mkdir -p ~/Desktop/omd-0.0.7-release-test
```

Run:

```bash
curl -sS -X POST http://127.0.0.1:4177/api/actions/dry-run-release \
  -H 'Content-Type: application/json' \
  -d "{\"version\":\"0.0.7\",\"releaseDir\":\"$HOME/Desktop/omd-0.0.7-release-test\"}"
```

Expected: response may fail if preflight dependencies are unavailable, but `~/Desktop/omd-0.0.7-release-test/state.json` exists and contains `steps.publishGithub.status: "disabled"` and `steps.publishGitee.status: "disabled"`. If preflight passes, `steps.dryRunRelease.summary.commands` contains commands and no real upload is performed.

- [ ] **Step 7: Browser manual validation**

Open `http://127.0.0.1:4177` in a browser and verify:

- Config summary renders.
- Action cards render four runnable low-risk actions and two disabled publish actions.
- Invalid action input shows an actionable error in the logs and suggestions panel.
- Loading an allowed release directory refreshes summary and required artifact rows.
- Command cards render with low/high risk badges and copy buttons.

---

## Self-Review

- Spec coverage: config externalization, standard state, safe action APIs, disabled publish placeholders, frontend action cards, artifact/manifest display, logs/suggestions, and verification are covered by Tasks 1-9.
- Placeholder scan: no TBD/TODO/fill-in placeholders remain; each code step includes concrete code or an exact command.
- Type consistency: action names, step keys, state fields, and API response fields are consistent across CLI, server, and frontend tasks.
