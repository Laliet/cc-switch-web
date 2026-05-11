# cc-switch-web v0.11.0-rc.3

> A release candidate for desktop and Web/headless usage, with hardened local proxy, client takeover, and remote server workflows

**[中文更新说明 Chinese Documentation ->](release-note-v0.11.0-rc.3-zh.md)**

---

## Overview

`v0.11.0-rc.3` is the third release candidate before the stable cc-switch-web `0.11.0` release. It builds on `v0.11.0-rc.1` and `v0.11.0-rc.2`, then further hardens proxy startup failure handling, client takeover failure handling, running restore status synchronization, sensitive log redaction, and the GitHub Release workflow.

This release is suitable for upgrading from `v0.11.0-rc.1` or `v0.11.0-rc.2` for continued validation. When local proxy takeover is enabled, cc-switch-web temporarily modifies supported client configuration files for Claude Code, Codex, Gemini CLI, and OpenCode. Stopping the proxy or restoring takeover attempts to restore those client configs.

---

## Highlights

- Added Web/headless workflows for managing providers from browsers on cloud servers, remote hosts, and non-desktop environments.
- Added a local HTTP API proxy that routes Claude, Codex, Gemini, and OpenCode requests to matching providers.
- Added client takeover support that temporarily points supported clients at the local proxy.
- Added proxy status, runtime stats, recent request logs, active takeover targets, and per-client proxy tests.
- Added proxy failover support, including fallback on first-byte timeout when no response body has been sent.
- Improved settings, directory configuration, Web login, environment conflict warnings, and import/export workflows.

---

## RC.3 Fixes

- Fixed a bug where failed proxy startup could still persist `enabled=true` / `liveTakeoverActive=true`.
- Fixed partial client takeover during proxy startup so already-written clients are restored if a later takeover target fails.
- Fixed running takeover toggles so live client config is applied or restored before persisted settings are changed.
- Fixed running restore so `/api/proxy/status` no longer reports stale takeover state after real client config has been restored.
- Fixed possible sensitive query leakage in the recent log `error` field through upstream error strings containing `key`, `api_key`, `access_token`, `token`, and similar parameters.
- Fixed the Release workflow so it no longer overwrites manually written GitHub Release notes with a fixed body template.
- Fixed repeated proxy startup in the current process so the status and user-facing behavior are clearer.
- Fixed unclear errors when the configured proxy port is already used by another process.
- Fixed duplicate Claude entries in proxy takeover target lists.
- Fixed streaming first-byte timeout failover when no response body has been sent yet.

---

## Web Bundle Optimization

- Lazy-load settings, skills, provider add/edit forms, usage script, MCP, and prompt panels.
- Lazy-load the CodeMirror editor in Usage Script and provide a lightweight textarea fallback.
- Load Prettier only when the user clicks format.
- Split React, Radix, TanStack Query, i18n, icons, CodeMirror, and Prettier into vendor chunks for Web builds.
- Reduced the Web entry chunk from about `2.16 MB` to about `272 KB`; `pnpm build:web` no longer reports Vite large chunk warnings.

---

## Upgrade Notes

- Users on `v0.11.0-rc.1` or `v0.11.0-rc.2` should upgrade to this release candidate for continued validation.
- If proxy takeover was enabled before upgrading, open proxy settings after the upgrade and verify takeover state. Use restore takeover if needed.
- Gemini OAuth providers still do not support takeover; use a Gemini API key provider for Gemini takeover.
- OpenCode takeover remains experimental, so keeping a backup of the original config is recommended.

---

## Validation

The changes in this release were validated with:

- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `pnpm build:web`
- `cargo test --manifest-path src-tauri/Cargo.toml --features web-server --test proxy_web_api`
- `git diff --check`

The `proxy_web_api` coverage includes:

- Failed startup on an occupied port does not incorrectly persist enabled proxy config.
- Running restore synchronizes takeover status and active targets back to false/empty.
- Upstream error recent logs do not leak sensitive query values.

---

## Known Boundaries

- The local proxy is an application-level HTTP API proxy, not a transparent system proxy.
- It does not modify OS global proxy settings.
- PAC / Clash-style rules are not supported.
- OpenCode takeover remains experimental.
- Gemini OAuth provider takeover is intentionally unsupported; use a Gemini API key provider for takeover testing.
- Multi-provider failover queues, circuit breaker UI, and cross-provider request/stream format conversion are deferred to later releases.
