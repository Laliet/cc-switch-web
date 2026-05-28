# cc-switch-web v0.12.0-rc.3 预发布

这是 v0.12.0 的第三个预发布版本，用于修正 RC 构建元数据并继续验证 0.12.0 的 SQLite 存储迁移、代理统计和 Web/headless 工作流。

## 相比 rc.2 的修正

- 修复预发布构建的内部版本号：Release workflow 现在会在构建前从 Git tag 注入版本号。
- 桌面应用元数据、Tauri 配置、Rust 包版本和 server binary 构建会使用 `v0.12.0-rc.3` 对应的 `0.12.0-rc.3`。
- 避免应用内版本、安装器元数据、更新器版本比较或 headless server 版本信息仍显示/识别为正式 `0.12.0`。

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
