# OMD Release Console

OMD 双端发布控制台是一个独立项目，不放进 `dix-extension-ui` 产品仓库。

## 使用方式

```bash
cd /Users/glame/Desktop/omd-release-console
npm run dev
```

浏览器调试模式仍然保留：

```bash
npm run serve
```

启动桌面窗口后，可以查看和执行安全发布动作：

- 最近的 release 目录
- GitHub/Gitee manifest 状态
- Windows/Mac 产物清单
- `checksums.sha256` 摘要
- 预检、Windows 4090 打包、Windows 产物收集、Mac 打包、生成 manifest、GitHub/Gitee 发布、线上验证、发布报告

## 发布原则

```text
4090 Windows 构建机：只负责 Windows 构建和签名
Mac 主控机：统一收集产物、生成 manifest、发布 GitHub/Gitee、验证端点
```

## 4090 Windows 构建脚本

Windows 打包由 4090 上的长期脚本执行：

```powershell
C:\Users\4090\Desktop\omd-release-build.ps1 -Version 0.0.7
```

Release Console 会在执行 `build-windows` 前上传仓库内的 `scripts/windows/omd-release-build.ps1`，然后通过 SSH 调用它。脚本使用固定构建目录 `C:\Users\4090\Desktop\dix-extension-ui-release-build`，保留 `node_modules` 和 `src-tauri\target` 以加速后续构建。构建成功后会在 4090 的 export 目录生成 `windows-artifacts.zip` 和 `build-summary.json`，`collect-windows-artifacts` 只拉取这个 zip。

脚本是版本无关的；每次发布通过 `-Version` 传入目标版本，并要求 `package.json` 和 `src-tauri\tauri.conf.json` 已经等于该版本。

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
cargo run -- build-windows 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- build-mac 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- collect-windows-artifacts 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- check-artifacts 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- manifest 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --step publish-github --confirm-version 0.0.7
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --step publish-gitee --confirm-version 0.0.7
cargo run -- verify 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- report 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --confirm-version 0.0.7
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --dry-run
```

当前后端和 CLI 已迁移为 Rust。桌面工具执行固定白名单动作：本机 Mac 打包、4090 Windows 打包、产物收集、manifest、GitHub/Gitee 发布、验证和报告。真实 GitHub/Gitee 上传必须输入与目标版本一致的确认版本，避免误触发真实发布。桌面版会内嵌默认发布配置，发布历史写入 macOS Application Support，不依赖从仓库目录启动。

## 分段发布动作

Rust 后端提供真实发布入口，但仍由版本确认闸门保护：

```bash
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --step publish-github --confirm-version 0.0.7
cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS --step publish-gitee --confirm-version 0.0.7
```

对应 API 为 `POST /api/actions/publish-github` 和 `POST /api/actions/publish-gitee`。请求必须包含与 `version` 完全一致的 `confirmVersion`。

- GitHub 发布通过 `gh release create` 创建 `v<version>` Release，并上传 `github/latest.json`、`windows/`、`mac/` 下的固定产物文件。
- Gitee 发布通过 Gitee v5 API 创建 Release、上传 `gitee/latest.json`、`windows/`、`mac/` 下的固定产物文件，并更新仓库根目录 `latest.json`。
- UI/API 不接受任意 shell，只能触发 Rust 白名单动作。
