# cc-switch-web v0.11.0

> Stable release for Web/headless proxy, client takeover, and OpenCode / OMO management improvements

**[中文更新说明 Chinese Documentation ->](release-note-v0.11.0-zh.md)**

## Overview

`v0.11.0` is the stable release of the `0.11` line. It promotes the Web/headless local HTTP proxy workflow validated in `v0.11.0-rc.1`, `v0.11.0-rc.2`, and `v0.11.0-rc.3` to the recommended stable release.

This release is suitable for daily use after the RC validation cycle. If you enable local proxy takeover, cc-switch-web will temporarily modify supported client configuration files for Claude Code, Codex, Gemini CLI, and OpenCode. Stopping the proxy or restoring takeover attempts to restore those client configs.

## Highlights

- Web/headless local HTTP proxy for Claude Code, Codex/OpenAI-compatible, Gemini, and OpenCode client requests
- One local proxy port with provider-aware routing and provider switching through cc-switch-web
- Settings controls for proxy start, stop, status, test, auto-start, timeout knobs, recent logs, and per-client takeover/restore
- Live client takeover and restore for Claude, Codex, Gemini, and experimental OpenCode configuration
- Adapter normalization for Claude, Codex/OpenAI-compatible, Gemini, and OpenCode provider endpoints
- Web/headless APIs for proxy status, config, settings, test, logs, takeover, restore, and stale takeover recovery
- OMO MCP and Skills entry points restored through OpenCode shared storage

## Fixes Since 0.10.1

- Hardened proxy startup and takeover UX after real server validation
- Show clearer occupied-port and already-running proxy errors
- Prevent duplicate takeover requests and duplicate/stuck takeover toasts
- Preserve Claude provider `env` objects and normalize root-level `ANTHROPIC_*` fragments
- Fix default Anthropic Skills repository scan paths and migrate existing empty scan-path entries
- Redact sensitive values from proxy logs more aggressively
- Keep restore status synchronized while proxy takeover is running
- Fix Release workflow release-note handling
- Stabilize the App integration test around lazy-loaded dialogs

## Upgrade Notes

- Users on `v0.10.1` can upgrade directly to `v0.11.0`.
- Users on any `v0.11.0-rc.*` build should upgrade to this stable release.
- Review proxy takeover settings before enabling them in shared or remote environments.
- Keep the proxy listen host at `127.0.0.1` unless you explicitly need LAN access and understand the exposure.

## Known Scope

- The local proxy is an application-level HTTP API proxy. It does not modify OS global proxy settings, PAC files, or Clash-style rules.
- Multi-provider failover queues, circuit breaker UI, usage/cost accounting, and cross-provider request/stream format conversion remain deferred to later releases.

## Validation

- `pnpm vitest run tests/integration/App.test.tsx`
- `pnpm vitest run`
