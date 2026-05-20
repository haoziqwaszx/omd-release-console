# Phase 1 发布准备中心设计

## 背景

OMD Release Console 当前是一个本地发布控制台，已经可以展示最近 release 目录、GitHub/Gitee manifest、Windows/Mac 产物、状态节点和下一版命令。CLI 已具备 `plan`、`preflight`、`manifest`、`verify`、`release --dry-run` 等能力。

当前限制是：前端只读，不触发发布相关动作；CLI 主要输出人类可读日志，没有统一结构化状态；失败后页面不能明确展示失败步骤、原因和下一步建议。

Phase 1 的目标是把控制台升级为低风险的“发布准备中心”，让用户能在页面完成发布前检查、manifest 生成、线上验证和 dry-run release，同时保持真实上传关闭。

## 目标与边界

Phase 1 要实现或规划以下能力：

- 页面输入目标版本和 release 目录。
- 页面触发低风险 actions：`preflight`、`manifest`、`verify`、`dry-run-release`。
- CLI 将每个 action 的结果写入标准化 `state.json`。
- 页面展示步骤状态、失败原因、日志摘要、缺失产物、manifest 差异和下一步建议。
- 新增 `release.config.json` 保存非敏感配置。
- 定义 `publish-github` / `publish-gitee` 的接口契约和安全要求，但 Phase 1 不执行真实上传。

Phase 1 不做：

- 不从页面执行真实 GitHub/Gitee 发布。
- 不做一键完整发布。
- 不做 release run 历史页面。
- 不做失败恢复编排，只为后续恢复执行保留状态结构。

## 架构

保留现有三层结构，并明确职责。

### CLI 层

CLI 继续承载发布知识和实际检查逻辑。`preflight`、`manifest`、`verify`、`release --dry-run` 既打印人类可读日志，也写入机器可读 `state.json`。CLI 是发布状态的事实来源。

### 后端层

后端只提供白名单 action API，不接收任意命令字符串。后端负责：

- 校验 `version`。
- 限制 `releaseDir` 范围。
- 调用固定 CLI 命令。
- 收集 stdout/stderr。
- 读取最新 `state.json`。
- 返回统一 action 结果。

### 前端层

前端负责输入、触发和展示。前端不重复实现复杂发布规则，优先展示 `state.json` 中的 `steps`、`checks`、`artifacts`、`manifests` 和 `suggestions`。

## 配置设计

