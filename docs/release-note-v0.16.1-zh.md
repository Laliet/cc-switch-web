# cc-switch-web v0.16.1 发布说明

v0.16.1 是 v0.16.0 的发布流程热修版本，功能内容与 v0.16.0 保持一致，重点修复 Windows 发布打包导致的整条 Release workflow 中断问题。

## 发布修复

- Windows Release 构建改为只生成当前发布流程实际上传的 MSI 包。
- 避免在 MSI 已构建成功后继续下载并打包未使用的 NSIS installer，防止外部下载 504 使 Windows job 失败。
- 补齐 v0.16.x 发布说明文件，并在 Release workflow 中增加发布说明存在性检查。

## 影响范围

- v0.16.0 首次发布已成功上传 macOS 和 Linux 桌面产物，但 Windows、server 二进制、`latest.json` 与 Docker 镜像缺失。
- 建议用户直接使用 v0.16.1；v0.16.1 会重新生成完整桌面产物、server 二进制、`latest.json` 和 Docker 镜像。
