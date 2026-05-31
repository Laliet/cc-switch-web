# cc-switch-web v0.14.1 发布说明

v0.14.1 是 v0.14 系列 Usage Dashboard 的补丁发布，重点修复自动刷新、历史 rollup 展示、request logs 分页以及模型定价匹配边界。

## 修复

- 修复 Usage Dashboard 自动刷新时相对时间范围不会前进的问题，`1d`、`7d`、`today` 等范围现在会在每次 refetch 时重新计算。
- 修复 request logs 在切换全局 App 或时间范围后仍停留旧页码的问题，避免筛选后误显示为空。
- 修复短时间范围查询只有 `usage_daily_rollups` 历史归档数据时趋势图为空的问题。
- 修复模型定价匹配过宽的问题，避免 `gpt-4` 错误匹配 `gpt-4o` / `gpt-4.1` 并导致 session import 或历史回填成本错误。
- 保留合法的命名空间模型匹配，例如 `provider/custom-model:extra` 仍可匹配 `custom-model` 定价并回填成本。

## 验证

- `pnpm vitest run tests/lib/query/usage.test.tsx`
- `pnpm vitest run tests/lib/query/usage.test.tsx tests/components/usage/RequestLogTable.test.tsx`
- `cd src-tauri && cargo test services::usage_stats --features desktop,test-hooks`
- `pnpm typecheck`
- `git diff --check`
