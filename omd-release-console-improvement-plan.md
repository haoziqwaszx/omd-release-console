# OMD Release Console 完善方案

## 1. 背景

OMD Release Console 当前是一个独立的本地发布控制台，用于辅助 OMD 双端发布。它不属于 `dix-extension-ui` 产品仓库，而是围绕发布流程单独建设的工具项目。

现有系统已经具备基础的发布查看与命令辅助能力：前端可以展示发布流程、最近的 release 目录、GitHub/Gitee manifest 状态、Windows/Mac 产物清单、状态节点和下一版常用命令；后端可以扫描桌面最新的 `omd-*-release-*` 目录并汇总 manifest、checksums、state 和产物；CLI 已具备 `plan`、`preflight`、`manifest`、`verify`、`release --dry-run` 等命令。

当前版本的核心限制是：前端只负责展示和检查，不直接执行发布；CLI 的真实 `release` 执行也被安全地阻断，需要用户先通过 `--dry-run` 核对命令。这一设计降低了误触发真实上传的风险，但也让系统距离“可控发布驾驶舱”还有一步距离。

本方案的目标，是在保持安全边界的前提下，把 OMD Release Console 从“发布状态查看器”逐步升级为“安全可控的发布驾驶舱”。

## 2. 产品目标

完善后的系统应该服务于负责 OMD 发版的人，帮助其在一个页面内完成发布前检查、发布过程推进、产物确认、manifest 生成、线上端点验证和发布报告沉淀。

系统不应一开始追求完全一键发布，而应先实现“分段执行、明确确认、全程留痕、失败可恢复”。真实发布动作涉及 GitHub/Gitee 上传、updater manifest、远程 Windows 构建、签名文件和下载 URL 验证，一旦状态不可追踪或误触发，风险较高。因此系统需要先建立可靠的执行状态、日志、确认机制和失败处理机制，再逐步开放真实发布能力。

## 3. 发布职责边界

OMD 双端发布应继续保持当前职责划分。

Windows 4090 构建机只负责 Windows 构建和签名，包括 NSIS 安装包、MSI 安装包、绿色 zip 包以及对应签名文件。

Mac 主控机负责统一收集产物、本机构建 Mac 产物、生成 checksums、生成 GitHub/Gitee 的 `latest.json`、发布 GitHub/Gitee Release，并最终验证 updater 端点和每个下载 URL。

Release Console 的职责不是替代这些机器和平台，而是把这些步骤串成一个可观察、可确认、可恢复的发布流程。

## 4. 当前能力与主要缺口

当前系统已有的能力包括：本地 Web 控制台、发布流程展示、最近 release 目录自动发现、manifest 摘要展示、产物列表展示、状态节点展示、下一版命令生成，以及 CLI 层面的预检、manifest 生成、checksums 生成、端点验证和 dry-run 发布命令输出。

主要缺口集中在发布执行和易用性两方面。

在发布执行方面，页面还不能触发任何发布动作；CLI 执行结果没有统一结构化写入 `state.json`；不同步骤之间缺少明确的状态机；失败后用户无法在页面上看到“失败在哪一步、为什么失败、下一步应该怎么做”；真实发布动作缺少逐步确认、重复发布检测、恢复执行和完整日志留痕。

在易用性方面，页面目前主要展示“已有信息”，但没有充分展示“缺什么、下一步做什么、哪里有风险”。产物区只列出现有文件，缺少应有产物清单和缺失提示；manifest 区展示了目标平台和签名状态，但缺少 GitHub/Gitee 差异、版本一致性和端点状态；命令区只是输出文本命令，没有按步骤解释命令用途、风险级别和复制操作；配置项写在脚本中，不利于长期维护。

## 5. 总体改造原则

第一，安全优先。默认只做 dry-run 和检查，所有真实发布动作都必须经过显式确认。危险动作不能只靠一个按钮触发，至少要要求用户输入目标版本号确认。

第二，分段执行。不要设计一个大的“发布”按钮，而是将发布拆成预检、创建发布目录、收集产物、生成 checksums、生成 manifest、发布 GitHub、发布 Gitee、验证端点等独立步骤。每一步可以单独执行、单独失败、单独重试。

