# Apple Tauri Release Wizard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the current release console UI into an Apple-style Tauri desktop release wizard that guides users from packaging to publishing with one primary next action.

**Architecture:** Keep the existing Rust action API intact. Replace the static HTML structure with a native-feeling shell, update CSS to a macOS utility style, and update `public/app.js` so low-level Rust steps map into five user-facing release stages.

**Tech Stack:** Rust 2021, Tauri v2 command bridge, vanilla HTML/CSS/JavaScript, Browser visual smoke checks.

---

## File Structure

- Modify `public/index.html`: replace the current centered assistant/card layout with a desktop app shell: toolbar, sidebar, main focus panel, details panel, and compact history.
- Modify `public/styles.css`: replace the current green dashboard styling with Apple-style light neutral UI, sidebar navigation, blue primary actions, compact controls, responsive desktop/narrow layouts.
- Modify `public/app.js`: add five-stage journey mapping, render sidebar stages, update main focus copy, and keep details/log/publish controls secondary.
- Keep `src/main.rs` unchanged for this UI pass unless verification exposes a real integration issue.
- Update `README.md` only if the user-facing run instructions need wording changes.

## Task 1: Lock Frontend Syntax Baseline

- [ ] **Step 1: Run current frontend syntax check**

```bash
node --check public/app.js
```

Expected: pass before UI edits.

## Task 2: Replace The HTML Shell

- [ ] **Step 1: Modify `public/index.html`**

Replace the visible body with:

```html
<body>
  <main class="release-app-shell">
    <header class="app-toolbar" aria-label="发布上下文">
      <div class="traffic-lights" aria-hidden="true">
        <span></span><span></span><span></span>
      </div>
      <div class="toolbar-title">
        <p class="eyebrow">OMD Release</p>
        <h1 id="pageTitle">发布向导</h1>
      </div>
      <div class="toolbar-status">
        <span class="status-label">整体状态</span>
        <strong id="overallStatusText">未开始</strong>
        <span id="overallStatusBadge" class="status-pill neutral">未开始</span>
      </div>
    </header>

    <section class="release-context-panel" aria-label="发布信息">
      <label class="context-field" for="versionInput">
        <span>目标版本</span>
        <input id="versionInput" value="0.0.7" placeholder="0.0.7" />
      </label>
      <label class="context-field release-dir-field" for="releaseDirInput">
        <span>Release 目录</span>
        <input id="releaseDirInput" placeholder="默认读取最新 release 目录" />
      </label>
      <button id="loadButton" class="tertiary-button" type="button">读取</button>
      <button id="refreshButton" class="tertiary-button" type="button">刷新</button>
      <button id="latestDirButton" class="tertiary-button desktop-only" type="button">最近目录</button>
      <button id="openDirButton" class="tertiary-button desktop-only" type="button">打开目录</button>
      <p id="releaseDirText" class="release-path">正在读取...</p>
    </section>

    <section class="release-workspace" aria-label="发布向导">
      <aside class="release-sidebar" aria-label="发布旅程">
        <div class="sidebar-heading">
          <span class="eyebrow">发布旅程</span>
          <strong>从打包到发布</strong>
        </div>
        <nav id="progressRail" class="journey-list" aria-label="发布进度"></nav>
      </aside>

      <section class="step-workbench" aria-label="当前发布步骤">
        <article id="currentStepCard" class="focus-panel">
          <div class="focus-meta">
            <span id="currentStepEyebrow" class="eyebrow">当前步骤</span>
            <span id="currentStepStatus" class="status-pill neutral">未开始</span>
          </div>
          <div id="currentStepNumber" class="step-number">1</div>
          <h2 id="currentStepTitle">准备发布信息</h2>
          <p id="currentStepDescription" class="step-description">确认目标版本和 release 目录后开始发布准备。</p>
          <div id="focusSuggestions" class="focus-suggestions"></div>
          <button id="primaryActionButton" class="primary-button" type="button">确认发布信息</button>
        </article>

        <details id="detailDrawer" class="detail-drawer" open>
          <summary>
            <span>发布详情</span>
            <strong id="detailsSummaryText">当前步骤详情</strong>
          </summary>
          <div id="stepDetailContent" class="detail-content"></div>
          <button id="toggleLogsButton" class="ghost-button" type="button">显示日志</button>
          <pre id="actionLogOutput" class="action-log hidden">暂无动作日志</pre>
        </details>
      </section>

      <aside class="release-inspector" aria-label="辅助信息">
        <section class="inspector-section">
          <h3>发布确认</h3>
          <input id="confirmVersionInput" placeholder="输入版本号后执行发布" />
          <div class="button-pair">
            <button id="publishGithubButton" class="secondary-button" type="button">发布 GitHub</button>
            <button id="publishGiteeButton" class="secondary-button" type="button">发布 Gitee</button>
          </div>
        </section>
        <section class="inspector-section">
          <h3>配置</h3>
          <div id="configSummary" class="compact-list"></div>
        </section>
        <section class="inspector-section">
          <h3>Manifest</h3>
          <div id="manifestList" class="compact-list"></div>
        </section>
        <section class="inspector-section">
          <h3>产物</h3>
          <div id="artifactList" class="compact-list"></div>
        </section>
        <section class="inspector-section">
          <h3>历史</h3>
          <div id="historyList" class="compact-list"></div>
        </section>
        <section class="inspector-section command-section">
          <h3>调试命令</h3>
          <div id="commandOutput" class="compact-list"></div>
        </section>
      </aside>
    </section>
  </main>
  <script type="module" src="/app.js"></script>
</body>
```

