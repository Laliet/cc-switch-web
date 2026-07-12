# cc-switch-web

> Web-based CC Switch for Claude Code, Codex, Gemini CLI, OpenCode & OMO.

<sub>🙏 This project is a fork of [farion1231/cc-switch](https://github.com/farion1231/cc-switch) by Jason Young. Thanks to the original author for the excellent work. This fork adds Web Server mode for cloud/headless deployment.</sub>

[![Release](https://img.shields.io/badge/Release-v0.19.1-ea7233?style=flat-square&logo=github)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![License](https://img.shields.io/github/license/Laliet/cc-switch-web?style=flat-square)](LICENSE)
[![Windows](https://img.shields.io/badge/Windows-0078D6?style=flat-square&logo=windows&logoColor=white)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![macOS](https://img.shields.io/badge/macOS-000000?style=flat-square&logo=apple&logoColor=white)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![Linux](https://img.shields.io/badge/Linux-FCC624?style=flat-square&logo=linux&logoColor=black)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![Docker](https://img.shields.io/badge/Docker-2496ED?style=flat-square&logo=docker&logoColor=white)](https://github.com/Laliet/cc-switch-web/pkgs/container/cc-switch-web)

**Cross-platform web-based All-in-One assistant for Claude Code, Codex, Gemini CLI, OpenCode & OMO**

English | [中文](README_ZH.md) | [Legal Notice](LEGAL_NOTICE.md) | [Changelog](CHANGELOG.md)

## About / 项目简介

**cc-switch-web** is a cross-platform web-based **CC Switch** for **Claude Code**, **Codex**, **Gemini CLI**, **OpenCode**, and **oh-my-opencode (OMO)**. It lets you switch providers, manage MCP servers, install skills, edit system prompts, and run the same workflow on desktop or headless cloud environments.

Whether you're working locally or in a headless cloud environment, cc-switch-web offers a seamless experience for:

- **One-click provider switching** between OpenAI-compatible API endpoints
- **Unified MCP server management** across Claude/Codex/Gemini/OpenCode
- **Skills marketplace** to browse and install Claude skills from GitHub
- **System prompt editor** with syntax highlighting
- **Configuration backup/restore** with version history
- **OpenCode and OMO configuration UI** with presets, model metadata, and OMO Slim support
- **Stream health checks** for validating provider streaming responses
- **Web server mode** for cloud/headless deployment with Basic Auth

---

## Contact /联系

If you have any questions, you can contact me here https://linux.do/t/topic/1217545

## Screenshots

| Provider Switching + Local Routing | Usage Dashboard |
| --- | --- |
| ![Provider Switching + Local Routing](assets/screenshots/v0.15.0-main.png) | ![Usage Dashboard](assets/screenshots/v0.15.0-usage-dashboard.png) |

| MCP Server Management | Prompt Management |
| --- | --- |
| ![MCP Server Management](assets/screenshots/v0.15.0-mcp.png) | ![Prompt Management](assets/screenshots/v0.15.0-prompts.png) |

| Skills Marketplace | Add Provider |
| --- | --- |
| ![Skills Marketplace](assets/screenshots/v0.15.0-skills.png) | ![Add Provider](assets/screenshots/v0.15.0-add-provider.png) |

| Configure Provider |
| --- |
| ![Configure Provider](assets/screenshots/v0.15.0-config-provider.png) |

---

## Features

### Core Features

- **Multi-Provider Management**: Switch between different AI providers (OpenAI-compatible endpoints) with one click
- **Unified MCP Management**: Configure Model Context Protocol servers across Claude/Codex/Gemini/OpenCode
- **Skills Marketplace**: Browse and install Claude skills from GitHub repositories
- **Prompt Management**: Create and manage system prompts with a built-in CodeMirror editor
- **OpenCode Provider Presets**: Select AI SDK packages, import preset models, fetch model lists, and edit model variants/options
- **OMO / OMO Slim UI**: Manage oh-my-opencode and oh-my-opencode-slim configuration with structured fields

### Extended Features

- **Backup Auto-failover**: Automatically switch to backup providers when primary fails
- **Stream Health Check**: Test streaming responses and classify common provider errors
- **Import/Export**: Backup and restore all configurations with version history
- **Cross-platform**: Available for Windows, macOS, Linux (desktop) and Web/Docker (server)

---

## Quick Start

### Option 1: Web Server Mode (Recommended)

Recommended: Use Web Server Mode for headless/cloud deployments and remote access.

Lightweight web server for headless environments. Access via browser, no GUI dependencies.

#### Method A: Prebuilt Binary (Recommended)

Download precompiled server binary—no compilation required:

| Architecture              | Download                                                                                                                           |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| **Linux x86_64 (glibc)**  | [cc-switch-server-linux-x86_64](https://github.com/Laliet/cc-switch-web/releases/download/v0.19.1/cc-switch-server-linux-x86_64)   |
| **Linux aarch64 (glibc)** | [cc-switch-server-linux-aarch64](https://github.com/Laliet/cc-switch-web/releases/download/v0.19.1/cc-switch-server-linux-aarch64) |

Release page: [v0.19.1 downloads](https://github.com/Laliet/cc-switch-web/releases/tag/v0.19.1)

> **Note (glibc)**: Binaries are built on Ubuntu 22.04 (glibc baseline).  
> If you see `GLIBC_2.xx not found`, use Docker or build from source.  
> Check your glibc with `ldd --version`.

**One-Line Deploy**:

```bash
curl -fsSL https://raw.githubusercontent.com/Laliet/cc-switch-web/main/scripts/deploy-web.sh | bash -s -- --prebuilt
```

**Quick fixes**:

- `GLIBC_2.xx not found`: use Docker (`ghcr.io/laliet/cc-switch-web:latest`) or build from source.
- Need container-first deployment: run `docker run -p 3000:3000 ghcr.io/laliet/cc-switch-web:latest`.
- Windows + WSL shared configs: Settings now provides a one-click WSL template path filler in Advanced tab.

**Advanced options**:

```bash
# Custom install directory and port
INSTALL_DIR=/opt/cc-switch PORT=8080 curl -fsSL https://raw.githubusercontent.com/Laliet/cc-switch-web/main/scripts/deploy-web.sh | bash -s -- --prebuilt

# Create systemd service for auto-start
CREATE_SERVICE=1 curl -fsSL https://raw.githubusercontent.com/Laliet/cc-switch-web/main/scripts/deploy-web.sh | bash -s -- --prebuilt
```

#### Method B: Docker Container

Docker image published to GitHub Container Registry (ghcr.io):

```bash
docker run -p 3000:3000 ghcr.io/laliet/cc-switch-web:latest
```

> ⚠️ **Note**: Docker image name must be **lowercase** (`laliet`, not `Laliet`)

**Advanced Docker options**:

```bash
# Use the deploy script (custom port/version/data dir/background)
./scripts/docker-deploy.sh -p 8080 --data-dir /opt/cc-switch-data -d

# Build locally (optional)
docker build -t cc-switch-web .
docker run -p 3000:3000 cc-switch-web
```

#### Method C: Build from Source

Dependencies: `libssl-dev`, `pkg-config`, Rust 1.78+, pnpm (no WebKit/GTK needed)

```bash
# 1. Clone and install dependencies
git clone https://github.com/Laliet/cc-switch-web.git
cd cc-switch-web
pnpm install

# 2. Build web assets
pnpm build:web

# 3. Build and run server
cd src-tauri
cargo build --release --features web-server --example server
HOST=0.0.0.0 PORT=3000 ./target/release/examples/server
```

### Web Server Login

- **Username**: `admin`
- **Password**: Auto-generated on first run, stored in `~/.cc-switch/web_password`
- **CORS**: Same-origin by default; set `CORS_ALLOW_ORIGINS=https://your-domain.com` for cross-origin (`CORS_ALLOW_ORIGINS="*"` is ignored). For LAN/private origins, enable `ALLOW_LAN_CORS=1` (or `CC_SWITCH_LAN_CORS=1`) to auto-allow
- **Note**: Web mode doesn't support native file pickers—enter paths manually

### Security

**Authentication**:

- Basic Auth is required for all API requests
- Browser will prompt for credentials (username/password)
- CSRF token is automatically injected and validated for non-GET requests

**Security Headers**:

- HSTS (HTTP Strict Transport Security) enabled by default
- X-Frame-Options: DENY (prevents clickjacking)
- X-Content-Type-Options: nosniff
- Referrer-Policy: no-referrer

**Best Practices**:

- Deploy behind a reverse proxy with TLS in production
- Set `ALLOW_HTTP_BASIC_OVER_HTTP=1` only if you understand the risks
- Keep `~/.cc-switch/web_password` file secure (mode 0600)

**Environment Variables**:
| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Server port | 3000 |
| `HOST` | Bind address | 127.0.0.1 |
| `ENABLE_HSTS` | Enable HSTS header | true |
| `CORS_ALLOW_ORIGINS` | Allowed origins (comma-separated) | (same-origin) |
| `CORS_ALLOW_CREDENTIALS` | Allow credentials in CORS | false |
| `ALLOW_LAN_CORS` | Auto-allow private LAN origins for CORS | false |
| `CC_SWITCH_LAN_CORS` | Auto-set when LAN CORS auto-allow is enabled | (unset) |
| `ALLOW_HTTP_BASIC_OVER_HTTP` | Suppress HTTP warning | false |
| `WEB_CSRF_TOKEN` | Override CSRF token | (auto-generated) |

### Option 2: Desktop Application (GUI)

Full-featured desktop app with graphical interface, built with Tauri.

| Platform    | Download                                                                                                                                           | Description                              |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| **Windows** | [CC-Switch-v0.19.1-Windows.msi](https://github.com/Laliet/cc-switch-web/releases/download/v0.19.1/CC-Switch-v0.19.1-Windows.msi)                   | Installer                                |
|             | [CC-Switch-v0.19.1-Windows-Portable.zip](https://github.com/Laliet/cc-switch-web/releases/download/v0.19.1/CC-Switch-v0.19.1-Windows-Portable.zip) | Portable (no install)                    |
| **macOS**   | [CC-Switch-v0.19.1-macOS.zip](https://github.com/Laliet/cc-switch-web/releases/download/v0.19.1/CC-Switch-v0.19.1-macOS.zip)                       | Universal binary (Intel + Apple Silicon) |
| **Linux**   | [CC-Switch-v0.19.1-Linux.AppImage](https://github.com/Laliet/cc-switch-web/releases/download/v0.19.1/CC-Switch-v0.19.1-Linux.AppImage)             | AppImage                                 |
|             | [CC-Switch-v0.19.1-Linux.deb](https://github.com/Laliet/cc-switch-web/releases/download/v0.19.1/CC-Switch-v0.19.1-Linux.deb)                       | Debian/Ubuntu package                    |

**macOS Note**: If you see "damaged" warning, run: `xattr -cr "/Applications/CC Switch.app"`

**Linux AppImage**: Make executable first: `chmod +x CC-Switch-*.AppImage`

**Linux One-Line Install** (recommended):

```bash
curl -fsSL https://raw.githubusercontent.com/Laliet/cc-switch-web/main/scripts/install.sh | bash
```

This script will:

- Auto-detect your architecture (x86_64/aarch64)
- Download the latest AppImage release
- Verify SHA256 checksum (if available)
- Install to `~/.local/bin/ccswitch` (user) or `/usr/local/bin/ccswitch` (root)
- Create desktop entry and application icon

**Advanced options**:

```bash
# Install a specific release version
VERSION=v0.19.1 curl -fsSL https://...install.sh | bash

# Skip checksum verification
NO_CHECKSUM=1 curl -fsSL https://...install.sh | bash
```

---

## Usage Guide

### 1. Adding a Provider

1. Launch CC-Switch and select your target app (Claude Code / Codex / Gemini / OpenCode / OMO)
2. Click **"Add Provider"** button
3. Choose a preset (e.g., OpenRouter, DeepSeek, GLM) or select "Custom"
4. Fill in:
   - **Name**: Display name for this provider
   - **Base URL**: API endpoint (e.g., `https://api.openrouter.ai/v1`)
   - **API Key**: Your API key for this provider
   - **Model** (optional): Specific model to use
5. Click **Save**

### 2. Switching Providers

- Click the **"Enable"** button on any provider card to activate it
- The active provider will be written to your CLI's config file immediately
- Use system tray menu for quick switching without opening the app

### 3. Managing MCP Servers

1. Go to **MCP** tab
2. Click **"Add Server"** to configure a new MCP server
3. Choose transport type: `stdio`, `http`, or `sse`
4. For stdio servers, provide the command and arguments
5. Enable/disable servers with the toggle switch

### 4. Installing Skills (Claude only)

1. Go to **Skills** tab
2. Browse available skills from configured repositories
3. Click **"Install"** to add a skill to `~/.claude/skills/`
4. Manage installed skills and add custom repositories

### 5. System Prompts

1. Go to **Prompts** tab
2. Create new prompts or edit existing ones
3. Enable a prompt to write it to the CLI's prompt file:
   - Claude: `~/.claude/CLAUDE.md`
   - Codex: `~/.codex/AGENTS.md`
   - Gemini: `~/.gemini/GEMINI.md`

---

## Configuration Files

CC-Switch manages these configuration files:

| App             | Config Files                                                                  |
| --------------- | ----------------------------------------------------------------------------- |
| **Claude Code** | `~/.claude.json` (MCP), `~/.claude/settings.json`, `~/.claude/CLAUDE.md`      |
| **Codex**       | `~/.codex/auth.json`, `~/.codex/config.toml`, `~/.codex/AGENTS.md`            |
| **Gemini**      | `~/.gemini/.env`, `~/.gemini/settings.json`, `~/.gemini/GEMINI.md`            |
| **OpenCode**    | `~/.config/opencode/opencode.json`                                            |
| **OMO**         | OMO / OMO Slim profiles and plugins under the shared OpenCode config location |

CC-Switch's runtime store: `~/.cc-switch/cc-switch.db`

Legacy import/export snapshot: `~/.cc-switch/config.json`. On startup, an
existing snapshot is imported into SQLite when the database is empty; after that
SQLite is the authoritative store for providers, MCP servers, prompts, skills,
proxy settings, provider health, request logs, failover queue entries, and
compatible JSON snapshots used by import/export flows.

---

## Development

```bash
# Install dependencies
pnpm install

# Run desktop app in dev mode
pnpm tauri dev

# Run only the frontend dev server
pnpm dev:renderer

# Build desktop app
pnpm tauri build

# Build web assets only
pnpm build:web

# Run tests
pnpm test:unit
```

---

## Tech Stack

- **Frontend**: React 18, TypeScript, Vite, Tailwind CSS, TanStack Query, Radix UI, CodeMirror
- **Backend**: Rust, Tauri 2.x, Axum (web server mode), tower-http
- **Tooling**: pnpm, Vitest, MSW

---

## What's New

> Current release: [v0.19.1](https://github.com/Laliet/cc-switch-web/releases/tag/v0.19.1)<br>
> `v0.19.1` is a release-quality patch for the v0.19 feature line.

### v0.19.1 - Release Quality Patch

- Keeps the v0.19.0 runtime behavior unchanged
- Fixes the Skills update result sorting implementation so the source passes Clippy with warnings denied
- Release notes: [v0.19.1](docs/release-note-v0.19.1-zh.md)

### v0.19.0 - Data Safety + Relay Workflow Completion

- Adds native SQLite SQL backup/restore, integrity validation, rollback, backup management, and scheduled retention
- Introduces WebDAV v2 with `manifest.json`, `db.sql`, `skills.zip`, checksums, compatibility validation, rollback, and change-triggered synchronization
- Adds native quota/balance queries for common relay providers without requiring user-authored JavaScript
- Completes Universal Provider, MCP import, Skills update/catalog, and per-app proxy settings workflows
- Adds a server-host Session Manager for Claude, Codex, Gemini, and OpenCode with search, messages, outline, deletion, and copied resume commands
- Release notes: [v0.19.0](docs/release-note-v0.19.0-zh.md)

### v0.15.0 - Local Routing + Claude Desktop Alignment

- Adds the stable Local Routing and Claude Desktop alignment release line
- Improves Usage Dashboard first-load behavior with automatic Recent data range selection when Today has no requests
- Adds the `All time` usage range and clarifies all-time data source statistics
- Adds usage data extent APIs for desktop and Web/headless mode
- Fixes Web-mode unmatched `/api/*` routes so they return JSON 404 responses instead of SPA HTML fallback
- Adds clearer frontend API failure toasts and inline Usage Dashboard error states
- Release notes: [v0.15.0](docs/release-note-v0.15.0-zh.md)

### v0.14.1 - Usage Dashboard Patch

- Fixes Usage Dashboard auto-refresh so relative time ranges recompute on each refetch
- Fixes request logs pagination reset when changing the global app or time range
- Fixes short historical trend queries that only have daily rollup data
- Tightens model-pricing matching so broad prefixes such as `gpt-4` do not misprice `gpt-4o`
- Release notes: [v0.14.1](docs/release-note-v0.14.1-zh.md)

### v0.14.0 - Usage Dashboard Prerelease

- Adds a full Usage Dashboard for proxy-backed requests, with cost summary, token breakdown, app split, trend chart, and refresh controls
- Adds searchable/paginated request logs with per-request token, latency, status, streaming, and cost detail
- Adds Provider and Model usage statistics plus a Dashboard pricing editor for model cost configuration
- Connects pricing updates to historical zero-cost proxy logs so newly configured model prices can backfill costs
- Adds Claude, Codex, and Gemini session log import with incremental sync and cross-source de-duplication
- Adds desktop and Web/headless Usage APIs under Tauri commands and `/api/usage/*`
- Release notes: [v0.14.0](docs/release-note-v0.14.0-zh.md)

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) and [v0.19.1 release notes](docs/release-note-v0.19.1-zh.md) - Current release: **v0.19.1**

---

## Credits

This project is a fork of **[cc-switch](https://github.com/farion1231/cc-switch)** by Jason Young (farion1231). We sincerely thank the original author for creating such an excellent foundation. Without the upstream project's pioneering work, cc-switch-web would not exist.

The upstream Tauri desktop app unified provider switching, MCP management, skills, and prompts with strong i18n and safety. cc-switch-web adds web/server runtime, CORS controls, Basic Auth, more templates, and documentation for cloud/headless deployment.

---

## Legal & Compliance Summary

> [!WARNING]
> This project is provided for learning, research, and community communication only. Please carefully evaluate whether your intended use complies with applicable laws, regulations, platform rules, and third-party service terms before use.
>
> By using this project, you agree to assess and bear the risks arising from your own configuration, deployment, and use. To the maximum extent permitted by applicable law, the project is provided on an **"AS IS"** basis, without any express or implied warranty.
>
> **This notice does not mean all legal risks can be fully excluded or waived.** Liabilities that cannot be excluded or limited under applicable law shall still be governed by the applicable law.

- **Allowed scope**: learning, research, self-hosting experimentation, and community communication.
- **Prohibited uses**: unlawful activity, infringement, unauthorized data collection, bypassing platform/service restrictions, evading rate limits or access controls, abusing other people's accounts / API keys / credentials, or violating third-party terms.
- **Third-party terms prevail**: when using services or tools from **OpenAI, Anthropic, Google Gemini, OpenCode, OMO**, cloud vendors, hosting providers, or any other third party, you must independently review and comply with their applicable terms, policies, and usage rules. If this project's documentation conflicts with those rules, **applicable law and the third party's binding terms prevail**.
- **Further notice**: see [LEGAL_NOTICE.md](LEGAL_NOTICE.md). The open-source license text is in [LICENSE](LICENSE), with supplementary notice in [LICENSE_NOTICE.md](LICENSE_NOTICE.md).

---

## License

MIT License — See [LICENSE](LICENSE) for details.
