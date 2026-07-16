# cc-switch-web v0.21.0 发布说明

> 2026-07-16 正式发布，当前稳定版为 v0.21.0。

v0.21.0 是 **OpenClaw 第二阶段** 版本。本次重点是完成 v0.20.0 已开始的工作流，不再引入新的半成品应用。核心逻辑继续对标 `farion1231/cc-switch v3.15.0` commit `9e3f1689038febb36da08993cd47281426b5dd7c`，基线记录在 [`upstream-v3.15.0.lock`](upstream-v3.15.0.lock)。

## OpenClaw 配置中心

- 新增模型目录、`agents.defaults`、Environment 和 Tools/Profile 结构化编辑器。
- 保留高级原始 JSON5 编辑入口，用于管理结构化表单尚未覆盖的字段。
- 结构化写入保留注释与未知字段，并使用原子替换、写前备份、SHA-256 ETag 和明确的 HTTP 409 冲突响应。
- 新增外部 Provider 发现、差异预览、选择性应用、幂等导入，以及对已标记为实时配置托管 Provider 的启动刷新。
- 实时 OpenClaw 文件中缺失 Provider 时，不会自动删除 Web 配置库中的记录。

## Session 与 Daily Memory

- Session 搜索下沉到服务端，覆盖主机完整快照，不再局限于前端首批加载的数据。
- 游标绑定 Provider 与搜索条件；浏览器会丢弃已经过期的慢查询响应。
- 长会话消息列表和用户消息目录使用虚拟化渲染，限制 DOM 数量。
- Daily Memory 支持搜索、完整查看、ETag 保护删除，并在删除前创建备份。

## Skills 已安装目录发现与导入

- 只扫描 Claude、Codex、Gemini、OpenCode、统一 Agent 存储和 CC Switch 存储等固定受支持目录。
- 浏览器只回传可信来源标签，不允许提交任意服务器路径。
- 导入前展示来源、目标、内容一致性、多来源冲突和目标应用。
- 冲突覆盖必须明确确认；统一存储使用原子替换，并按配置执行复制或符号链接同步，重复导入保持幂等。

## Provider 预设与路由状态

- 对上游 v3.15.0 的 OpenClaw、Gemini 和 Codex 预设逐项记录合并结果，同时保留本项目适合中国大陆网络的现有 Provider。
- Provider 卡片新增 P1/P2 故障转移顺序、代理实际路由目标、熔断状态、失败次数/窗口、最近失败时间和最新 Stream Check 时间。

## 已知边界

- Hermes Agent 的 Provider、MCP、Skills、记忆、会话和 Web UI 范围独立且较大，明确留给后续独立版本。
- OpenClaw Local Routing、MCP、Prompt、Skills 与 Usage 仍不支持；上游 v3.15.0 也没有提供这些 OpenClaw 集成。
- Web 模式管理的是服务器主机，不是浏览器设备上的文件或应用。
