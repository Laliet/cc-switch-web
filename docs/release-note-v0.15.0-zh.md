# cc-switch-web v0.15.0 发布说明

v0.15.0 是 **Local Routing + Claude Desktop 对齐版** 的正式版本，整合 v0.15.0-rc 系列测试反馈，重点补齐本地路由、Claude Desktop 对齐、用量统计可见性、Web 模式稳定性与错误反馈。

## 用量页修复

- 用量 Dashboard 首次打开时，如果 Today 没有请求但数据库存在历史用量，会自动切到最新用量所在的 7 天窗口。
- 自动校准按当前 App 筛选生效，切到 Codex 时会优先使用 Codex 自己的最新用量时间范围。
- 时间范围下拉增加 `All time`，用户可以直接查看全量卡片、趋势、日志和统计表。
- 自动校准窗口在下拉框中显示为 `Recent data`，用户手动选择任意时间范围后不会再被自动覆盖。
- Data sources 标题调整为 `All-time data sources`，明确它展示的是全量来源统计，不是当前时间范围内的统计。

## 接口与测试

- 新增用量数据范围接口，用于返回 `firstSeenAt`、`lastSeenAt` 和请求数量。
- Web API 新增 `/api/usage/data-extent`，桌面端新增 `get_usage_data_extent` command。
- 修复 Codex 用量同步中中文模型名可能触发 UTF-8 byte boundary panic 的问题。
- Web 模式下不存在的 `/api/*` 路径现在返回 JSON 404，不再被 SPA fallback 伪装成 HTML 200。
- 前端增加全局 API 失败 toast，并对断线、超时、HTML 错误响应给出更明确的错误消息。
- 用量 Dashboard 增加内联错误态，关键用量请求失败时不再静默显示为 0。
- 增加前端 range helper 与 Dashboard 自动校准测试。
- 增加 Rust 内存库单测，覆盖按 App 查询用量首末时间。
- 增加中文模型名规范化和 Web API 未匹配路由回归测试。
- 增加 API adapter 错误响应和全局 query client 错误 toast 回归测试。

## 说明

- Rectifier 当前仍是配置入口和体验对齐入口，不是完整的上游修复引擎。
- v0.15.0-rc.1、v0.15.0-rc.2、v0.15.0-rc.3 均为测试候选版本；正式使用建议升级到 v0.15.0。

## 验证

- `pnpm typecheck`
- `pnpm vitest run tests/lib/query/queryClient.test.ts tests/lib/adapter.core.test.ts tests/components/usage/UsageDashboard.test.tsx tests/lib/usageRange.test.ts tests/lib/query/usage.test.tsx`
- `cargo test --manifest-path src-tauri/Cargo.toml usage_data_extent_reports_latest_data_by_app --lib`
- `cargo test --manifest-path src-tauri/Cargo.toml codex_model_normalization_handles_multibyte_names --lib`
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features web-server,test-hooks --test web_auth test_missing_api_route_returns_json_404`
- `pnpm build:web`
- `git diff --check`