- [ ] **Step 2: Run syntax check**

```bash
node --check public/app.js
```

Expected: pass, because HTML change should not break JS ids.

## Task 3: Replace Visual Styling

- [ ] **Step 1: Modify `public/styles.css`**

Implement Apple-style layout rules for:

- `.release-app-shell`
- `.app-toolbar`
- `.release-context-panel`
- `.release-workspace`
- `.release-sidebar`
- `.journey-list`
- `.journey-item`
- `.step-workbench`
- `.focus-panel`
- `.release-inspector`
- `.detail-drawer`

Use blue primary color `#0071e3`, neutral background `#f5f5f7`, and restrained borders.

- [ ] **Step 2: Run frontend syntax check**

```bash
node --check public/app.js
```

Expected: pass.

## Task 4: Map Steps Into Five Release Stages

- [ ] **Step 1: Modify `public/app.js` progress model**

Replace the flat ten-step rail with five stage objects:

```js
const journeyStages = [
  { key: 'prepare', label: '准备发版', description: '确认版本、源码和发布目录。', steps: ['preflight'] },
  { key: 'build', label: '打包双端', description: '打包 Windows 和 Mac 应用。', steps: ['buildWindows', 'buildMac'] },
  { key: 'collect', label: '收集检查', description: '收集产物并检查签名。', steps: ['collectWindowsArtifacts', 'artifactCheck', 'manifest'] },
  { key: 'publish', label: '发布分发', description: '发布 GitHub 和 Gitee。', steps: ['publishGithub', 'publishGitee'] },
  { key: 'verify', label: '验证报告', description: '验证线上更新并生成报告。', steps: ['verify', 'report'] },
];
```

- [ ] **Step 2: Update `mapProgressItems`**

Return five `.journey-item` entries with status derived from contained low-level steps.

- [ ] **Step 3: Update current step copy**

Use user-facing labels:

- `准备发版`
- `打包 Windows`
- `收集 Windows 产物`
- `打包 Mac`
- `检查产物`
- `生成 Manifest`
- `发布 GitHub`
- `发布 Gitee`
- `验证线上更新`
- `生成发布报告`

- [ ] **Step 4: Run frontend syntax check**

```bash
node --check public/app.js
```

Expected: pass.

## Task 5: Verify UI Behavior

- [ ] **Step 1: Run full check**

```bash
npm run check
```

Expected: Rust tests, clippy, and JS syntax pass.

- [ ] **Step 2: Start local server**

```bash
cargo run -- serve
```

Expected: server prints `http://127.0.0.1:4177`.

- [ ] **Step 3: Browser smoke test**

Open `http://127.0.0.1:4177` and verify:

- five sidebar stages render
- one primary action is visible
- publish confirmation input and buttons render in inspector
- `Phase 2` copy is absent
- body uses Apple-style neutral/blue styling

## Self-Review

- Spec coverage: covered product shell, Apple visual direction, journey sidebar, one primary action, secondary details, and safety boundaries.
- Placeholder scan: no placeholder-only steps.
- Type consistency: plan uses existing DOM ids plus new container classes; JS action names match existing Rust actions.
