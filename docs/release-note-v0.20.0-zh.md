# cc-switch-web v0.20.0 发布说明

> 2026-07-15 正式发布，当前稳定版为 v0.20.0。

v0.20.0 聚焦三个目标：补齐 Web/headless 能力闭环，完成 OpenClaw 第一阶段，并提高代理诊断与故障切换的可靠性。核心逻辑继续对标 `farion1231/cc-switch v3.15.0`，仅在 Web/服务器运行边界上保留必要差异。对标版本的 commit、tag 和镜像来源已锁定在 [`docs/upstream-v3.15.0.lock`](upstream-v3.15.0.lock)。

## 运行时能力与 Web/headless 边界

- 新增统一运行时能力契约，桌面端和 Web 端明确声明可用 App、全局功能和每 App 功能。
- App 已知但功能不可用时返回带稳定错误码的 HTTP 501；未知 App 名称仍返回 HTTP 400。
- Web UI 根据能力隐藏或降级文件对话框、托盘、应用更新、便携模式、环境管理和原生端点测试等桌面专属操作。
- 所有 Web 配置、Session、Workspace 和 Claude Desktop 状态均指向 cc-switch-server 所在主机，不代表浏览器设备。
- Claude Desktop profile 仅在受支持的 macOS/Windows 主机工作；Linux 会返回明确的不支持原因。

## OpenClaw 第一阶段

- 支持增量管理 OpenClaw Provider，不覆盖配置中由 OpenClaw 或用户维护的其他 Provider。
- 支持读取/设置/清除默认模型，并提供 Provider、默认模型和配置健康状态。
- 新增 Workspace 编辑器，只允许 `AGENTS.md`、`SOUL.md`、`USER.md`、`IDENTITY.md`、`TOOLS.md`、`MEMORY.md`、`HEARTBEAT.md`、`BOOTSTRAP.md` 和 `BOOT.md`。
- Workspace 写入支持 ETag 并发冲突保护、写前备份、恢复和每文件最多 20 个备份；单文件上限为 1 MiB。
- 支持 `memory/YYYY-MM-DD.md` 每日记忆的列表、读取和写入。
- Session Manager 新增 OpenClaw agent 会话扫描、消息查看和安全删除。
- 第一阶段不包含 OpenClaw Local Routing、MCP、Prompt、Skills 或 Usage。

## Session Manager 性能与安全

- 新增分页 Session API；游标绑定扫描快照，缓存更新后旧游标会明确过期，避免翻页重复或漏项。
- 按 Provider 缓存扫描结果，并通过稳定文件树指纹只重扫发生变化的 Provider。
- 指纹扫描不跟随符号链接，限制最大深度和条目数；无法完整覆盖时强制重扫。
- 会话读取和删除拒绝源文件符号链接、父目录符号链接逃逸及 Provider 根目录外路径。
- Web 错误不再返回服务器绝对路径。

## 代理协议与故障切换

- 增加真实 HTTP conformance，覆盖 Anthropic Messages、OpenAI/Codex Responses、Gemini Native、Vertex 完整 URL，以及 Claude Desktop 的 OpenAI Chat、Responses 与 Gemini 双向转换。
- 校验 method、URL、认证头、`tool_choice`、JSON Schema 下划线字段、`session_id` 和 `prompt_cache_key` 请求关联。
- Usage 解析区分“没有 usage”“usage 字段不完整”和“明确上报全零”，避免把未知用量误记为零。
- SQLite Failover 队列保持配置顺序；`backupCurrent` 去重后作为尾部回退，没有有效队列时再使用当前/备用 Provider。
- 流式首字节超时测试使用真实分段响应，验证响应头到达后 body 延迟仍可正确触发超时逻辑。
- Claude Desktop 需要重启的判断改为比较 CC Switch 最近写入时间与 Claude Desktop 主进程启动时间。

## 诊断、额度与 OMO

- SQLite Schema v7 新增 Stream Check 历史表和索引，支持持久化测试配置、筛选历史和每 Provider 最新结果。
- 新增 Provider 订阅额度摘要和设置页汇总入口。
- OMO/OMO Slim 的 MCP 与 Skills 继续复用 OpenCode 存储；Prompt、Usage、Session 和 Local Routing 保持关闭。
- 代理配置和 takeover 路由继续拒绝 OMO/OMO Slim，避免把 OMO profile 误当作独立代理客户端。

## WebDAV 与数据兼容

- 新上传的 WebDAV v2 快照写入 `v2/db-v7/<profile>/`。
- 预览、下载、历史列表和指定备份恢复会回退读取 v0.19.1 的 `db-v6` 主快照及历史目录。
- 继续支持读取 v0.18 的旧 JSON 快照；恢复后再次上传会转换为当前 v2/db-v7 格式。
- Manifest 会验证协议、Schema、artifact 大小/SHA256，以及 `snapshotId` 是否与 artifact 哈希一致。
- Stream Check 历史属于本机诊断数据，不上传到 WebDAV；恢复 db-v6/db-v7 快照时保留本地历史。

## Web 安全收口

- 5xx 响应不再返回内部错误原文；501 能力边界仍保留可操作的公开说明。
- 认证失败不再透传上游响应正文，配置错误不再暴露配置内容。
- 4xx 响应对绝对路径和敏感凭据字段进行兜底过滤。
- OpenClaw 写入结果仅返回备份标识；健康告警不回显解析器细节、Provider ID 或模型引用。
- Workspace、Session 和 Skill 错误不得泄露服务器路径或文件正文。

## 升级与降级

- v0.19.1 的 Schema v6 数据库会在首次启动 v0.20.0 时自动迁移到 Schema v7。
- 升级前建议创建 SQLite 备份，并确保桌面端与 cc-switch-server 没有同时写同一数据库。
- v0.19.1 不支持打开已经升级到 v7 的数据库。需要降级时，请先恢复升级前的 v6 数据库备份，不要直接复用 v7 文件。
- WebDAV db-v6 仅作为读取/恢复兼容源；v0.20.0 不会继续向 db-v6 路径写入。

## 已知边界

- Web 端不能操作浏览器设备上的文件、终端或桌面应用。
- OMO/OMO Slim 和 OpenClaw 不参与代理 takeover。
- Claude Desktop 的 MCP、Prompt、Skills、Usage 和 Session 不由该 profile 管理。
