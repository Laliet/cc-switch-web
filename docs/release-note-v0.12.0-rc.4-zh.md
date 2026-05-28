# cc-switch-web v0.12.0-rc.4 预发布

这是 v0.12.0 的第四个预发布版本，用于修正 RC 构建元数据和 Windows MSI 版本兼容问题。

## 相比 rc.3 的修正

- 修复 Windows MSI 对 prerelease 版本号的限制导致的打包失败。
- npm/Cargo 版本仍使用 `0.12.0-rc.4`，便于 headless/server 和日志识别真实 RC 版本。
- Tauri 桌面包和 updater 版本使用 MSI 兼容格式 `0.12.0-4`，避免 Windows 打包失败。
- `latest.json` 的 updater 版本同步使用 Tauri 兼容版本，确保桌面更新器版本比较一致。

## 重点验证

- 从旧 `~/.cc-switch/config.json` 迁移到 `~/.cc-switch/cc-switch.db`。
- Provider、MCP、Prompt、Skill、Proxy 设置的读写是否都以 SQLite 为运行时主状态。
- Proxy request log、usage rollup、pricing/cost 和 failover 行为。
- Universal Provider 的后端/API/设置页最小工作流。
- Linux server binary、Docker 镜像、桌面安装包和 Tauri updater 元数据。

## 已知边界

- 这是预发布版本，适合手动测试和问题回归验证。
- Universal Provider 仍不是完整上游级别的专门页面和高级模型映射体验。
- 模型价格种子仍是子集，可通过 UI/API 继续维护。