第三，状态留痕。每个步骤的开始时间、结束时间、结果、错误信息、关键输出和下一步建议都要写入 release 目录下的状态文件。页面展示不应依赖瞬时命令输出，而应以结构化状态为准。

第四，参数白名单。后端不应从前端接收任意命令并执行，只应开放固定 action，例如 `preflight`、`manifest`、`verify`、`dryRunRelease`。参数只允许 version、releaseDir 等必要字段，并做 SemVer 校验、路径限制和危险字符过滤。

第五，先辅助决策，再自动执行。优先让系统告诉用户“当前状态是否安全、缺少什么、下一步应该做什么”，再逐步把低风险步骤自动化。真实上传和远程构建应放在后续阶段接入。

## 6. 阶段一：发布准备中心

阶段一目标是让发版前的所有检查都能在页面完成，但仍不执行真实上传。这个阶段应作为最小可落地版本。

页面增加版本号输入、目标 release 目录输入、源仓库路径展示、Windows 构建机状态、GitHub/Gitee 凭据状态、updater 端点状态、产物要求清单和检查结果区域。

后端增加安全 action API，例如 `POST /api/actions/preflight`、`POST /api/actions/manifest`、`POST /api/actions/verify` 和 `POST /api/actions/dry-run-release`。这些 API 不接受任意 shell 命令，只调用固定 CLI 能力。所有 action 执行前检查 version 是否符合 SemVer，releaseDir 是否在允许目录范围内。

CLI 需要把检查结果结构化写入 `state.json`。例如预检不应只在终端输出“进度：预检通过”，还应记录每个检查项：源码版本、Tauri 版本、updater.key、gh CLI、GitHub 登录状态、SSH 连通性、Gitee 凭据、GitHub updater 端点、Gitee updater 端点。每个检查项应有 `status`、`message`、`checkedAt` 和可选的 `suggestion`。

前端展示上，应把“下一版命令”升级为命令卡片。每张卡片包含命令名称、命令内容、用途说明、风险等级和复制按钮。对于 dry-run 和 verify 等低风险命令，可以提供“运行”按钮；对于真实发布命令，只显示命令和说明，不直接执行。

阶段一完成后，系统应能回答三个关键问题：当前能不能发版，不能的话卡在哪里；当前 release 目录里的产物和 manifest 是否完整；下一步应该执行什么。

## 7. 阶段二：分段执行发布

阶段二目标是让页面可以触发部分真实发布步骤，但必须分段确认。

建议将发布流程拆成以下步骤：创建 release 目录、运行预检、检查本地产物、生成 checksums、生成 GitHub manifest、生成 Gitee manifest、验证本地 manifest、dry-run 发布命令、发布 GitHub Release、发布 Gitee Release、验证线上 updater 端点、生成发布报告。

每个步骤都应有明确状态：未开始、可执行、执行中、成功、失败、已跳过、需要人工处理。页面中每一步都应显示最近执行时间、耗时、摘要输出和失败原因。

真实发布 GitHub/Gitee 前必须加入强确认。建议使用版本号确认机制：如果目标版本是 `0.0.7`，用户必须在确认框中输入 `0.0.7` 才能继续。同时页面应展示即将发布的仓库、release tag、产物数量、manifest 版本和线上当前版本，避免用户发布到错误仓库或重复发布。

失败处理应尽量可恢复。例如 GitHub 发布成功但 Gitee 发布失败时，系统不能简单显示“发布失败”，而应标记 GitHub 已完成、Gitee 失败，并建议用户重试 Gitee 发布或手动处理后继续 verify。CLI 可增加 `--step` 或 `--resume-from` 参数，为后续恢复执行打基础。

## 8. 阶段三：完整发布编排

阶段三目标是引入 release run 概念，把一次发布从开始到结束完整编排和归档。

每次发布生成一个唯一 runId，记录版本号、commit、release 目录、开始时间、结束时间、执行人、目标仓库、每个步骤状态、关键输出摘要和失败位置。页面从单纯的状态看板升级为发布向导，按“确认版本与 commit、环境预检、产物检查、manifest 生成、发布、验证、报告”引导用户完成发版。

