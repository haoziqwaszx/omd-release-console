# Phase 1.5 产品化发布向导设计

## 背景

Phase 1 已经把 OMD Release Console 从只读状态页升级为可触发低风险 actions 的发布准备中心，包括配置摘要、action API、结构化 `state.json`、产物检查、日志和建议展示。

当前问题是页面仍然像工程调试面板：配置、动作、日志、manifest、产物和命令平铺展示，用户注意力被分散。发版用户需要的是一个统一、清晰、美观的产品化流程：页面主动告诉用户当前在哪一步、为什么卡住、下一步点什么。

Phase 1.5 的目标是重构前端信息架构和视觉表达，把页面改成一个单屏 Stepper 发布向导。此次重点是产品体验，不新增真实发布能力。

## 目标

- 页面只强调一个当前步骤和一个主行动。
- 所有步骤引导集中在一个主工作区，不让用户在多个面板之间找答案。
- 辅助信息只服务当前步骤，默认不抢占主流程注意力。
- 视觉系统统一、专业、美观，符合发布工具的安全感和可靠感。
- 保持现有 Phase 1 API 和 `state.json` 数据结构，不做后端大改。

## 非目标

- 不启用真实 GitHub/Gitee 发布。
- 不引入 React/Vue 等前端框架。
- 不新增复杂多页面路由。
- 不把所有底层日志默认展开。
- 不做 release history 或 run archive。

## 产品结构

页面从“多个面板平铺”改成“单屏发布向导”。从上到下由四个核心区域组成。

### 1. Release Header

顶部区域负责确认发布上下文，只展示必要信息：

- 产品标题：`OMD Release Console`
- 副标题：`安全可控的 OMD 双端发布向导`
- 目标版本输入
- release 目录输入
- 整体状态 badge：`未开始`、`有阻塞`、`可继续`、`已完成`
- 次要按钮：`刷新`、`读取目录`

Header 不展示完整配置摘要，不展示 manifest URL，不展示日志。

### 2. Release Stepper

Stepper 展示完整发布准备路径，但只承担导航和状态表达，不承载详情。

步骤：

1. 预检
2. 产物检查
3. Manifest
4. 线上验证
5. Dry-run
6. 发布，Phase 2 启用

每个 step 只显示：编号、名称、状态点和简短状态文案。

统一状态文案：

- `未开始`
- `正在执行`
- `已完成`
- `有阻塞`
- `后续阶段`

### 3. Current Step Focus Card

这是页面视觉中心。用户进入页面后应优先看这里。

主卡片展示：

- 当前步骤标题，例如 `当前步骤：预检`
- 当前步骤说明，例如 `确认源码版本、凭据、SSH 和 updater 端点是否满足发版条件`
- 当前状态 badge
- 主 CTA，例如 `运行预检`
- 最多 3 条阻塞建议
- 一句下一步解释，例如 `修复阻塞项后重新运行预检`

规则：

- 页面同一时间只能有一个 primary CTA。
- 当前步骤失败时，主 CTA 变成重试当前步骤。
- 当前步骤成功后，自动推进到下一步。
- 高风险真实发布步骤显示为禁用，不成为主 CTA。

### 4. Current Step Details

详情区位于主卡片下方，只展示与当前步骤有关的信息。

建议使用 tabs 或紧凑分段：

- `检查结果`
- `相关产物`
- `Manifest 摘要`
- `日志`

显示规则：

- 预检步骤默认展示 checks。
- 产物检查步骤默认展示 required artifacts。
- Manifest 步骤默认展示 GitHub/Gitee manifest 摘要和差异。
- 线上验证步骤默认展示 endpoint 验证结果。
- Dry-run 步骤默认展示命令卡片。
- 日志默认折叠或放在最后，不抢主流程。

低频信息，例如完整配置摘要、完整产物列表、原始 stdout/stderr，放在底部详情区或折叠区域。

## 用户引导逻辑

前端根据现有 state 计算 `currentStep` 和 `primaryAction`。

### Step 选择规则

1. 如果没有版本号或 release 目录：
   - 当前步骤：`准备发布信息`
   - 主 CTA：`确认发布信息`

2. 如果 `preflight` 未成功：
   - 当前步骤：`预检`
   - 主 CTA：`运行预检`

3. 如果 `preflight` 失败：
   - 当前步骤：`预检`
   - 主 CTA：`重新运行预检`
   - 主卡片显示最多 3 条阻塞建议

4. 如果预检成功但产物检查或 manifest 未成功：
   - 当前步骤：`产物与 Manifest`
   - 主 CTA：`生成 Manifest`

