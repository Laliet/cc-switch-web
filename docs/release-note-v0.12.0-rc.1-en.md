# cc-switch-web v0.12.0-rc.1

> Release candidate for SQLite runtime storage migration and Web/headless alignment

**[中文更新说明 Chinese Documentation ->](release-note-v0.12.0-rc.1-zh.md)**

---

## Overview

`v0.12.0-rc.1` is the first release candidate for `0.12.0`. This release focuses on the runtime storage migration: cc-switch-web now uses SQLite as its authoritative runtime store, while continuing to align Web/headless proxy usage, failover, Universal Provider, and model pricing workflows.

The new runtime source of truth is:

```text
~/.cc-switch/cc-switch.db
```

The legacy `~/.cc-switch/config.json` file is retained as an import/export snapshot for compatibility, backup, and migration workflows, but it is no longer the authoritative runtime state.

---

## Highlights

- Migrated runtime state to a SQLite-backed `AppState`.
- Added and split the database layer into schema, migration, backup, and DAO modules.
- Moved core provider, MCP, prompt, skill, config import/export, and proxy config read/write paths to DB-backed APIs.
- Added startup migration from legacy `config.json` and legacy proxy settings into SQLite.
- Clarified `config.json` as a legacy import/export snapshot.
- Added database tables and DAOs for provider health, proxy request logs, usage daily rollups, failover queue, model pricing, and universal providers.
- Persisted proxy request logs and parsed response usage, cache tokens, streaming usage, first-token latency, and cost.
- Wired proxy failover to prefer the DB-backed failover queue.
- Added failover queue Tauri commands, Web API routes, and settings UI controls.
- Added Universal Provider typed models, DAO, service, Tauri commands, Web API routes, and a basic settings workflow.
- Added model pricing DAO/API/UI and expanded default model pricing seeds.
- Improved test home isolation for Web/headless mode so tests do not accidentally read the real account home.

---

## Storage Migration Notes

On first launch with this release:

1. If the SQLite database has not been migrated and the core tables are empty, cc-switch-web attempts to import legacy `~/.cc-switch/config.json`.
2. If the legacy JSON file does not exist, it falls back to default config and live config auto-import.
3. The imported state is written to `~/.cc-switch/cc-switch.db`.
4. Legacy proxy settings are imported into the DB-backed proxy config.
5. Future runtime reads and writes use SQLite as the source of truth.

Before testing the upgrade path, backing up these files is recommended:

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

## Validation

This release candidate was validated with:

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

The coverage includes:

- SQLite roundtrips for providers, MCP, prompts, skills, proxy config, and universal providers.
- Legacy JSON import/export snapshot boundaries.
- MCP sync and live config snapshot refresh after Codex provider switches.
- Proxy request logging, usage/cost calculation, and streaming usage updates.
- DB-backed failover queue selection and Web API management.
- Universal Provider API roundtrip, sync, and generated provider deletion.
- Path isolation under the `web-server` feature.

---

## Known Boundaries

- This is a `0.12.0` release candidate, not the final stable release.
- `config.json` is still used for legacy import/export and backup compatibility, but not as runtime source of truth.
- Universal Provider has a basic workflow, but not the full upstream-level advanced model mapping experience yet.
- Model pricing seeds have been expanded, but do not promise full coverage for every model from every provider; use the UI/API to maintain custom pricing.
- The local proxy is an application-level HTTP API proxy, not a transparent system proxy.
- Gemini OAuth providers still do not support proxy takeover; use a Gemini API key provider for Gemini takeover testing.
