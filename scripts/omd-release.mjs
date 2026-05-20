#!/usr/bin/env node
import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import os from 'node:os';
import path from 'node:path';
import { loadReleaseConfig, artifactName, expandHome } from './release-config.mjs';
import { createStateManager } from './release-state.mjs';

const config = loadReleaseConfig();
const [command, version, ...rawArgs] = process.argv.slice(2);
const args = parseArgs(rawArgs);

if (!command || args.help) {
  help();
  process.exit(0);
}

if (command !== 'plan' && !isVersion(version)) {
  fail('请提供 SemVer 版本号，例如 0.0.7。');
}

const releaseDir = command === 'plan'
  ? ''
  : path.resolve(expandHome(args.releaseDir || path.join(os.homedir(), 'Desktop', `omd-${version}-release-${timestamp()}`)));
const state = command === 'plan'
  ? null
  : createStateManager({
      releaseDir,
      version: version || '',
      sourceRepo: config.sourceRepo,
      commit: hasGitRepo(config.sourceRepo) ? safeExec('git', ['rev-parse', 'HEAD'], config.sourceRepo) : '',
    });

if (state) {
  state.setStepDisabled('publishGithub', '真实发布在 Phase 1 未启用');
  state.setStepDisabled('publishGitee', '真实发布在 Phase 1 未启用');
}

switch (command) {
  case 'plan':
    plan();
    break;
  case 'preflight':
    preflight(version);
    break;
  case 'manifest':
    checksums();
    manifests(version);
    break;
  case 'verify':
    verify(version);
    break;
  case 'release':
    release(version);
    break;
  default:
    fail(`未知命令：${command}`);
}

function help() {
  console.log(`OMD 发布 CLI

用法：
  node scripts/omd-release.mjs plan
  node scripts/omd-release.mjs preflight 0.0.7
  node scripts/omd-release.mjs manifest 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release
  node scripts/omd-release.mjs verify 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release
  node scripts/omd-release.mjs release 0.0.7 --dry-run

说明：
  第一版 CLI 负责预检、manifest、校验和、端点验证，以及打印双端发布命令。
  真正上传和远程构建命令会在 --dry-run 中展示，后续可继续接入一键执行。`);
}

function plan() {
  console.log(`OMD 双端发布流程：
1. Mac 预检源码、凭据、4090 SSH 和 updater 端点。
2. Mac 提交并推送产品源码版本。
3. 4090 从远端 commit 创建干净 worktree。
4. 4090 构建并签名 NSIS、MSI、绿色 zip。
5. Mac 用 scp 拉回 Windows 产物。
6. Mac 本机构建 dmg 和 app.tar.gz。
7. Mac 统一生成 checksums、GitHub latest.json、Gitee latest.json。
8. Mac 发布 GitHub Release 并标记 latest。
9. Mac 发布 Gitee Release 并更新根 latest.json。
10. Mac 验证两个 updater 端点和每个下载 URL。`);
}

function preflight(expectedVersion) {
  state.startStep('preflight', '开始预检');
  progress('开始预检');
  const failures = [];

  check('sourcePackageVersion', '源码 package.json 版本', () => {
    const packageJson = json(path.join(config.sourceRepo, 'package.json'));
    assert(packageJson.version === expectedVersion, `package.json 当前版本是 ${packageJson.version}`);
    return `package.json 版本 ${packageJson.version}`;
  }, '请先同步 package.json version');

  const tauriConfig = check('tauriVersion', 'Tauri 配置版本', () => {
    const value = json(path.join(config.sourceRepo, 'src-tauri', 'tauri.conf.json'));
    assert(value.version === expectedVersion, `tauri.conf.json 当前版本是 ${value.version}`);
    return { message: `tauri.conf.json 版本 ${value.version}`, value };
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
    const endpoints = tauriConfig.value?.value?.plugins?.updater?.endpoints || [];
    assert(endpoints.includes(config.giteeLatest), '缺少 Gitee updater 端点');
    return 'Gitee updater 端点存在';
  }, '请检查 tauri updater endpoints');

  check('githubEndpoint', 'GitHub updater 端点', () => {
    const endpoints = tauriConfig.value?.value?.plugins?.updater?.endpoints || [];
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
      return { ok: true, value: result.value ?? result };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      failures.push({ key, message, suggestion });
      state.recordCheck({ key, label, status: 'failed', message, suggestion });
      state.recordSuggestion(suggestion, 'error', key);
      return { ok: false, value: null };
    }
  }
}

