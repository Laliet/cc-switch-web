# cc-switch-web v0.18.0 发布说明

v0.18.0 是 **上游 3.15.0 对齐与 Web/headless 可靠性增强** 版本，重点回灌 cc-switch 代理可靠性逻辑，并补齐 Web 版在 Deep Link、WebDAV、Skills、Usage Dashboard 和 Failover/Health 上的关键能力。

## 代理可靠性

- 对齐上游 cc-switch 3.15.0 的请求转发与转换逻辑，增强 OpenAI Chat、OpenAI Responses、Gemini Native、Copilot 与 Codex OAuth 路径。
- 补齐 `tool_choice` 映射、工具参数 canonical JSON、Responses/Gemini/Copilot/Codex 响应转换、usage 解析和 final usage delta。
- 优化 HTTP client 默认行为，包含 600 秒请求超时、30 秒连接超时、连接池、TCP keepalive、禁用自动压缩以及 IPv6/监听地址处理。
- 增加 thinking signature/budget rectifier，在 Anthropic 兼容上游返回特定 400/422 错误时按上游逻辑重试修正。
- 迁入 Bedrock thinking optimizer 与 cache injector，并默认关闭，仅在明确启用且 provider 标记为 Bedrock 时生效。

## Failover 与 Health

- 代理状态新增 provider health、熔断状态、窗口失败率和 circuit reset 能力。
- Failover 队列按配置顺序选择备用 provider，并跳过已熔断 provider。
- 前端设置页新增 health badge、reset circuit breaker 操作，并校验 failover 必须依赖 takeover 启用，避免 Web/headless 下出现“看起来开着但实际不可用”的状态。

## Deep Link

- Web API 与前端确认弹窗支持 provider、MCP、prompt、skill 深链接导入。
- Provider deep link 支持 inline `config` JSON/TOML merge，显式 URL 参数优先级高于 config 字段。
- Skill deep link 复用现有 SkillService 下载逻辑，保留中国大陆网络下的 GitHub 镜像兜底。
- 桌面 deep link 事件、Tauri command 与 Web API 都统一走 parse + merge 逻辑。

## WebDAV 同步

- WebDAV 从单次 snapshot 扩展为可用云同步，支持 latest snapshot、历史备份 index、保留最近版本、备份列表和指定版本恢复。
- 新增 sync 决策：本地上传、远端下载、无变化、远端为空、本地为空与冲突提示。
- 新增 Web/headless 自动同步 worker，按保存的 WebDAV 设置周期执行，并记录同步 marker。

## Skills

- 卸载 skill 前自动备份，并支持备份列表、恢复和删除。
- 支持 ZIP/.skill 导入，兼容根 `SKILL.md` 和多目录 skill 包。
- 增加 skill 存储位置与同步方式设置，支持 `~/.cc-switch/skills` 与 `~/.agents/skills` 作为 SSOT，并提供迁移入口。

## Usage Dashboard

- 新增日期范围选择器、Pricing 编辑弹窗和更完整的请求表格字段。
- 请求日志展示 billing model、fresh input tokens、cache read/write、unpriced 状态、倍率、总延迟和首 token 延迟。
- 增强 pricing 与 usage helper，提升 cache-inclusive provider、零成本和未定价数据的展示准确性。

## 验证

- 已通过 TypeScript 类型检查、Deep Link/adapter/API Vitest、Proxy/WebDAV/Usage/Settings 组件测试。
- 已通过 Rust Deep Link、WebDAV、optimizer、cache、proxy settings roundtrip 测试。
- 已通过 desktop feature lib check 与 web-server example check。
- 已进行 Web/headless 手动路径测试：本地 server 启动、JSON 到 SQLite 迁移、provider API、proxy 启停、Claude/Codex 中转转发、Failover/Health、Deep Link、Skills、Usage 和本地 WebDAV 同步。
