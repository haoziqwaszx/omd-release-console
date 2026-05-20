import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { readdir, readFile, stat } from 'node:fs/promises';
import { existsSync, statSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadReleaseConfig, configSummary, expandHome } from './scripts/release-config.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const publicDir = path.join(__dirname, 'public');
const desktopDir = path.join(os.homedir(), 'Desktop');
const port = Number(process.env.PORT || 4177);
const releaseConfig = loadReleaseConfig();

const mimeTypes = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
};

createServer(async (request, response) => {
  try {
    const url = new URL(request.url, `http://${request.headers.host}`);

    if (url.pathname === '/api/summary') {
      const releaseDir = url.searchParams.get('releaseDir') || await findLatestReleaseDir();
      return sendJson(response, await summarizeRelease(releaseDir));
    }

    if (url.pathname === '/api/commands') {
      return sendJson(response, releaseCommands(url.searchParams.get('version') || '0.0.7'));
    }

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

    return serveStatic(response, url.pathname);
  } catch (error) {
    sendJson(response, { ok: false, error: error.message }, 500);
  }
}).listen(port, '127.0.0.1', () => {
  console.log(`OMD 发布控制台已启动：http://127.0.0.1:${port}`);
});

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
  const relative = path.relative(desktopDir, releaseDir);
  return Boolean(relative)
    && !relative.startsWith('..')
    && !path.isAbsolute(relative)
    && new RegExp(`^omd-${escapeRegExp(version)}-release-.+`).test(path.basename(releaseDir));
}

function runNodeCommand(commandArgs) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, commandArgs, { cwd: __dirname });
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

async function serveStatic(response, pathname) {
  const safePath = pathname === '/' ? '/index.html' : pathname;
  const file = path.normalize(path.join(publicDir, safePath));

  if (!file.startsWith(publicDir)) {
    response.writeHead(403);
    response.end('Forbidden');
    return;
  }

  const target = existsSync(file) ? file : path.join(publicDir, 'index.html');
  const body = await readFile(target);
  response.writeHead(200, {
    'Content-Type': mimeTypes[path.extname(target)] || 'application/octet-stream',
  });
  response.end(body);
}

async function findLatestReleaseDir() {
  const entries = await readdir(desktopDir, { withFileTypes: true });
  const candidates = [];

  for (const entry of entries) {
    if (!entry.isDirectory() || !/^omd-.+-release-/.test(entry.name)) {
      continue;
    }
    const fullPath = path.join(desktopDir, entry.name);
    const info = await stat(fullPath);
    candidates.push({ fullPath, mtimeMs: info.mtimeMs });
  }

  candidates.sort((left, right) => right.mtimeMs - left.mtimeMs);
  return candidates[0]?.fullPath || '';
}

async function summarizeRelease(releaseDir) {
  if (!releaseDir) {
    return {
      ok: true,
      releaseDir: '',
      message: '没有找到发布目录',
      manifests: {},
      artifacts: [],
      requiredArtifacts: [],
      checksums: [],
      state: null,
    };
  }

  const normalized = path.resolve(expandHome(releaseDir));
  const state = await readJsonIfExists(path.join(normalized, 'state.json'));
  const version = state?.version || versionFromReleaseDir(normalized) || '';
  const [githubManifest, giteeManifest, checksums, artifacts] = await Promise.all([
    readJsonIfExists(path.join(normalized, 'github', 'latest.json')),
    readJsonIfExists(path.join(normalized, 'gitee', 'latest.json')),
    readLinesIfExists(path.join(normalized, 'checksums.sha256')),
    listArtifacts(normalized),
  ]);
  const requiredArtifacts = requiredArtifactSummaries(normalized, version);

  return {
    ok: true,
    releaseDir: normalized,
    manifests: {
      github: manifestSummary(githubManifest),
      gitee: manifestSummary(giteeManifest),
    },
    artifacts,
    requiredArtifacts,
    checksums: checksums.slice(0, 80),
    state,
  };
}

function requiredArtifactSummaries(releaseDir, version) {
  if (!version) return [];
  return releaseConfig.requiredArtifacts.flatMap((target) => {
    const fileName = target.file.replaceAll('{version}', version);
    const sigName = target.signature.replaceAll('{version}', version);
    return [
      artifactSummary(releaseDir, target.scope, fileName, true, ''),
      artifactSummary(releaseDir, target.scope, sigName, true, fileName),
    ];
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

async function listArtifacts(releaseDir) {
  const scopes = ['windows', 'mac'];
  const files = [];

  for (const scope of scopes) {
    const dir = path.join(releaseDir, scope);
    if (!existsSync(dir)) continue;
    const entries = await readdir(dir, { withFileTypes: true });
    for (const entry of entries) {
      if (!entry.isFile()) continue;
      const fullPath = path.join(dir, entry.name);
      const info = await stat(fullPath);
      files.push({
        scope,
        name: entry.name,
        size: info.size,
        modifiedAt: new Date(info.mtimeMs).toISOString(),
      });
    }
  }

  return files.sort((left, right) => left.scope.localeCompare(right.scope) || left.name.localeCompare(right.name));
}

function manifestSummary(manifest) {
  if (!manifest) {
    return { exists: false, version: '', platforms: [] };
  }

  return {
    exists: true,
    version: manifest.version || '',
    notes: manifest.notes || '',
    pubDate: manifest.pub_date || '',
    platforms: Object.entries(manifest.platforms || {}).map(([key, value]) => ({
      key,
      hasSignature: Boolean(value.signature),
      url: value.url || '',
    })),
  };
}

function releaseCommands(version) {
  return {
    version,
    commands: [
      `node scripts/omd-release.mjs plan`,
      `node scripts/omd-release.mjs preflight ${version}`,
      `node scripts/omd-release.mjs release ${version} --dry-run`,
      `node scripts/omd-release.mjs release ${version}`,
      `node scripts/omd-release.mjs verify ${version} --release-dir ~/Desktop/omd-${version}-release-YYYYMMDD-HHMMSS`,
    ],
  };
}

async function readJsonIfExists(file) {
  if (!existsSync(file)) return null;
  return JSON.parse(await readFile(file, 'utf8'));
}

async function readLinesIfExists(file) {
  if (!existsSync(file)) return [];
  return (await readFile(file, 'utf8')).split('\n').filter(Boolean);
}

function isVersion(value) {
  return /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(value || '');
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function sendJson(response, payload, status = 200) {
  response.writeHead(status, { 'Content-Type': 'application/json; charset=utf-8' });
  response.end(JSON.stringify(payload, null, 2));
}
