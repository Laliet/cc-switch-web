# CC-Switch Web Server 使用指南

## 运行模型

cc-switch-web 提供两种运行形态：

1. Tauri 桌面应用，管理本机配置并提供系统托盘、原生对话框和应用更新等能力。
2. Axum Web Server，通过浏览器管理 cc-switch-server 所在主机，适用于 Linux 服务器、Docker 和无头环境。

Web 模式不是浏览器设备的远程桌面。Provider、Session、Workspace、CLI 配置和进程状态都属于服务器主机。前端通过 `GET /api/capabilities` 获取运行时能力，不可用操作应隐藏或返回带错误码的 501。

## v0.20.0 能力范围

| 应用 | Provider | MCP / Prompt / Skills | Usage | Session | Local Routing |
| --- | --- | --- | --- | --- | --- |
| Claude | 支持 | 支持 | 支持 | 支持 | 支持 |
| Codex | 支持 | 支持 | 支持 | 支持 | 支持 |
| Gemini | 支持 | 支持 | 支持 | 支持 | 支持 |
| OpenCode | 增量模式 | 支持 | 支持 | 支持 | 支持 |
| OpenClaw | 增量模式 | 第一阶段不支持 | 第一阶段不支持 | 支持 | 第一阶段不支持 |
| OMO / OMO Slim | 支持 profile | MCP/Skills 复用 OpenCode；Prompt 不支持 | 不支持 | 不支持 | 不支持 |
| Claude Desktop | 支持 profile | 不支持 | 不支持 | 不支持 | 仅受支持的 macOS/Windows 主机 |

Web 模式不提供原生文件/目录选择器、系统托盘、应用更新、便携模式设置、环境管理、终端拉起或原生端点测试。服务器 Session 的恢复操作只复制命令，不会启动服务器终端。

请求中的 App 名称未知或参数格式错误时返回 HTTP 400。App 名称已知、但对应功能在上表中不可用时返回 HTTP 501，并带 `<feature>_<app>_unavailable` 格式的稳定 `code`；前端应以 `/api/capabilities` 为准，不依赖试错调用。

## 快速开始

### 预编译服务器

当前稳定版为 v0.20.0。可从 Release 页面下载对应架构的 `cc-switch-server-linux-*`，或运行：

```bash
curl -fsSL https://raw.githubusercontent.com/Laliet/cc-switch-web/main/scripts/deploy-web.sh | bash -s -- --prebuilt
```

预编译 GNU 版本以 Ubuntu 22.04 为基线。遇到 glibc 不兼容时优先使用 Docker、musl 变体或源码构建。

### Docker

```bash
docker run --name cc-switch-web \
  -p 127.0.0.1:3000:3000 \
  -v cc-switch-data:/root/.cc-switch \
  ghcr.io/laliet/cc-switch-web:latest
```

需要管理服务器主机上的 Claude/Codex/Gemini/OpenCode/OpenClaw 配置时，还应把相应配置目录挂载到容器中。挂载路径决定 Web UI 实际管理的数据；不要把宿主机根目录整体暴露给容器。

### 源码构建

依赖：Rust、Node.js、pnpm、`pkg-config` 和 OpenSSL 开发包。纯 Web 构建不需要 Tauri 的 WebKit/GTK 桌面依赖。

```bash
git clone https://github.com/Laliet/cc-switch-web.git
cd cc-switch-web
pnpm install
pnpm build:web

cd src-tauri
cargo build --release --no-default-features --features web-server --example server
HOST=127.0.0.1 PORT=3000 ./target/release/examples/server
```

`HOST` 默认是 `127.0.0.1`，`PORT` 默认是 `3000`。绑定非回环地址时，服务默认拒绝通过裸 HTTP 暴露 Basic Auth；生产环境应使用 TLS 反向代理。

## systemd 与反向代理

将构建产物安装为 `/usr/local/bin/cc-switch-server` 后，可创建：