CLI 的 `release` 命令可以在这个阶段逐步开放真实执行，但仍建议保留 `--dry-run`、`--yes`、`--step`、`--resume-from` 等控制参数。默认行为仍应安全：没有 `--yes` 时不执行危险动作；页面触发危险动作前必须完成确认。

阶段三还可以加入发布历史页面，列出最近多次 release run，方便回看某个版本当时发布了哪些产物、manifest 内容是什么、校验结果如何、失败和重试记录是什么。

## 9. 易用性设计

页面应从“展示已有状态”升级为“指导用户完成发布”。

首先是下一步建议。系统应根据当前状态自动判断下一步。例如没有 release 目录时提示“先创建或选择 release 目录”；manifest 缺失时提示“先生成 manifest”；产物缺签名时提示“回到 Windows 构建机检查签名产物”；线上版本低于目标版本时提示“可以发布”；线上版本等于目标版本时提示“可能已经发布过，请谨慎重复操作”。

其次是缺失产物诊断。页面不应只显示实际存在的文件，还应显示理论上必须存在的文件。Windows 至少应检查 NSIS、MSI、绿色 zip 及其 `.sig`，Mac 至少应检查 `OMD.app.tar.gz` 及其 `.sig`。每个目标产物显示存在、缺失、大小、修改时间和签名状态。

再次是 manifest 对比。页面应展示 GitHub 和 Gitee manifest 的版本、发布时间、平台目标、下载 URL 和签名状态，并标记两边是否一致。如果 GitHub 有某个平台而 Gitee 缺失，或者 URL 版本与目标版本不一致，页面应明确提示。

命令区也应更友好。命令不应只是一个大文本框，而应按步骤分组。每个命令卡片说明“这一步做什么、什么时候运行、失败后看哪里、是否有风险”。低风险命令可以提供运行按钮，高风险命令只提供复制和确认入口。

错误提示应面向行动。比如不要只显示“命令执行失败”，而应尽量显示“缺少 gh CLI，请先安装或确认 PATH”；“Gitee 凭据不可用，请检查 git credential”；“updater endpoint 缺少 GitHub latest.json，请检查 tauri 配置”。

## 10. 配置设计

当前脚本中写死了源仓库、GitHub 仓库、Gitee 仓库、Windows SSH、Windows 仓库路径、updater URL 和代理地址。建议新增 `release.config.json`，将非敏感配置外置化。

建议配置包括：sourceRepo、githubRepo、giteeOwner、giteeRepo、giteeLatest、githubLatest、windowsSsh、windowsRepo、windowsTarget、githubProxy、releaseDirPattern、requiredArtifacts。

敏感信息不应写入配置文件，例如 token、密码、私钥内容。系统只检测这些凭据是否存在，例如通过 `gh auth status`、`git credential fill` 或 SSH BatchMode 检查。

页面可以读取配置摘要并展示，但不要显示敏感内容。配置异常时，页面应提示具体字段缺失或格式错误。

## 11. 状态文件设计

建议将 release 目录下的 `state.json` 标准化，作为页面展示和失败恢复的核心数据源。

建议结构包含：version、releaseDir、sourceRepo、commit、createdAt、updatedAt、currentStep、overallStatus、checks、steps、artifacts、manifests、errors 和 suggestions。

checks 用于记录预检项，例如源码版本、Tauri 版本、updater key、gh CLI、GitHub 登录、SSH、Gitee 凭据、updater endpoints。

steps 用于记录发布步骤，例如 preflight、artifactCheck、checksums、manifestGithub、manifestGitee、dryRunRelease、publishGithub、publishGitee、verify。

artifacts 用于记录必须产物和实际产物，包括平台、文件名、是否存在、大小、修改时间、checksum 和签名状态。

manifests 用于记录 GitHub/Gitee manifest 的版本、平台目标、URL、签名状态和一致性检查结果。

suggestions 用于给页面提供下一步建议，避免前端重复实现复杂判断。

