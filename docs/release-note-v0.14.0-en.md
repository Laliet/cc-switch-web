# cc-switch-web v0.14.0 Prerelease Notes

v0.14.0 completes the Usage Dashboard, request logs, and pricing/cost loop for proxy-backed usage.

## Added

- Full Usage Dashboard with total cost, request count, real token usage, success rate, cache hit rate, and per-app breakdown.
- Usage trend chart for proxy request cost over the selected time range.
- Request log table with Provider, Model, status, time-range filtering, and pagination.
- Request detail panel with token, cost, latency, streaming, error, and data-source fields.
- Provider and Model statistics for cost, token, request count, success rate, and latency analysis.
- Dashboard model-pricing editor with historical zero-cost proxy log backfill after pricing updates.
- Claude, Codex, and Gemini local session log import with incremental sync, cross-source de-duplication, and model-pricing cost calculation.
- Desktop Tauri commands and Web/headless `/api/usage/*` endpoints.

## Notes

- Dashboard statistics combine live proxy request logs, historical daily rollups, and imported Claude/Codex/Gemini session logs.
- Session log import de-duplicates against proxy logs before writing records to avoid double charging the same request.
