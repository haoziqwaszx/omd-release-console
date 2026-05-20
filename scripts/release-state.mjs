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