function release(expectedVersion) {
  preflight(expectedVersion);
  mkdirSync(releaseDir, { recursive: true });
  progress(`发布目录：${releaseDir}`);

  if (args.dryRun) {
    state.startStep('dryRunRelease', '生成 dry-run 发布命令');
    const commands = dryRunCommands(expectedVersion);
    for (const commandLine of commands) {
      console.log(`[试运行] ${commandLine}`);
    }
    state.completeStep('dryRunRelease', 'dry-run 发布命令已生成', { commands });
    return;
  }

  state.failStep('dryRunRelease', '真实发布在 Phase 1 未启用', '请先使用 --dry-run 核对命令；真实发布将在后续阶段启用');
  fail('当前版本为安全第一版：请先使用 --dry-run 核对命令，再逐步接入真实执行。');
}

function dryRunCommands(expectedVersion) {
  const commit = exec('git', ['rev-parse', 'HEAD'], config.sourceRepo);
  const worktree = `C:\\Users\\4090\\Desktop\\dix-extension-ui-release-${expectedVersion}-${timestamp()}`;
  return [
    `ssh ${config.windowsSsh} "powershell -NoProfile -NonInteractive -Command \\"git -C ${config.windowsRepo} fetch --prune origin; git -C ${config.windowsRepo} worktree add --detach ${worktree} ${commit}\\""`,
    `ssh ${config.windowsSsh} "powershell -NoProfile -NonInteractive -Command \\"cd ${worktree}; npm ci; npm run lint; npm run build; npx tauri build --bundles nsis,msi --target ${config.windowsTarget}\\""`,
    `scp ${config.windowsSsh}:/C:/Users/4090/Desktop/.../OMD_${expectedVersion}_x64-setup.exe ${releaseDir}/windows/`,
    `npm --prefix ${config.sourceRepo} run tauri:build:mac`,
    `node scripts/omd-release.mjs manifest ${expectedVersion} --release-dir ${releaseDir}`,
    `gh release create v${expectedVersion} ${releaseDir}/github/latest.json ${releaseDir}/windows/* ${releaseDir}/mac/* -R ${config.githubRepo} --title "OMD ${expectedVersion}"`,
    `node scripts/omd-release.mjs verify ${expectedVersion} --release-dir ${releaseDir}`,
  ];
}

function checksums() {
  mkdirSync(releaseDir, { recursive: true });
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
  const files = listFiles(releaseDir).filter((file) => !file.endsWith('checksums.sha256'));
  const body = files.map((file) => `${sha256(file)}  ${path.relative(releaseDir, file)}`).join('\n');
  writeFileSync(path.join(releaseDir, 'checksums.sha256'), body ? `${body}\n` : '', 'utf8');
  state.completeStep('checksums', 'checksums.sha256 已生成', { count: files.length });
  progress(`已写入 ${path.join(releaseDir, 'checksums.sha256')}`);
}

