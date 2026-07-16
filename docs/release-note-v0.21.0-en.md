# cc-switch-web v0.21.0 Release Notes

> Released on 2026-07-16. This is the current stable release.

v0.21.0 is the **OpenClaw Phase Two** release. It completes workflows introduced in v0.20.0 instead of adding another partially supported application. Core behavior remains aligned with `farion1231/cc-switch v3.15.0` commit `9e3f1689038febb36da08993cd47281426b5dd7c`, pinned in [`upstream-v3.15.0.lock`](upstream-v3.15.0.lock).

## OpenClaw Configuration

- Adds structured editors for the model catalog, `agents.defaults`, Environment, and Tools/Profile.
- Keeps an advanced raw JSON5 editor for fields not represented by structured controls.
- Preserves comments and unknown fields during structured writes, with atomic replacement, backups, SHA-256 ETags, and explicit HTTP 409 conflicts.
- Adds external Provider discovery, difference preview, selective apply, idempotent import, and startup refresh for providers already marked as live-managed.
- Never removes a Web-managed Provider merely because it is absent from the live OpenClaw file.

## Sessions and Daily Memory

- Moves Session search to the server so results cover the complete host-side snapshot rather than only the first loaded page.
- Binds cursors to provider and search filters and ignores obsolete browser search responses.
- Virtualizes long message lists and user-message outlines to keep DOM size bounded.
- Adds Daily Memory search, full-content viewing, ETag-protected deletion, and a backup before deletion.

## Skills Discovery and Import

- Scans only fixed supported directories for Claude, Codex, Gemini, OpenCode, unified agent storage, and CC Switch storage.
- Browser requests use trusted source labels, never arbitrary server paths.
- Shows source, target, content equality, multi-source conflicts, and target apps before import.
- Uses explicit overwrite confirmation, atomic unified-storage replacement, configured copy/symlink synchronization, and idempotent multi-app state registration.

## Provider Presets and Routing Status

- Accounts for every upstream v3.15.0 OpenClaw, Gemini, and Codex preset while retaining local China-accessible entries.
- Shows P1/P2 failover order, the Provider currently receiving proxy traffic, circuit state, failure counts/window, recent failure time, and latest Stream Check time.

## Known Boundaries

- Hermes Agent is intentionally deferred to a dedicated later release because its Provider, MCP, Skills, memory, session, and Web UI scope is independent and large.
- OpenClaw local routing, MCP, prompts, Skills, and usage remain unsupported; upstream v3.15.0 does not provide those OpenClaw integrations either.
- Web mode manages the server host, not files or applications on the browser device.