新增 `release.config.json`，保存非敏感配置。建议字段：

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
    { "key": "windows-x86_64-nsis", "scope": "windows", "file": "OMD_{version}_x64-setup.exe", "signature": "OMD_{version}_x64-setup.exe.sig" },
    { "key": "windows-x86_64-msi", "scope": "windows", "file": "OMD_{version}_x64_en-US.msi", "signature": "OMD_{version}_x64_en-US.msi.sig" },
    { "key": "windows-x86_64-green", "scope": "windows", "file": "OMD_{version}_x64_green.zip", "signature": "OMD_{version}_x64_green.zip.sig" },
    { "key": "darwin-aarch64", "scope": "mac", "file": "OMD.app.tar.gz", "signature": "OMD.app.tar.gz.sig" }
  ]
}
```

敏感信息不写入配置文件。系统只检测凭据是否存在，例如 `gh auth status`、`git credential fill`、SSH BatchMode 检查。

`GET /api/config` 只返回配置摘要，不返回 token、密码、私钥等敏感内容。

## 状态文件设计

`state.json` 是页面展示、失败诊断和后续恢复执行的核心数据源。

建议顶层结构：

```json
{
  "version": "0.0.7",
  "releaseDir": "/Users/glame/Desktop/omd-0.0.7-release-20260520-170000",
  "sourceRepo": "/Users/glame/Desktop/dix-extension-ui",
  "commit": "abc123",
  "createdAt": "2026-05-20T09:00:00.000Z",
  "updatedAt": "2026-05-20T09:05:00.000Z",
  "currentStep": "preflight",
  "overallStatus": "running",
  "checks": [],
  "steps": {},
  "artifacts": [],
  "manifests": {},
  "errors": [],
  "suggestions": []
}
```

### Step 状态

`steps` 使用固定 key：

- `preflight`
- `artifactCheck`
- `checksums`
- `manifestGithub`
- `manifestGitee`
- `dryRunRelease`
- `verify`
- `publishGithub`，Phase 1 预留
- `publishGitee`，Phase 1 预留

每步状态为：

- `not_started`
- `running`
- `success`
- `failed`
- `skipped`
- `manual_required`
- `disabled`

每个 step 记录：

```json
{
  "status": "success",
  "startedAt": "2026-05-20T09:00:00.000Z",
  "endedAt": "2026-05-20T09:01:00.000Z",
  "durationMs": 60000,
  "message": "预检完成",
  "summary": {},
  "error": null,
  "suggestions": []
}
```

### Checks

`checks` 记录预检项，例如源码版本、Tauri 版本、updater key、gh CLI、GitHub 登录、SSH、Gitee 凭据、updater endpoints。

每项记录：

```json
{
  "key": "githubAuth",
  "label": "GitHub 登录状态",
  "status": "success",
  "message": "gh auth status 通过",
  "checkedAt": "2026-05-20T09:00:00.000Z",
  "suggestion": ""
}
```

### Artifacts

`artifacts` 同时记录必须产物和实际产物。每项包含平台、文件名、是否存在、大小、修改时间、checksum 和签名状态。

### Manifests

`manifests` 记录 GitHub/Gitee manifest 的版本、发布时间、平台目标、URL、签名状态和一致性检查结果。

### Suggestions

`suggestions` 给前端提供下一步建议，避免前端重复实现复杂判断。建议包含 `severity`、`message`、`action` 可选字段。

## 后端 API

保留现有：

- `GET /api/summary`
- `GET /api/commands`

新增：

- `GET /api/config`
- `POST /api/actions/preflight`
- `POST /api/actions/manifest`
- `POST /api/actions/verify`
- `POST /api/actions/dry-run-release`

请求体统一：

```json
{
  "version": "0.0.7",
  "releaseDir": "/Users/glame/Desktop/omd-0.0.7-release-20260520-170000"
}
```

响应体统一：

```json
{
  "ok": true,
  "action": "preflight",
  "status": "success",
  "message": "预检完成",
  "state": {},
  "logs": {
    "stdout": "...",
    "stderr": "..."
  },
  "suggestions": []
}
```

Action 到 CLI 的映射固定：

- `preflight` → `node scripts/omd-release.mjs preflight <version> --release-dir <dir>`
- `manifest` → `node scripts/omd-release.mjs manifest <version> --release-dir <dir>`
- `verify` → `node scripts/omd-release.mjs verify <version> --release-dir <dir>`
- `dry-run-release` → `node scripts/omd-release.mjs release <version> --release-dir <dir> --dry-run`

## 真实发布接口预留

Phase 1 定义但不启用真实发布接口：

- `POST /api/actions/publish-github`
- `POST /api/actions/publish-gitee`

未来请求体必须包含：

```json
{
  "version": "0.0.7",
  "releaseDir": "/Users/glame/Desktop/omd-0.0.7-release-20260520-170000",
  "confirmVersion": "0.0.7"
}
```

安全要求：

- `confirmVersion` 必须等于 `version`。
- 线上已有相同版本时必须标记重复发布风险。
- GitHub 和 Gitee 发布状态必须独立记录。
- 一个平台成功、另一个平台失败时，不能显示整体成功。
- 失败时保留日志和状态，不自动清理 release 目录。

Phase 1 中如果这些接口存在，必须返回禁用状态，例如：

```json
{
  "ok": false,
  "action": "publish-github",
  "status": "disabled",
  "message": "真实发布在 Phase 1 未启用"
}
```

## CLI 改造

新增共享状态写入模块，提供：

- `loadState(releaseDir)`
- `saveState(releaseDir, state)`
- `startStep(stepKey, metadata)`
- `completeStep(stepKey, summary)`
- `failStep(stepKey, error, suggestion)`
- `recordCheck(checkKey, status, message, suggestion?)`
- `recordArtifact(artifact)`
- `recordManifest(host, summary)`
- `recordSuggestion(message, severity?)`

行为调整：

- `preflight` 尽量执行所有检查，最后汇总失败；不要第一个失败就终止全部检查。
- `manifest` 先执行产物完整性检查，再生成 `checksums.sha256` 和 GitHub/Gitee `latest.json`。
- `manifest` 缺少签名文件时写入 `artifacts` 和 `suggestions`。
- `verify` 记录每个 updater endpoint 和每个平台 URL 的 HTTP 状态、版本匹配状态、签名状态。
- `release --dry-run` 写入 `dryRunRelease` 步骤和命令列表，但不执行真实上传。
- `release` 无 `--dry-run` 时继续阻断真实执行，并写入 `manual_required` 或失败状态。

CLI 必须继续支持独立终端使用，页面 action 只是调用固定 CLI 能力。

## 前端设计

前端升级为发布准备中心，不需要改成复杂 SPA。

### 发布输入区

展示和输入：

- 目标版本号。
- release 目录。
- 配置摘要：sourceRepo、GitHub/Gitee repo、updater endpoint。

### 步骤卡片区

显示步骤：

- `preflight`
- `manifest`
- `verify`
- `dry-run release`
- `publish GitHub`，禁用，标记后续阶段启用
- `publish Gitee`，禁用，标记后续阶段启用

每张卡展示状态、最近执行时间、耗时、摘要、失败原因、建议。低风险步骤提供“运行”按钮；高风险步骤只显示说明或禁用状态。

### 产物完整性区

页面不只显示实际文件，还显示 requiredArtifacts。每个产物展示平台、文件名、存在/缺失、大小、修改时间和签名状态。

### Manifest 对比区

展示 GitHub/Gitee 的版本、pub_date、平台 targets、URL、签名状态，并标记两边是否一致、是否与目标版本一致。

### 日志与建议区

展示最近 action 的 stdout/stderr 摘要和 `state.suggestions`。错误提示应面向行动，例如缺少 gh CLI、Gitee 凭据不可用、updater endpoint 缺少 latest.json。

### 命令卡片

现有命令区从大文本框升级为命令卡片：命令名、用途、风险等级、复制按钮。`dry-run` 可运行；真实发布命令只展示说明，不作为 Phase 1 按钮。

## 安全控制

- 所有版本号必须符合 SemVer。
- 所有 releaseDir 必须限制在允许目录范围内。
- 后端不接收任意命令字符串。
- 后端 action 只能映射到固定 CLI 命令。
- 真实发布前必须要求版本号确认。
- Phase 1 不执行真实发布。
- 线上已有相同版本时必须提示重复发布风险。
- GitHub 和 Gitee 发布结果独立记录。
- 失败时保留日志和状态，不自动清理 release 目录。

## 测试与验收标准

- `npm run check` 通过。
- `GET /api/summary` 仍兼容现有页面。
- `GET /api/config` 返回非敏感配置摘要。
- 传入非法版本号时，action API 返回结构化错误，不执行 CLI。
- 传入允许范围外的 releaseDir 时，action API 返回结构化错误，不执行 CLI。
- `preflight` action 能调用 CLI，并把检查结果写入 `state.json`。
- `manifest` action 能生成或更新 checksums 和 manifest，并记录缺失产物或签名。
- `verify` action 能记录 endpoint 和 URL 验证结果。
- `dry-run-release` action 能返回命令列表并写入状态，但不执行真实发布。
- 前端能显示每个步骤状态、失败原因、日志摘要和下一步建议。
- `publish-github` / `publish-gitee` 在 Phase 1 不可真实执行；若接口存在，也必须返回禁用状态或要求未满足的确认信息。
- 不存在从前端传任意 shell 命令给后端执行的路径。
- UI 手动验证：启动 `npm run dev`，在浏览器完成版本输入、目录加载、低风险 action 触发、失败提示查看和 summary 刷新。