function manifests(expectedVersion) {
  for (const host of ['github', 'gitee']) {
    state.startStep(host === 'github' ? 'manifestGithub' : 'manifestGitee', `生成 ${host} manifest`);
    const dir = path.join(releaseDir, host);
    mkdirSync(dir, { recursive: true });
    const manifest = manifestFor(expectedVersion, host);
    const file = path.join(dir, 'latest.json');
    writeFileSync(file, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
    validateManifest(file);
    const summary = manifestSummaryForState(manifest);
    state.recordManifest(host, summary);
    state.completeStep(host === 'github' ? 'manifestGithub' : 'manifestGitee', `${host} manifest 已生成`, summary);
    progress(`已写入 ${file}`);
  }
}

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

function manifestFor(expectedVersion, host) {
  const base = host === 'github'
    ? `https://github.com/${config.githubRepo}/releases/download/v${expectedVersion}`
    : `https://gitee.com/${config.giteeOwner}/${config.giteeRepo}/releases/download/v${expectedVersion}`;
  const platforms = {};

  for (const target of config.requiredArtifacts) {
    const sigPath = path.join(releaseDir, target.scope, artifactName(target.signature, expectedVersion));
    assert(existsSync(sigPath), `缺少签名文件：${sigPath}`);
    platforms[target.key] = {
      signature: readFileSync(sigPath, 'utf8').replace(/\r?\n/g, ''),
      url: `${base}/${artifactName(target.file, expectedVersion)}`,
    };
  }

  return {
    version: expectedVersion,
    notes: `OMD ${expectedVersion}`,
    pub_date: new Date().toISOString(),
    platforms,
  };
}

function validateManifest(file) {
  const bytes = readFileSync(file);
  assert(!(bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf), `${file} 含有 BOM`);
  const manifest = JSON.parse(bytes.toString('utf8'));
  assert(!manifest.platforms['windows-x86_64'], '不能发布通用 windows-x86_64 target');
  for (const target of config.requiredArtifacts) {
    assert(manifest.platforms[target.key]?.url, `${target.key} 缺少 url`);
    assert(manifest.platforms[target.key]?.signature, `${target.key} 缺少 signature`);
  }
}

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

  state.recordManifest('onlineVerify', { results });
  if (results.some((result) => result.status === 'failed')) {
    state.failStep('verify', '线上验证失败', '请检查失败的 updater endpoint 或下载 URL');
    fail('线上验证失败');
  }

  state.completeStep('verify', '线上验证通过', { endpoints: results.length });
}

function fetchJson(url) {
  return JSON.parse(exec('curl', ['-L', ...curlProxy(url), '--connect-timeout', '10', '--max-time', '30', '-sS', url]));
}

function httpStatus(url) {
  return Number(exec('curl', ['-L', ...curlProxy(url), '-o', '/dev/null', '-sS', '-w', '%{http_code}', '--connect-timeout', '10', '--max-time', '30', url]));
}

function curlProxy(url) {
  return config.githubProxy && url.includes('github.com') ? ['--proxy', config.githubProxy] : [];
}

function listFiles(dir) {
  if (!existsSync(dir)) return [];
  const files = [];
  for (const entry of readdirSyncSafe(dir)) {
    const file = path.join(dir, entry);
    const info = statSync(file);
    if (info.isDirectory()) files.push(...listFiles(file));
    if (info.isFile()) files.push(file);
  }
  return files;
}

function readdirSyncSafe(dir) {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
}

function hasCommand(command) {
  return spawnSync('which', [command], { stdio: 'ignore' }).status === 0;
}

function hasGiteeCredential() {
  const result = spawnSync('git', ['credential', 'fill'], {
    input: 'protocol=https\nhost=gitee.com\n\n',
    encoding: 'utf8',
  });
  return result.status === 0 && result.stdout.includes('password=');
}

function run(command, commandArgs, cwd) {
  const result = spawnSync(command, commandArgs, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (result.status !== 0) {
    throw new Error(`命令执行失败：${command} ${commandArgs.join(' ')}\n${result.stdout}${result.stderr}`);
  }
}

function exec(command, commandArgs, cwd = process.cwd()) {
  return execFileSync(command, commandArgs, { cwd, encoding: 'utf8' }).trim();
}

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

function json(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}

function sha256(file) {
  return createHash('sha256').update(readFileSync(file)).digest('hex');
}

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const item = values[index];
    if (!item.startsWith('--')) continue;
    const key = item.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    const next = values[index + 1];
    parsed[key] = next && !next.startsWith('--') ? (index += 1, next) : true;
  }
  return parsed;
}

function timestamp() {
  return new Date().toISOString().replace(/[-:]/g, '').replace(/\..+/, '').replace('T', '-');
}

function isVersion(value) {
  return /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(value || '');
}

function progress(message) {
  console.log(`进度：${message}`);
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function fail(message) {
  console.error(`错误：${message}`);
  process.exit(1);
}
