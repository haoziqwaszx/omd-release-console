# OMD Release Console

OMD 双端发布控制台是一个独立项目，不放进 `dix-extension-ui` 产品仓库。

## 使用方式

```bash
cd /Users/glame/Desktop/omd-release-console
cargo run -- serve
```

也可以继续使用 npm 包装命令：

```bash
npm run dev
```

打开终端输出的本地地址后，可以查看和执行安全发布准备动作：

- 最近的 release 目录
- GitHub/Gitee manifest 状态
- Windows/Mac 产物清单
- `checksums.sha256` 摘要
- 预检、生成 manifest、线上验证、dry-run 发布命令

## 发布原则

```text
4090 Windows 构建机：只负责 Windows 构建和签名
Mac 主控机：统一收集产物、生成 manifest、发布 GitHub/Gitee、验证端点
```

## 目录约定

发布输出目录默认形如：

```text
~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS/
  windows/
  mac/
  github/latest.json
  gitee/latest.json
  checksums.sha256
  state.json
  release-report.md
```

## CLI 骨架

```bash
cargo run -- plan
cargo run -- preflight 0.0.7
cargo run -- manifest 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- verify 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --dry-run
```

当前后端和 CLI 已迁移为 Rust。页面只执行低风险发布准备动作；真实 GitHub/Gitee 上传仍保持禁用，避免误触发真实发布。

## Phase 2A 分段发布安全骨架

Rust 后端已预留分段发布入口，但不会自动上传：

```bash
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --step publish-github --confirm-version 0.0.7
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --step publish-gitee --confirm-version 0.0.7
```

对应 API 为 `POST /api/actions/publish-github` 和 `POST /api/actions/publish-gitee`。请求必须包含与 `version` 完全一致的 `confirmVersion`。确认通过后，系统只写入 `manual_required` 状态并输出需人工核对执行的命令，不执行真实上传。
