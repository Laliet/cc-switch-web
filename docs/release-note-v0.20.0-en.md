# cc-switch-web v0.20.0 Release Notes

> Released on 2026-07-15. This is the current stable release.

v0.20.0 focuses on Web/headless parity, OpenClaw phase one, and proxy diagnostic and failover reliability. Core behavior continues to follow `farion1231/cc-switch v3.15.0`, with only the runtime differences required by a browser-managed server host. The comparison baseline is pinned in [`docs/upstream-v3.15.0.lock`](upstream-v3.15.0.lock).

## Runtime Capabilities

- Adds a shared runtime capability contract for desktop and Web, including global and per-app features.
- Returns stable coded HTTP 501 responses when an app is known but the requested feature is unavailable; unknown app names remain HTTP 400.
- Hides or degrades native-only file dialogs, tray, application updates, portable settings, environment management, and native endpoint tests in Web mode.
- Makes it explicit that Web configuration, Sessions, Workspace, and Claude Desktop status belong to the cc-switch-server host, not the browser device.
- Claude Desktop profiles require a supported macOS or Windows host; Linux reports the unsupported boundary directly.

## OpenClaw Phase One

- Adds additive OpenClaw provider management without replacing unrelated user/OpenClaw-managed providers.
- Adds default-model read/set/clear operations and provider/default-model/config health status.
- Adds an allowlisted Workspace editor for `AGENTS.md`, `SOUL.md`, `USER.md`, `IDENTITY.md`, `TOOLS.md`, `MEMORY.md`, `HEARTBEAT.md`, `BOOTSTRAP.md`, and `BOOT.md`.
- Workspace writes use ETag conflict protection, pre-write backups, restore, a 20-backup limit per file, and a 1 MiB file limit.
- Adds list/read/write support for `memory/YYYY-MM-DD.md` daily memory.
- Adds OpenClaw agent sessions to Session Manager scanning, message viewing, and protected deletion.
- Local routing, MCP, prompts, skills, and usage are outside the phase-one scope.

## Sessions

- Adds snapshot-bound pagination so a cache refresh expires an old cursor instead of reordering an active result set.
- Caches each provider independently and rescans only providers whose bounded file-tree fingerprint changed.
- Does not follow symbolic links and forces a rescan when depth or entry limits prevent a complete fingerprint.
- Rejects source symlinks, parent-symlink escapes, and paths outside known provider roots.
- Removes server absolute paths from Web error responses.

## Proxy Reliability

- Adds real HTTP conformance coverage for Anthropic Messages, OpenAI/Codex Responses, Gemini Native, full Vertex URLs, and Claude Desktop OpenAI Chat/Responses/Gemini conversions.
- Verifies method, URL, auth, `tool_choice`, underscore JSON Schema fields, `session_id`, and `prompt_cache_key` correlation.
- Distinguishes missing usage, partial usage, and explicitly reported all-zero usage.
- Preserves SQLite failover queue order and uses a deduplicated `backupCurrent` only as the final fallback.
- Exercises streaming first-byte timeouts with headers flushed before a delayed body.
- Determines Claude Desktop restart state by comparing the latest CC Switch write with the desktop process start time.

## Diagnostics, Quotas, and OMO

- SQLite Schema v7 adds retained Stream Check history, indexes, saved configuration, filtered history, and latest-per-provider results.
- Adds provider subscription quota summaries and a settings overview.
- OMO/OMO Slim MCP and Skills continue to share OpenCode storage; prompts, usage, sessions, and local routing remain disabled.
- Proxy settings and takeover routes continue to reject OMO/OMO Slim.

## WebDAV and Compatibility

- New WebDAV v2 uploads use `v2/db-v7/<profile>/`.
- Preview, download, history listing, and restore fall back to v0.19.1 `db-v6` current snapshots and history.
- Legacy v0.18 JSON snapshots remain a read-only migration source.
- Manifests validate protocol/schema, artifact size/SHA256, and that `snapshotId` matches the artifact hashes.
- Stream Check history remains machine-local and is preserved when a db-v6 or db-v7 snapshot is restored.

## Web Error Disclosure

- Internal 5xx responses no longer expose raw errors; intentional 501 capability messages remain public.
- Authentication failures do not relay upstream response bodies, and configuration errors do not expose configuration details.
- Public 4xx responses apply a final guard against absolute paths and credential fields.
- OpenClaw write results expose only a backup identifier, and health warnings do not echo parser details, provider IDs, or model references.
- Workspace, Session, and Skill errors must not expose server paths or file content.

## Upgrade and Downgrade

- A v0.19.1 Schema v6 database is migrated automatically on the first v0.20.0 start.
- Create a SQLite backup before upgrading and avoid concurrent desktop/server writes to the same database.
- v0.19.1 cannot open a database after it has been upgraded to Schema v7. Restore a pre-upgrade v6 backup before downgrading.
- db-v6 WebDAV locations are read/restore compatibility sources only; v0.20.0 writes new data to db-v7.

## Known Boundaries

- Web mode cannot control files, terminals, or desktop applications on the browser device.
- OMO/OMO Slim and OpenClaw do not participate in proxy takeover.
- Claude Desktop profiles do not manage MCP, prompts, skills, usage, or sessions.
