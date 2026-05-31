# cc-switch-web v0.14.1 Release Notes

v0.14.1 is a patch release for the v0.14 Usage Dashboard, focused on auto-refresh ranges, historical rollup display, request-log pagination, and model-pricing match boundaries.

## Fixed

- Fix Usage Dashboard auto-refresh so relative ranges such as `1d`, `7d`, and `today` recompute on every refetch.
- Fix request logs so changing the global app or time range resets pagination to the first page.
- Fix short-range trend queries that only have `usage_daily_rollups` historical data so they no longer render as empty.
- Fix model-pricing matching so broad prefixes such as `gpt-4` do not incorrectly match `gpt-4o` / `gpt-4.1` during session import or historical cost backfill.
- Preserve valid namespaced matches such as `provider/custom-model:extra` for `custom-model` pricing.

## Verified

- `pnpm vitest run tests/lib/query/usage.test.tsx`
- `pnpm vitest run tests/lib/query/usage.test.tsx tests/components/usage/RequestLogTable.test.tsx`
- `cd src-tauri && cargo test services::usage_stats --features desktop,test-hooks`
- `pnpm typecheck`
- `git diff --check`
