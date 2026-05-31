# cc-switch-web v0.15.0-rc.2 预发布说明

v0.15.0-rc.2 是 **Local Routing + Claude Desktop 对齐版** 的候选预发布，用于替代资产不完整的 v0.15.0-rc.1。

## 新增与对齐

- 新增 Local Routing 入口与配置闭环，继续向 cc-switch 的本地路由能力对齐。
- 新增 Claude Desktop 应用类型识别与配置目录入口，和 Claude Code 共享 Claude 配置目录基础能力。
- 保持 Claude Desktop 在 Prompt、MCP、Skills 与通用配置片段等未完全适配功能中的显式边界，避免误写入不支持的配置面。

## 修复

- 修复 v0.15.0-rc.1 发布流水线中 web-server feature 构建失败的问题。
- 补齐 web API 配置处理器中的 Claude Desktop 分支，确保 server 二进制和 `latest.json` 能随 Release workflow 正常生成。

## 说明

- Rectifier 当前是配置入口和体验对齐入口，不是完整的上游修复引擎。
- v0.15.0-rc.1 的桌面端资产已上传，但缺少 server 二进制和 `latest.json`，因此不建议作为有效候选版本使用。

## 验证

- `pnpm build:web`
- `cargo check --manifest-path src-tauri/Cargo.toml --features web-server --example server`
- `pnpm typecheck`