5. 如果 manifest 成功但 verify 未成功：
   - 当前步骤：`线上验证`
   - 主 CTA：`验证端点`

6. 如果 verify 成功但 dry-run 未成功：
   - 当前步骤：`Dry-run`
   - 主 CTA：`生成 dry-run 命令`

7. 如果 dry-run 成功：
   - 当前步骤：`准备完成`
   - 主 CTA：`复制发布命令`
   - 发布步骤显示 `Phase 2 启用`

### 当前步骤必须回答的问题

主工作区只回答三个问题：

1. 我现在在哪一步？
2. 为什么卡在这里？
3. 修完后点哪个按钮？

## 视觉系统

风格定位：专业发布工具，强调安全、秩序和可靠性。

### 颜色

建议浅灰背景 + 白色主卡片：

- 页面背景：浅灰
- 主卡片：白色
- 标题和结构色：深 slate
- 安全动作：绿色
- 注意/等待：Amber
- 阻塞：红色
- 未开始/禁用：灰色

### 按钮层级

- 页面只能出现一个 primary CTA。
- Primary CTA 使用绿色，表示安全可执行。
- 次要动作使用 outline/ghost。
- 禁用发布动作使用灰色，不允许看起来像可点击主按钮。

### 信息密度

- 主卡片只展示当前步骤必要信息。
- Stepper 只展示流程进度。
- 日志默认折叠。
- 配置摘要不在首页平铺。
- Manifest URL、文件名、checksums 等长文本只在详情区出现。

### 交互反馈

- action 执行中，主按钮显示 loading 状态并禁用。
- action 成功后，自动推进到下一步。
- action 失败后，主卡片显示阻塞原因和建议操作。
- 所有可点击元素有明确 hover/focus 状态。
- 移动端不产生横向滚动。

## 组件划分

仍使用现有 vanilla HTML/CSS/JS，但按产品组件组织。

### ReleaseHeader

职责：输入/展示发布上下文。

数据来源：

- `versionInput`
- `releaseDirInput`
- `/api/summary`
- `/api/config` 的少量摘要

### ReleaseStepper

职责：展示 6 个步骤的状态。

依赖：

- `state.steps`
- 前端计算出的 step view model

### CurrentStepCard

职责：展示当前步骤、主 CTA、状态和阻塞建议。

依赖：

- 前端计算出的 `currentStep`
- `state.suggestions`
- action loading state

### StepDetailPanel

职责：展示当前步骤相关详情。

依赖：

- `state.checks`
- `state.artifacts`
- `summary.requiredArtifacts`
- `summary.manifests`
- `state.manifests`
- 最近 action logs

### SecondaryDetails

职责：展示低频信息。

内容：

- 配置摘要
- 完整产物列表
- 原始日志
- 命令卡片

默认折叠或弱化显示。

## 数据与 API

Phase 1.5 不要求新增 API。前端继续使用：

- `GET /api/summary`
- `GET /api/config`
- `POST /api/actions/preflight`
- `POST /api/actions/manifest`
- `POST /api/actions/verify`
- `POST /api/actions/dry-run-release`

前端新增 view model 计算层：

- `buildReleaseViewModel(summary, config, lastAction)`
- `resolveCurrentStep(state, summary)`
- `resolvePrimaryAction(currentStep)`
- `mapStepStatus(state.steps)`
- `topSuggestions(state.suggestions, max = 3)`

这些函数只负责展示逻辑，不改变后端状态。

## 验收标准

- 页面首屏只突出一个当前步骤卡片和一个主 CTA。
- 用户不需要查看多个面板就能知道下一步该做什么。
- Stepper 展示完整流程，但不抢主卡片注意力。
- 配置、日志、manifest 长 URL、完整产物列表不再首页平铺。
- 失败状态下，主卡片展示最多 3 条行动建议。
- 成功状态下，当前步骤自动推进到下一步。
- 真实发布步骤继续禁用，并标记 `Phase 2 启用`。
- `npm run check` 通过。
- 浏览器手动验证：
  - 首屏视觉集中。
  - 运行预检后，失败建议出现在主卡片。
  - required artifacts 出现在当前步骤详情，而不是抢占主流程。
  - 命令卡片只在 Dry-run 或详情区出现。
  - 375px、768px、1024px、1440px 下无横向滚动。

## 自检

- 本设计聚焦单一 UI/UX 重构，不包含真实发布、历史 run 或后端新能力。
- 当前步骤、主 CTA、详情区、低频信息的边界明确。
- 视觉规则与产品目标一致：集中注意力、统一、美观、安全。
- 没有未定义的 Phase 1 后端依赖。
