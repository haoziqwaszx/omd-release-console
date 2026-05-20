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