```ini
[Unit]
Description=CC-Switch Web Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=cc-switch
WorkingDirectory=/var/lib/cc-switch
Environment=HOST=127.0.0.1
Environment=PORT=3000
ExecStart=/usr/local/bin/cc-switch-server
Restart=on-failure
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

建议由 Nginx/Caddy 在 HTTPS 上公开服务：

```nginx
server {
    listen 443 ssl;
    server_name cc-switch.example.com;

    ssl_certificate /etc/letsencrypt/live/cc-switch.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/cc-switch.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto https;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

若明确接受风险并直接监听内网 HTTP，需设置 `ALLOW_HTTP_BASIC_OVER_HTTP=1`。不要在公网裸 HTTP 上传 API Key、OAuth token 或 Workspace 内容。

## 认证与安全

- 首次启动使用用户名 `admin`，随机密码保存在 `~/.cc-switch/web_password`，文件权限应为 0600。
- 可在设置中轮换用户名和密码；服务不会把密码内容打印到控制台。
- 所有 API 需要 Basic Auth；非 GET/HEAD 请求还需要 `X-CSRF-Token`。
- 前端自动获取并注入 CSRF token。手工调用可先访问 `GET /api/system/csrf-token`。
- 默认同源。跨域仅允许 `CORS_ALLOW_ORIGINS` 中的精确 Origin；`*` 不被接受。
- 绑定局域网 IP 时可用 `ALLOW_LAN_CORS=1` 启用私有来源规则。
- 外网监听且未显式设置时，Usage Script 出口策略会调整为 `strict`。
- HSTS、`X-Frame-Options: DENY`、`X-Content-Type-Options: nosniff` 和 `Referrer-Policy: no-referrer` 默认启用。
- 内部 5xx、认证上游正文、服务器绝对路径和敏感凭据字段不会写入 Web 错误响应；公开的 501 能力边界仍保留说明。

常用环境变量：

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `HOST` | `127.0.0.1` | 监听 IP |
| `PORT` | `3000` | 监听端口 |
| `WEB_CSRF_TOKEN` | 自动生成 | 固定 CSRF token |
| `ENABLE_HSTS` | `true` | HSTS 响应头 |
| `CORS_ALLOW_ORIGINS` | 同源 | 逗号分隔的精确 Origin |
| `CORS_ALLOW_CREDENTIALS` | `false` | 跨域携带凭据 |
| `ALLOW_LAN_CORS` | `false` | 私有局域网来源规则 |
| `ALLOW_HTTP_BASIC_OVER_HTTP` | `false` | 明确接受非回环裸 HTTP 风险 |
| `USAGE_SCRIPT_EGRESS_POLICY` | 按监听地址决定 | Usage Script 网络出口策略 |
| `WEBDAV_AUTO_SYNC_INTERVAL_SECS` | `300`，最小 60 | WebDAV 自动同步周期 |

## OpenClaw 与 Workspace

OpenClaw 第一阶段管理服务器主机的 `~/.openclaw/openclaw.json`、`workspace/` 和 `agents/`：

- Provider 使用增量写入，不删除未知 Provider。
- 支持默认模型的读取、设置和清除。
- Workspace 只允许固定文件名，拒绝符号链接和根目录逃逸。
- 单文件最大 1 MiB；覆盖必须提供 ETag；写入前创建备份，每文件最多保留 20 个。
- 每日记忆仅接受有效的 `YYYY-MM-DD`，对应 `memory/YYYY-MM-DD.md`。
- Session 扫描不跟随 agent、sessions 目录或 JSONL 文件符号链接。

这些都是服务器主机文件。若 cc-switch-server 运行在 Docker 中，必须显式挂载希望管理的 OpenClaw 目录。

## Session Manager

- 支持 Claude、Codex、Gemini、OpenCode 和 OpenClaw。
- `GET /api/sessions/page` 使用不透明的 `scannedAt:offset` 游标，单页最多 200 条。
- 游标绑定扫描快照；后台刷新后旧游标返回过期错误，客户端应从第一页重新加载。
- 每个 Provider 独立缓存；文件树变化后只重扫对应 Provider。
- 所有消息读取和删除都限制在已知会话根目录，并拒绝符号链接逃逸。

## 数据库与 WebDAV

运行时权威存储是 `~/.cc-switch/cc-switch.db`，`config.json` 仅用于旧版导入/兼容导出。

v0.20.0 使用 SQLite Schema v7：

- v6 首次启动自动迁移到 v7。
- v7 新增 Stream Check 历史与索引。
- v0.19.1 无法打开已升级的 v7 数据库；降级前必须恢复 v6 备份。

WebDAV v2：

- 新上传位置：`<remoteDir>/v2/db-v7/<profile>/`。
- 快照由 `manifest.json`、`db.sql` 和 `skills.zip` 组成，校验协议、Schema、大小、SHA256 和 snapshot identity。
- v0.20.0 可回退读取/恢复 `db-v6` 主快照与历史备份，并继续读取 v0.18 JSON 快照。
- 用量、请求日志、健康状态、会话同步和 Stream Check 历史属于本机数据，不上传；恢复时保留本地诊断历史。
- `managed_auth_accounts` 表及其中的 Claude/Codex/Gemini OAuth access/refresh token 不进入 WebDAV 快照；恢复快照不会覆盖本机托管账号。

升级前建议在设置中创建 SQLite 备份，并避免桌面端和 Web Server 同时写同一个数据库。

## 主要 API

```text
GET    /api/capabilities

GET    /api/providers/:app
POST   /api/providers/:app
PUT    /api/providers/:app/:id
DELETE /api/providers/:app/:id
POST   /api/providers/:app/:id/switch

GET    /api/openclaw/status
GET    /api/openclaw/providers
GET    /api/openclaw/default-model
PUT    /api/openclaw/default-model
DELETE /api/openclaw/default-model

GET    /api/workspace/files
GET    /api/workspace/files/:name
PUT    /api/workspace/files/:name
GET    /api/workspace/files/:name/backups
POST   /api/workspace/files/:name/restore
GET    /api/workspace/memory
GET    /api/workspace/memory/:date
PUT    /api/workspace/memory/:date

GET    /api/sessions/page
POST   /api/sessions/messages
POST   /api/sessions/delete-batch

GET    /api/stream-check/config
PUT    /api/stream-check/config
POST   /api/stream-check/providers/:id       body: { "appType": "claude" }
POST   /api/stream-check/all                 body: { "appType": "claude", "proxyTargetsOnly": false }
GET    /api/stream-check/logs
GET    /api/stream-check/logs/latest?appType=claude

GET    /api/subscriptions/quota?provider=claude&accountId=<id>&force=false

GET    /api/proxy/status
GET    /api/proxy/failover/:app
PUT    /api/proxy/failover/:app
PUT    /api/proxy/takeover/:app

POST   /api/webdav/snapshot/upload
POST   /api/webdav/snapshot/download
GET    /api/webdav/snapshot/preview
GET    /api/webdav/backups
POST   /api/webdav/backups/restore
```

`:app` 包括 `claude`、`claude-desktop`、`codex`、`gemini`、`opencode`、`openclaw`、`omo` 和 `omo-slim`，但每个 API 仍受能力矩阵限制。OMO/OMO Slim 不可用于代理 `bindApp` 或 takeover。

Stream Check 的标准单 Provider 路由是 `/providers/:id`，应用类型放在 JSON body；旧版 `/providers/:app/:id` 保留为兼容路由。OpenClaw、OMO 和 OMO Slim 的代理配置、takeover、failover、熔断重置及 Stream Check 会返回带 `*_unavailable` code 的 HTTP 501，不会静默执行桌面逻辑。Provider 卡片读取 `/stream-check/logs/latest`，展示最近状态、响应时间和错误分类。

## 故障排查

### 只能本机访问

默认绑定 `127.0.0.1`。推荐保持该设置并使用反向代理或 SSH 隧道：

```bash
ssh -L 3000:127.0.0.1:3000 user@server
```

### 公开监听启动失败

这是 Basic Auth 裸 HTTP 保护。配置 TLS 反代，或在可信内网显式设置：

```bash
ALLOW_HTTP_BASIC_OVER_HTTP=1 HOST=0.0.0.0 PORT=3000 ./cc-switch-server
```

### GLIBC 版本不兼容

使用 Docker、musl 预编译或在目标系统源码构建。可用 `ldd --version` 检查 glibc。

### Web 管理不到 CLI/OpenClaw 配置

确认服务用户的 HOME、目录覆盖配置和 Docker volume。浏览器本地的 `~/.claude` 不会自动映射到远端服务器。

### Session 翻页返回 cursor expired

会话目录在翻页期间发生变化或用户主动刷新。重新请求第一页即可。

## 开发验证

```bash
pnpm typecheck
pnpm test:unit

cd src-tauri
cargo test --no-default-features --features web-server,test-hooks
```

完整发布构建会产生较大的 `src-tauri/target`；按项目 `AGENTS.md` 要求，验证结束后应删除构建产物。
