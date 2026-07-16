# cc-switch-web

> 面向 Claude Code、Codex、Gemini CLI、OpenCode、OpenClaw 与 OMO 的 Web 版 CC Switch。

<sub>🙏 本项目是 [farion1231/cc-switch](https://github.com/farion1231/cc-switch)（Jason Young）的 fork 版本。感谢原作者的出色工作。本 fork 添加了 Web 服务器模式，支持云端/无头部署。</sub>

[![Release](https://img.shields.io/badge/Release-v0.21.0-ea7233?style=flat-square&logo=github)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![License](https://img.shields.io/github/license/Laliet/cc-switch-web?style=flat-square)](LICENSE)
[![Windows](https://img.shields.io/badge/Windows-0078D6?style=flat-square&logo=windows&logoColor=white)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![macOS](https://img.shields.io/badge/macOS-000000?style=flat-square&logo=apple&logoColor=white)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![Linux](https://img.shields.io/badge/Linux-FCC624?style=flat-square&logo=linux&logoColor=black)](https://github.com/Laliet/cc-switch-web/releases/latest)
[![Docker](https://img.shields.io/badge/Docker-2496ED?style=flat-square&logo=docker&logoColor=white)](https://github.com/Laliet/cc-switch-web/pkgs/container/cc-switch-web)

**面向 Claude Code / Codex / Gemini CLI / OpenCode / OpenClaw / OMO 的跨平台 Web 版一站式助手**

[English](README.md) | 中文 | [法律声明](LEGAL_NOTICE.md) | [更新日志](CHANGELOG.md)

## 项目简介

**cc-switch-web** 是一个面向 **Claude Code**、**Codex**、**Gemini CLI**、**OpenCode**、**OpenClaw** 和 **oh-my-opencode（OMO）** 的跨平台 Web 版 **CC Switch**。它支持通过桌面应用或带认证的无头 Web 控制台管理供应商和主机侧配置。

无论你是在本地开发还是在无图形界面的云端环境，cc-switch-web 都能提供流畅的体验：

- **一键切换供应商** — 支持 OpenAI 兼容 API 端点
- **统一 MCP 管理** — 跨 Claude/Codex/Gemini/OpenCode 统一管理
- **技能市场** — 从 GitHub 浏览并安装 Claude 技能
- **提示词编辑器** — 内置语法高亮
- **配置备份/恢复** — 支持版本历史
- **OpenCode 与 OMO 配置 UI** — 支持供应商预设、模型元数据和 OMO Slim
- **OpenClaw 配置中心** — 支持模型、Agents defaults、Environment、Tools、外部 Provider 对账、Workspace、每日记忆和会话
- **流式健康检查与历史** — 验证供应商的流式响应并保留诊断记录
- **Web 服务器模式** — 支持 Basic Auth，适用于云端/无头部署

---

## 界面展示

| 供应商切换与 Local Routing | 用量 Dashboard |
| --- | --- |
| ![供应商切换与 Local Routing](assets/screenshots/v0.15.0-main.png) | ![用量 Dashboard](assets/screenshots/v0.15.0-usage-dashboard.png) |

| MCP 服务器管理 | 提示词管理 |
| --- | --- |
| ![MCP 服务器管理](assets/screenshots/v0.15.0-mcp.png) | ![提示词管理](assets/screenshots/v0.15.0-prompts.png) |

| 技能商店 | 添加供应商 |
| --- | --- |
| ![技能商店](assets/screenshots/v0.15.0-skills.png) | ![添加供应商](assets/screenshots/v0.15.0-add-provider.png) |

| 配置供应商 |
| --- |
| ![配置供应商](assets/screenshots/v0.15.0-config-provider.png) |

---

## 功能特性

### 核心功能

- **多供应商管理**：一键切换不同 AI 供应商（OpenAI 兼容端点）
- **统一 MCP 管理**：跨 Claude/Codex/Gemini/OpenCode 配置 Model Context Protocol 服务器
- **技能市场**：从 GitHub 仓库浏览并安装 Claude 技能
- **提示词管理**：内置 CodeMirror 编辑器创建和管理系统提示词
- **OpenCode 供应商预设**：选择 AI SDK package、导入推荐模型、拉取模型列表、编辑模型变体和选项
- **OMO / OMO Slim UI**：用结构化字段管理 oh-my-opencode 与 oh-my-opencode-slim 配置
- **OpenClaw 管理**：管理增量供应商、模型、Agents defaults、Environment、Tools、原始 JSON5、Workspace 与每日记忆，并提供 ETag 冲突保护和备份

### 扩展功能

- **备用供应商自动切换**：主供应商失败时自动切换到备用
- **流式健康检查**：测试流式响应并识别常见供应商错误
- **会话管理**：分页浏览服务器主机上的 Claude、Codex、Gemini、OpenCode 与 OpenClaw 会话，并安全读取或删除
- **导入/导出**：备份和恢复所有配置，支持版本历史
- **跨平台**：支持 Windows、macOS、Linux（桌面版）和 Web/Docker（服务器版）

---

## 快速开始

### 方式一：Web 服务器模式（推荐）

推荐优先使用 Web 服务器模式，尤其适合云端/无头部署与远程访问。

轻量级 Web 服务器，适用于无图形界面的服务器环境。通过浏览器访问，无需 GUI 依赖。

#### 方法 A：预编译二进制（推荐）

下载预编译的服务器二进制，无需编译：

| 架构                      | 下载链接                                                                                                                           |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| **Linux x86_64 (glibc)**  | [cc-switch-server-linux-x86_64](https://github.com/Laliet/cc-switch-web/releases/download/v0.21.0/cc-switch-server-linux-x86_64)   |
| **Linux aarch64 (glibc)** | [cc-switch-server-linux-aarch64](https://github.com/Laliet/cc-switch-web/releases/download/v0.21.0/cc-switch-server-linux-aarch64) |

发布页：[v0.21.0 下载](https://github.com/Laliet/cc-switch-web/releases/tag/v0.21.0)

> **glibc 说明**：预编译二进制基于 Ubuntu 22.04 构建。  
> 如果报 `GLIBC_2.xx not found`，请改用 Docker 或源码构建。  
> 可用 `ldd --version` 查看 glibc 版本。

**一键部署**：

```bash
curl -fsSL https://raw.githubusercontent.com/Laliet/cc-switch-web/main/scripts/deploy-web.sh | bash -s -- --prebuilt
```

**常见问题速查**：

- 报 `GLIBC_2.xx not found`：建议使用 Docker（`ghcr.io/laliet/cc-switch-web:latest`）或源码构建。
- 想直接容器化运行：使用 `docker run -p 3000:3000 ghcr.io/laliet/cc-switch-web:latest`。
- Windows + WSL 共用配置：设置页支持一键填充 WSL 模板路径（高级设置页中的“填充 WSL 模板路径”）。

**高级选项**：

```bash
# 自定义安装目录和端口
INSTALL_DIR=/opt/cc-switch PORT=8080 curl -fsSL https://raw.githubusercontent.com/Laliet/cc-switch-web/main/scripts/deploy-web.sh | bash -s -- --prebuilt

# 创建 systemd 服务（开机自启）
CREATE_SERVICE=1 curl -fsSL https://raw.githubusercontent.com/Laliet/cc-switch-web/main/scripts/deploy-web.sh | bash -s -- --prebuilt
```

#### 方法 B：Docker 容器

Docker 镜像发布到 GitHub Container Registry (ghcr.io)：

```bash
docker run -p 3000:3000 ghcr.io/laliet/cc-switch-web:latest
```

> ⚠️ **注意**：Docker 镜像名必须**全小写**（`laliet`，不是 `Laliet`）

**Docker 高级选项**：

```bash
# 使用部署脚本（自定义端口/版本/数据目录、可后台运行）
./scripts/docker-deploy.sh -p 8080 --data-dir /opt/cc-switch-data -d

# 本地构建镜像（可选）
docker build -t cc-switch-web .
docker run -p 3000:3000 cc-switch-web
```

#### 方法 C：源码构建

依赖：`libssl-dev`、`pkg-config`、Rust 1.78+、pnpm（无需 WebKit/GTK）

```bash
# 1. 克隆并安装依赖
git clone https://github.com/Laliet/cc-switch-web.git
cd cc-switch-web
pnpm install

# 2. 构建 Web 资源
pnpm build:web

# 3. 构建并运行服务器
cd src-tauri
cargo build --release --no-default-features --features web-server --example server
HOST=0.0.0.0 PORT=3000 ./target/release/examples/server
```

### Web 服务器登录

- **用户名**：`admin`
- **密码**：首次运行自动生成，保存在 `~/.cc-switch/web_password`
- **跨域设置**：默认同源；需跨域请设置 `CORS_ALLOW_ORIGINS=https://your-domain.com`（`CORS_ALLOW_ORIGINS="*"` 会被忽略）；局域网/私有来源可通过 `ALLOW_LAN_CORS=1`（或 `CC_SWITCH_LAN_CORS=1`）自动放行
- **注意**：Web 模式不支持原生文件选择器，请手动输入路径

### Web 运行边界

Web 模式管理的是 **cc-switch-server 所在主机** 的文件与进程，不是浏览器所在设备。运行时能力接口 `GET /api/capabilities` 会驱动前端隐藏或降级原生专属操作。

| 应用 | Web/headless 支持范围 |
| --- | --- |
| Claude、Codex、Gemini | 服务器主机上的供应商、MCP、提示词、Skills、用量、会话与 Local Routing |
| OpenCode | 增量供应商，以及 MCP、提示词、Skills、用量、会话与 Local Routing |
| OpenClaw | 增量供应商、结构化/原始配置、外部 Provider 对账、白名单 Workspace/每日记忆编辑与会话 |
| OMO / OMO Slim | Provider profile，并复用 OpenCode MCP/Skills 存储；不支持代理接管 |
| Claude Desktop | 仅在受支持的 macOS/Windows 服务器主机管理 Provider/Local Routing profile；不支持 MCP、提示词、Skills、用量与会话 |

原生文件对话框、系统托盘、应用更新、便携模式设置、环境管理和原生端点测试仍仅限桌面端。OpenClaw Local Routing、MCP、提示词、Skills 与用量继续保持不支持，因为上游 v3.15.0 也没有提供这些 OpenClaw 集成。

### 安全

**认证**：

- 所有 API 请求都需要 Basic Auth
- 浏览器会弹出用户名/密码提示
- 对非 GET 请求会自动注入并校验 CSRF Token

**安全响应头**：

- 默认启用 HSTS（HTTP Strict Transport Security）
- X-Frame-Options: DENY（防止点击劫持）
- X-Content-Type-Options: nosniff
- Referrer-Policy: no-referrer

**最佳实践**：

- 生产环境建议在反向代理后部署，并启用 TLS
- 仅在充分理解风险的情况下设置 `ALLOW_HTTP_BASIC_OVER_HTTP=1` 以抑制 HTTP 警告
- 请妥善保护 `~/.cc-switch/web_password` 文件（权限建议 0600）

**环境变量**：
| 变量 | 说明 | 默认值 |
|------|------|--------|
| `PORT` | 服务端口 | 3000 |
| `HOST` | 监听地址 | 127.0.0.1 |
| `ENABLE_HSTS` | 是否启用 HSTS 响应头 | true |
| `CORS_ALLOW_ORIGINS` | 允许的来源（逗号分隔） | （同源） |
| `CORS_ALLOW_CREDENTIALS` | 是否允许 CORS 携带凭据 | false |
| `ALLOW_LAN_CORS` | 自动放行局域网私有来源 | false |
| `CC_SWITCH_LAN_CORS` | 局域网自动放行启用时自动写入 | （未设置） |
| `ALLOW_HTTP_BASIC_OVER_HTTP` | 抑制 HTTP 警告 | false |
| `WEB_CSRF_TOKEN` | 覆盖 CSRF Token | （自动生成） |

### 方式二：桌面应用（GUI）

功能完整的桌面应用，带图形界面，基于 Tauri 构建。

| 平台        | 下载链接                                                                                                                                           | 说明                                |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------- |
| **Windows** | [CC-Switch-v0.21.0-Windows.msi](https://github.com/Laliet/cc-switch-web/releases/download/v0.21.0/CC-Switch-v0.21.0-Windows.msi)                   | 安装版                              |
|             | [CC-Switch-v0.21.0-Windows-Portable.zip](https://github.com/Laliet/cc-switch-web/releases/download/v0.21.0/CC-Switch-v0.21.0-Windows-Portable.zip) | 绿色版（免安装）                    |
| **macOS**   | [CC-Switch-v0.21.0-macOS.zip](https://github.com/Laliet/cc-switch-web/releases/download/v0.21.0/CC-Switch-v0.21.0-macOS.zip)                       | 通用二进制（Intel + Apple Silicon） |
| **Linux**   | [CC-Switch-v0.21.0-Linux.AppImage](https://github.com/Laliet/cc-switch-web/releases/download/v0.21.0/CC-Switch-v0.21.0-Linux.AppImage)             | AppImage                            |
|             | [CC-Switch-v0.21.0-Linux.deb](https://github.com/Laliet/cc-switch-web/releases/download/v0.21.0/CC-Switch-v0.21.0-Linux.deb)                       | Debian/Ubuntu 包                    |

**macOS 提示**：如遇"已损坏"警告，在终端执行：`xattr -cr "/Applications/CC Switch.app"`

**Linux AppImage**：先添加执行权限：`chmod +x CC-Switch-*.AppImage`

**Linux 一键安装**（推荐）：

```bash
curl -fsSL https://raw.githubusercontent.com/Laliet/cc-switch-web/main/scripts/install.sh | bash
```

该脚本会：

- 自动检测系统架构（x86_64/aarch64）
- 下载最新版 AppImage
- 校验 SHA256（如有校验文件）
- 安装到 `~/.local/bin/ccswitch`（普通用户）或 `/usr/local/bin/ccswitch`（root）
- 创建桌面快捷方式和应用图标

**高级选项**：

```bash
# 安装指定版本
VERSION=v0.21.0 curl -fsSL https://...install.sh | bash

# 跳过校验
NO_CHECKSUM=1 curl -fsSL https://...install.sh | bash
```

---

## 使用指南

### 1. 添加供应商

1. 启动 CC-Switch，选择目标应用（Claude Code / Codex / Gemini / OpenCode / OMO）
2. 点击 **"添加供应商"** 按钮
3. 选择预设（如 OpenRouter、DeepSeek、智谱 GLM）或选择"自定义"
4. 填写配置：
   - **名称**：供应商显示名称
   - **Base URL**：API 端点（如 `https://api.openrouter.ai/v1`）
   - **API Key**：该供应商的 API 密钥
   - **模型**（可选）：指定使用的模型
5. 点击 **保存**

### 2. 切换供应商

- 点击任意供应商卡片上的 **"启用"** 按钮即可激活
- 激活的供应商配置会立即写入 CLI 配置文件
- 使用系统托盘菜单可快速切换，无需打开应用窗口

### 3. 管理 MCP 服务器

1. 进入 **MCP** 标签页
2. 点击 **"添加服务器"** 配置新的 MCP 服务器
3. 选择传输类型：`stdio`、`http` 或 `sse`
4. 对于 stdio 服务器，提供命令和参数
5. 使用开关启用/禁用服务器

### 4. 安装技能（仅 Claude）

1. 进入 **技能** 标签页
2. 浏览已配置仓库中的可用技能
3. 点击 **"安装"** 将技能添加到 `~/.claude/skills/`
4. 管理已安装的技能，可添加自定义仓库

### 5. 系统提示词

1. 进入 **提示词** 标签页
2. 创建新提示词或编辑现有提示词
3. 启用提示词后会写入对应 CLI 的提示词文件：
   - Claude: `~/.claude/CLAUDE.md`
   - Codex: `~/.codex/AGENTS.md`
   - Gemini: `~/.gemini/GEMINI.md`

---

## 配置文件

CC-Switch 管理以下配置文件：

| 应用            | 配置文件                                                                  |
| --------------- | ------------------------------------------------------------------------- |
| **Claude Code** | `~/.claude.json`（MCP）、`~/.claude/settings.json`、`~/.claude/CLAUDE.md` |
| **Codex**       | `~/.codex/auth.json`、`~/.codex/config.toml`、`~/.codex/AGENTS.md`        |
| **Gemini**      | `~/.gemini/.env`、`~/.gemini/settings.json`、`~/.gemini/GEMINI.md`        |
| **OpenCode**    | `~/.config/opencode/opencode.json`                                        |
| **OpenClaw**    | `~/.openclaw/openclaw.json`、`~/.openclaw/workspace/`、`~/.openclaw/agents/` |
| **OMO**         | OpenCode 共享配置目录下的 OMO / OMO Slim profile 与插件配置               |

CC-Switch 运行时主存储：`~/.cc-switch/cc-switch.db`

旧版导入/导出快照：`~/.cc-switch/config.json`。启动时，如果 SQLite 数据库为空且
已存在旧快照，会先导入数据库；此后 providers、MCP servers、prompts、skills、
proxy settings、provider health、request logs、failover queue，以及导入/导出
兼容快照均以 SQLite 为权威存储。

v0.21.0 继续使用 SQLite **Schema v7**，其中包含持久化 Stream Check 历史。WebDAV v2 的新快照写入 `v2/db-v7/<profile>/`，仍可读取和恢复 v0.19.1 的 `db-v6` 主快照与历史备份，并继续把 v0.18 旧 JSON 快照作为只读迁移来源。运行时日志、用量、健康状态、会话同步状态和 Stream Check 历史不会上传；恢复时会保留服务器本地已有的诊断历史。

---

## 开发

```bash
# 安装依赖
pnpm install

# 开发模式运行桌面应用
pnpm tauri dev

# 仅运行前端开发服务器
pnpm dev:renderer

# 构建桌面应用
pnpm tauri build

# 仅构建 Web 资源
pnpm build:web

# 运行测试
pnpm test:unit
```

---

## 技术栈

- **前端**：React 18、TypeScript、Vite、Tailwind CSS、TanStack Query、Radix UI、CodeMirror
- **后端**：Rust、Tauri 2.x、Axum（Web 服务器模式）、tower-http
- **工具链**：pnpm、Vitest、MSW

---

## 更新内容

> 当前版本：[v0.21.0](https://github.com/Laliet/cc-switch-web/releases/tag/v0.21.0)<br>
> `v0.21.0` 完成 OpenClaw 第二阶段、全局 Session 搜索、已安装 Skills 发现和 Provider 路由状态展示。

### v0.21.0 - OpenClaw 第二阶段

- 补齐 OpenClaw 模型、Agents defaults、Environment、Tools/Profile 结构化配置和高级原始 JSON5 编辑
- 新增外部 Provider 对账，支持 ETag 绑定的预览/应用和幂等启动刷新
- 新增主机完整 Session 搜索及长会话消息/目录虚拟化
- 新增已安装 Skills 发现/导入，包含可信固定目录、冲突预览、明确覆盖和多 App 同步
- 新增 Daily Memory 搜索及带备份的 ETag 保护删除
- 合并上游 v3.15.0 全部 OpenClaw、Gemini 和 Codex 预设，同时保留本项目适合中国大陆网络的 Provider
- Provider 卡片展示故障转移优先级、代理实际路由、熔断状态/失败统计和最新健康检查时间
- 发布说明：[v0.21.0](docs/release-note-v0.21.0-zh.md)

### v0.20.0 - Web/Headless 与 OpenClaw 第一阶段

- 新增桌面/Web 运行时能力契约和明确的服务器主机边界
- 新增 OpenClaw 第一阶段：供应商/默认模型、Workspace/每日记忆和 Session Manager
- Schema 升级到 v7，持久化 Stream Check 配置与历史，并新增 Provider 额度摘要
- 增加真实 HTTP 代理 conformance、精确 usage 解析、稳定 Failover 顺序和 Claude Desktop 重启状态判断
- 加固 Session 分页/路径、Web 错误脱敏、WebDAV db-v6 回退与 manifest identity 校验
- OMO/OMO Slim 的 MCP 与 Skills 继续复用 OpenCode 存储，同时继续禁止 OMO 代理接管
- 上游 v3.15.0 对标基线已锁定在 [`docs/upstream-v3.15.0.lock`](docs/upstream-v3.15.0.lock)
- 发布说明：[v0.20.0](docs/release-note-v0.20.0-zh.md)

### v0.19.1 - 发布质量修复版

- 保持 v0.19.0 的运行时行为不变
- 修复 Skills 更新结果的排序实现，使源码在 warnings denied 条件下通过 Clippy
- 更新说明：[v0.19.1](docs/release-note-v0.19.1-zh.md)

### v0.19.0 - 数据安全与中转服务体验完善版

- 新增 SQLite 原生 SQL 备份恢复、完整性校验、失败回滚、备份管理和定时保留
- 新增 WebDAV v2：`manifest.json`、`db.sql`、`skills.zip`、哈希和兼容性校验、恢复回滚及变更触发同步
- 中转 Provider 额度和余额改为 Rust 原生查询，不再要求用户手写 JavaScript
- 补齐 Universal Provider、MCP 导入、Skills 更新/目录和每 App 独立代理参数闭环
- 新增 Claude、Codex、Gemini、OpenCode 服务器会话管理，支持搜索、消息、目录、删除和复制恢复命令
- 更新说明：[v0.19.0](docs/release-note-v0.19.0-zh.md)

### v0.15.0 - Local Routing + Claude Desktop 对齐版

- 发布 Local Routing + Claude Desktop 对齐正式版本
- 用量 Dashboard 首次打开时，如果 Today 没有请求但存在历史用量，会自动切到最新用量所在的 Recent data 窗口
- 时间范围新增 `All time`，并将 Data sources 明确为全量来源统计
- 新增用量数据范围接口，Web/headless 端提供 `/api/usage/data-extent`
- 修复 Web 模式不存在的 `/api/*` 路径返回 HTML 200 的问题，现在返回 JSON 404
- 增加全局 API 失败 toast 与 Usage Dashboard 内联错误态
- 更新说明：[v0.15.0](docs/release-note-v0.15.0-zh.md)

### v0.14.1 - Usage Dashboard 修复版

- 修复 Usage Dashboard 自动刷新时相对时间范围不会前进的问题
- 修复 request logs 切换全局 App 或时间范围后未重置页码的问题
- 修复短时间范围只有 daily rollup 历史数据时趋势图为空的问题
- 收紧模型定价匹配，避免 `gpt-4` 错误匹配 `gpt-4o`
- 更新说明：[v0.14.1](docs/release-note-v0.14.1-zh.md)

### v0.14.0 - Usage Dashboard 预发布版

- 新增完整 Usage Dashboard：展示代理请求成本汇总、token 拆分、应用维度占比、趋势图与自动刷新
- 新增可搜索/分页的 request logs，并支持查看单次请求的 token、延迟、状态、流式信息和成本详情
- 新增 Provider / Model 使用统计，以及 Dashboard 内模型价格维护面板
- 模型价格更新后会尝试回填历史零成本代理日志，补齐 pricing/cost 闭环
- 新增 Claude、Codex、Gemini session log 导入，支持增量同步与跨来源去重
- 新增桌面端 Tauri commands 与 Web/headless `/api/usage/*` API
- 更新说明：[v0.14.0](docs/release-note-v0.14.0-zh.md)

---

## 更新日志

参见 [CHANGELOG.md](CHANGELOG.md) 与 [v0.21.0 发布说明](docs/release-note-v0.21.0-zh.md) - 当前版本：**v0.21.0**

---

## 致谢

本项目基于 Jason Young (farion1231) 的开源项目 **[cc-switch](https://github.com/farion1231/cc-switch)** 二次开发。衷心感谢原作者创建了如此优秀的开源项目，为本项目奠定了坚实基础。没有上游项目的开拓性工作，就不会有 cc-switch-web 的诞生。

上游 Tauri 桌面应用统一了供应商切换、MCP 管理、技能和提示词功能，具备完善的国际化和安全特性。cc-switch-web 在此基础上增加了 Web/服务器运行模式、CORS 控制、Basic Auth、更多模板，以及云端/无头部署文档。

---

## 法律与合规摘要

> [!WARNING]
> 本项目仅供学习、研究与社区交流使用。请在使用前仔细甄别并确认你的具体用途是否符合所在地适用法律法规、平台规则以及第三方服务条款。
>
> 使用本项目即表示你理解并同意：你应自行评估、决定并承担因配置、部署、调用或其他使用行为产生的相关风险。在适用法律允许的最大范围内，本项目按 **“现状（AS IS）”** 提供，不提供任何明示或默示保证。
>
> **但这并不意味着可以完全排除或豁免一切法律责任。** 凡适用法律规定不得排除、限制或免除的责任，仍应依法承担。

- **允许范围**：学习、研究、自托管实验与社区交流。
- **禁止用途**：任何违法违规行为、侵权行为、未获授权的数据抓取或数据处理、绕过平台/服务限制、规避访问控制或限流、滥用他人账号 / API Key / 凭证，或违反第三方条款的行为。
- **第三方条款优先**：当你使用 **OpenAI、Anthropic、Google Gemini、OpenCode、OMO** 以及云厂商、托管平台或其他第三方服务时，你必须自行阅读并遵守其适用条款、政策和使用规则。如本项目文档与前述规则冲突，**以适用法律及第三方具有约束力的条款为准**。
- **进一步说明**：详见 [LEGAL_NOTICE.md](LEGAL_NOTICE.md)。开源许可文本见 [LICENSE](LICENSE)，补充说明见 [LICENSE_NOTICE.md](LICENSE_NOTICE.md)。

---

## 许可证

MIT License — 详见 [LICENSE](LICENSE)
