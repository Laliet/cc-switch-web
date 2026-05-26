# cc-switch-web v0.12.0-rc.1

> SQLite 运行时存储迁移与 Web/headless 能力对齐的预发布候选版本

**[English Version ->](release-note-v0.12.0-rc.1-en.md)**

---

## 概览

`v0.12.0-rc.1` 是 `0.12.0` 的第一个候选版本。本版本重点完成运行时存储架构迁移：cc-switch-web 的主状态从 legacy JSON snapshot 切换为 SQLite 数据库，并继续补齐 Web/headless 场景下的 proxy usage、failover、Universal Provider 与模型价格管理。

新的运行时主存储：

```text
~/.cc-switch/cc-switch.db
```

旧的 `~/.cc-switch/config.json` 仍保留为 legacy import/export snapshot，用于导入、导出和备份兼容，但不再是运行时主状态。

---

## 主要更新

- 将运行时主状态迁移到 SQLite-backed `AppState`。
- 新增并拆分数据库层：schema、migration、backup、DAO。
- Provider、MCP、Prompt、Skill、Config import/export、Proxy config 等核心读写路径改为 DB-backed。
- 启动时支持从 legacy `config.json` 和旧 settings proxy 配置导入到 SQLite。
- `config.json` 明确降级为 legacy import/export snapshot。
- 新增 provider health、proxy request logs、usage daily rollup、failover queue、model pricing、universal providers 等数据库表和 DAO。
- Proxy request logs 会落库，并解析响应 usage、cache token、streaming usage、first token latency 和 cost。
- Proxy failover 现在可优先使用 DB failover queue。
- 新增 failover queue 的 Tauri commands、Web API 和设置页管理入口。
- 新增 Universal Provider typed model、DAO、service、Tauri commands、Web API 和设置页基础工作流。
- 新增模型价格 DAO/API/UI，并扩展默认模型价格 seed。
- Web/headless 模式下测试 home 隔离更稳定，避免误读真实账户目录。

---

## 存储迁移说明

首次启动新版本时：

1. 如果 SQLite 数据库尚未迁移且核心表为空，会尝试读取 legacy `~/.cc-switch/config.json`。
2. 如果 legacy JSON 不存在，则执行默认配置和 live config 自动导入。
3. 导入结果写入 `~/.cc-switch/cc-switch.db`。
4. legacy settings 中的 proxy 配置会同步导入 DB proxy config。
5. 后续运行时以 SQLite 为主，不再以 `config.json` 为权威状态。

建议升级前备份：

```text
~/.cc-switch/config.json
~/.cc-switch/settings.json
~/.claude.json
~/.codex/auth.json
~/.codex/config.toml
~/.gemini/.env
~/.gemini/settings.json
```

---

## 验证

本候选版本发布前已通过：

- `cargo test --features web-server`
- `cargo test --features web-server database`
- `cargo test --features web-server --test proxy_web_api`
- `cargo test --features web-server --test provider_commands`
- `cargo test --features web-server --test provider_service`
- `cargo test --features web-server --test mcp_commands`
- `cargo test --features web-server --test prompt_service`
- `cargo test --features web-server --test import_export_sync`
- `pnpm typecheck`
- `cargo fmt --all`
- `git diff --check`

覆盖重点包括：

- SQLite provider/MCP/prompt/skill/proxy/universal provider roundtrip。
- legacy JSON import/export snapshot 边界。
- Codex provider switch 后 MCP 同步和 live config snapshot 刷新。
- Proxy request log、usage/cost、streaming usage update。
- DB failover queue 选择和 Web API 管理。
- Universal Provider API roundtrip、sync 和 generated provider 删除。
- Web-server feature 下的路径隔离。

---

## 已知边界

- 这是 `0.12.0` 候选版本，不是最终稳定版。
- `config.json` 仍会用于 legacy 导入、导出、备份兼容，但不是运行时主存储。
- Universal Provider 已具备基础工作流，但还不是完整上游级别的高级模型映射体验。
- 模型价格 seed 已扩展，但不承诺覆盖所有供应商的全部模型；可通过 UI/API 自行维护。
- 本地代理仍是应用级 HTTP API proxy，不是系统透明代理。
- Gemini OAuth provider 仍不支持 proxy takeover；Gemini takeover 请使用 API Key provider。
