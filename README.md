# OMD Release Console

OMD 双端发布控制台是一个独立项目，不放进 `dix-extension-ui` 产品仓库。

## 使用方式

```bash
cd /Users/glame/Desktop/omd-release-console
npm run dev
```

打开终端输出的本地地址后，可以查看：

- 发布总流程
- 最近的 release 目录
- GitHub/Gitee manifest 状态
- Windows/Mac 产物清单
- `checksums.sha256` 摘要
- 下一版发布常用命令

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
node scripts/omd-release.mjs plan
node scripts/omd-release.mjs preflight 0.0.7
node scripts/omd-release.mjs manifest 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
node scripts/omd-release.mjs verify 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release-YYYYMMDD-HHMMSS
```

第一版前端只展示和检查，不从页面直接执行发布，避免误触发真实上传。
