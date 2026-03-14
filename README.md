# cc-switch-web

<sub>🙏 This project is a fork of [farion1231/cc-switch](https://github.com/farion1231/cc-switch) by Jason Young. Thanks to the original author for the excellent work. This fork adds Web Server mode for cloud/headless deployment.</sub>

[![Release](https://img.shields.io/github/v/release/Laliet/cc-switch-web?style=flat-square&logo=github&label=Release)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![License](https://img.shields.io/github/license/Laliet/cc-switch-web?style=flat-square)](LICENSE)
[![Windows](https://img.shields.io/badge/Windows-0078D6?style=flat-square&logo=windows&logoColor=white)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![macOS](https://img.shields.io/badge/macOS-000000?style=flat-square&logo=apple&logoColor=white)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![Linux](https://img.shields.io/badge/Linux-FCC624?style=flat-square&logo=linux&logoColor=black)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![Docker](https://img.shields.io/badge/Docker-2496ED?style=flat-square&logo=docker&logoColor=white)](https://github.com/Laliet/cc-switch-web/pkgs/container/cc-switch-web)

**Cross-platform web-based All-in-One assistant for Claude Code, Codex, Gemini CLI, OpenCode & OMO**

English | [中文](README_ZH.md) | [Changelog](CHANGELOG.md)

> Recommended stable release: [v0.9.2](https://github.com/Laliet/cc-switch-web/releases/tag/v0.9.2)  
> `v0.10.0` is currently a prerelease for testing only

## About / 项目简介

**cc-switch-web** is a cross-platform web-based All-in-One assistant for **Claude Code**, **Codex**, **Gemini CLI**, **OpenCode**, and **oh-my-opencode (OMO)**, based on **CC Switch**. It provides both a desktop application and a web server mode for managing AI CLI providers, MCP servers, skills, and system prompts.

This project is a free and open-source project shared for informational and community use only. Any legal issues arising from the use of this project are not related to the project developer.

Whether you're working locally or in a headless cloud environment, cc-switch-web offers a seamless experience for:

- **One-click provider switching** between OpenAI-compatible API endpoints
- **Unified MCP server management** across Claude/Codex/Gemini/OpenCode
- **Skills marketplace** to browse and install Claude skills from GitHub
- **System prompt editor** with syntax highlighting
- **Configuration backup/restore** with version history
- **Web server mode** for cloud/headless deployment with Basic Auth

---

## Contact /联系

If you have any questions, you can contact me here https://linux.do/t/topic/1217545

## What's New

### v0.9.2 - Current Stable Release

- Recommended for daily use and production
- Download stable builds from: [v0.9.2](https://github.com/Laliet/cc-switch-web/releases/tag/v0.9.2)

### v0.10.0 - Current Prerelease

- Add OpenCode provider management and automatic OpenCode config writing
- Add oh-my-opencode (OMO) configuration management and plugin sync support
- Allow updating Web-mode login username/password directly from Settings
- Improve rollback safety for OMO/OpenCode linked writes and current-provider edits
- `v0.10.0` is currently a prerelease and still needs more cross-machine validation

### v0.8.0 - Previous Stable Baseline

- Web mode defaults to same-origin CORS
- `ALLOW_LAN_CORS=1` / `CC_SWITCH_LAN_CORS=1` for private LAN auto-allow
- Safer default behavior for remote deployments

## Screenshots

![Main Interface](pic/界面展示.png)
_Main Interface_

![Prompt Management](pic/提示词管理展示.png)
_Prompt Management_

![MCP Server Management](pic/MCP服务器管理展示.png)
_MCP Server Management_

![Skills Marketplace](pic/skills商店管理展示.png)
_Skills Marketplace_

![Extended Provider List](pic/扩展的中转服务商列表.png)
_Extended Provider List_

![Configure Provider](pic/配置中转服务商展示.png)
_Configure Provider_

---

## Features

### Core Features

- **Multi-Provider Management**: Switch between different AI providers (OpenAI-compatible endpoints) with one click
- **Unified MCP Management**: Configure Model Context Protocol servers across Claude/Codex/Gemini/OpenCode
- **Skills Marketplace**: Browse and install Claude skills from GitHub repositories
- **Prompt Management**: Create and manage system prompts with a built-in CodeMirror editor

### Extended Features

- **Backup Auto-failover**: Automatically switch to backup providers when primary fails
- **Import/Export**: Backup and restore all configurations with version history
- **Cross-platform**: Available for Windows, macOS, Linux (desktop) and Web/Docker (server)

---

## Quick Start

### Option 1: Web Server Mode (Recommended)

Recommended: Use Web Server Mode for headless/cloud deployments and remote access.

Lightweight web server for headless environments. Access via browser, no GUI dependencies.

#### Method A: Prebuilt Binary (Recommended)

Download precompiled server binary—no compilation required:

| Architecture              | Download                                                                                                                          |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| **Linux x86_64 (glibc)**  | [cc-switch-server-linux-x86_64](https://github.com/Laliet/cc-switch-web/releases/download/v0.9.2/cc-switch-server-linux-x86_64)   |
| **Linux aarch64 (glibc)** | [cc-switch-server-linux-aarch64](https://github.com/Laliet/cc-switch-web/releases/download/v0.9.2/cc-switch-server-linux-aarch64) |

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

| Platform    | Download                                                                                                                                        | Description                              |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| **Windows** | [CC-Switch-v0.9.2-Windows.msi](https://github.com/Laliet/cc-switch-web/releases/download/v0.9.2/CC-Switch-v0.9.2-Windows.msi)                   | Installer (current stable)               |
|             | [CC-Switch-v0.9.2-Windows-Portable.zip](https://github.com/Laliet/cc-switch-web/releases/download/v0.9.2/CC-Switch-v0.9.2-Windows-Portable.zip) | Portable (no install)                    |
| **macOS**   | [CC-Switch-v0.9.2-macOS.zip](https://github.com/Laliet/cc-switch-web/releases/download/v0.9.2/CC-Switch-v0.9.2-macOS.zip)                       | Universal binary (Intel + Apple Silicon) |
| **Linux**   | [CC-Switch-v0.9.2-Linux.AppImage](https://github.com/Laliet/cc-switch-web/releases/download/v0.9.2/CC-Switch-v0.9.2-Linux.AppImage)             | AppImage (current stable)                |
|             | [CC-Switch-v0.9.2-Linux.deb](https://github.com/Laliet/cc-switch-web/releases/download/v0.9.2/CC-Switch-v0.9.2-Linux.deb)                       | Debian/Ubuntu package                    |

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
# Install current stable version
VERSION=v0.9.2 curl -fsSL https://...install.sh | bash

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

| App             | Config Files                                      |
| --------------- | ------------------------------------------------- |
| **Claude Code** | `~/.claude.json` (MCP), `~/.claude/settings.json` |
| **Codex**       | `~/.codex/auth.json`, `~/.codex/config.toml`      |
| **Gemini**      | `~/.gemini/.env`, `~/.gemini/settings.json`       |

CC-Switch's own config: `~/.cc-switch/config.json`

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

## Changelog

See [CHANGELOG.md](CHANGELOG.md) — Recommended stable version: **v0.9.2**

---

## Credits

This project is a fork of **[cc-switch](https://github.com/farion1231/cc-switch)** by Jason Young (farion1231). We sincerely thank the original author for creating such an excellent foundation. Without the upstream project's pioneering work, cc-switch-web would not exist.

The upstream Tauri desktop app unified provider switching, MCP management, skills, and prompts with strong i18n and safety. cc-switch-web adds web/server runtime, CORS controls, Basic Auth, more templates, and documentation for cloud/headless deployment.

---

## License

MIT License — See [LICENSE](LICENSE) for details.