## 12. 后端 API 设计

建议保留现有 `GET /api/summary` 和 `GET /api/commands`，并新增以下接口。

`GET /api/config` 返回当前发布配置摘要。

`POST /api/actions/preflight` 执行预检，并写入 `state.json`。

`POST /api/actions/check-artifacts` 检查必须产物和签名文件。

`POST /api/actions/manifest` 生成 checksums 和 GitHub/Gitee manifest。

`POST /api/actions/verify` 验证线上 updater 端点和下载 URL。

`POST /api/actions/dry-run-release` 输出完整 dry-run 发布命令，并写入状态。

后续阶段可以新增 `POST /api/actions/publish-github` 和 `POST /api/actions/publish-gitee`，但这两个接口必须要求 confirmVersion 字段，且 confirmVersion 必须等于目标 version。

所有 action API 都应返回结构化结果，包括 ok、action、status、message、state、logs 和 suggestions。

## 13. CLI 改造建议

CLI 应从“打印进度文本”升级为“既打印人类可读日志，也写入机器可读状态”。

建议新增公共状态写入模块，封装 `startStep`、`completeStep`、`failStep`、`recordCheck`、`recordArtifact`、`recordSuggestion` 等函数。这样 `preflight`、`manifest`、`verify`、`release` 可以共享状态写入逻辑。

`preflight` 建议拆成多个独立检查项，即使某一项失败，也尽量继续检查其他项目，最终汇总失败。这比遇到第一个失败就退出更适合页面诊断。

`manifest` 建议先运行产物完整性检查，再生成 checksums 和 manifest。如果缺少签名文件，应在状态里明确列出缺失文件。

`verify` 建议记录每个 endpoint、每个平台 URL 的 HTTP 状态、版本匹配状态和签名存在状态。

`release` 建议暂时继续默认阻断真实执行，但可以先完善 `--dry-run` 输出和状态写入。等阶段二开始后，再逐步开放单步真实执行。

## 14. 安全控制

真实发布相关动作必须具备以下安全控制。

所有版本号必须符合 SemVer。所有 releaseDir 必须限制在允许目录范围内，避免用户传入任意系统路径。所有命令必须由后端白名单生成，不能从前端接收任意命令字符串。真实发布前必须要求版本号确认。线上已有相同版本时必须提示重复发布风险。发布 GitHub 和 Gitee 应分别执行，不能其中一个失败后继续假装成功。失败时必须保留日志和状态，不自动清理 release 目录。默认执行 dry-run，只有明确确认时才执行真实动作。

## 15. 推荐实施优先级

第一优先级是标准化 `state.json`，让 CLI 的检查和执行结果都能结构化保存。这是页面可视化、失败诊断和后续恢复执行的基础。

第二优先级是新增安全 action API，让页面可以触发 `preflight`、`manifest`、`verify` 和 `dry-run release`，但不接收任意命令。

第三优先级是增强前端交互，包括版本输入、发布目录输入、步骤按钮、步骤状态、失败原因、下一步建议和缺失产物清单。

第四优先级是配置外置化，新增 `release.config.json`，减少脚本内硬编码。

第五优先级才是接入真实 GitHub/Gitee 发布动作。真实上传应在状态追踪、确认机制和错误恢复稳定后再开放。

## 16. 最小可落地版本

建议下一步先实现一个低风险小闭环：页面输入版本号和 release 目录，点击按钮运行预检、生成 manifest、验证端点和 dry-run release。每一步都有状态、日志、失败提示和下一步建议。真实 GitHub/Gitee 上传仍不从页面执行。

这个版本完成后，系统就能从“只看状态”升级为“可操作的发布准备中心”，能够显著减少发版前的人工判断成本，同时不引入真实误发布风险。

## 17. 后续演进

当最小可落地版本稳定后，可以继续加入真实发布的分段执行、发布历史、release run、失败恢复、报告归档和多版本对比。长期目标是让 OMD Release Console 成为 OMD 双端发布的唯一入口：所有发布步骤都可见、可确认、可追踪、可复盘。
