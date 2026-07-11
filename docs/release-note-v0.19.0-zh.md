# cc-switch-web v0.19.0 发布说明

v0.19.0 是 **数据安全与中转服务体验完善** 正式版本。本版本继续对标上游 `farion1231/cc-switch v3.15.0` 的数据结构、业务流程和实现逻辑，并针对 Web/服务器运行形态补齐安全边界。

## SQLite 原生备份与恢复

- 使用完整 SQL 导出和导入 SQLite 数据库，不再依赖不完整的业务对象快照。
- 导入前自动创建安全备份，在临时数据库执行迁移、Schema 和完整性校验，成功后通过 SQLite Backup API 替换正式数据库。
- 支持手动创建、列表、恢复、重命名和删除备份，并支持定时备份及保留数量设置。
- 恢复过程发生错误时保留原数据库，避免半恢复状态破坏现有配置。

## WebDAV v2 与自动同步

- 新快照协议使用 `manifest.json`、`db.sql` 和 `skills.zip`。
- 上传和恢复时校验 SHA256、文件大小、协议版本和数据库兼容版本，manifest 最后上传，避免读取未完成快照。
- 数据库或 Skills 恢复失败时自动回滚，并继续支持读取 0.18.0 的旧 JSON 快照；新上传统一使用 v2。
- SQLite 持久配置变化后触发 1 秒防抖自动同步，最长等待 10 秒；导入期间抑制同步，并排除用量、日志、健康和会话运行数据。
- 设置页展示最后同步时间、状态和错误。

## 中转 Provider 体验

- Rust 原生支持 Kimi For Coding、智谱 GLM Coding Plan、MiniMax 国内/国际站额度查询。
- Rust 原生支持 DeepSeek、StepFun、SiliconFlow 国内/国际站、OpenRouter 和 Novita AI 余额查询。
- 已识别的中转无需用户手写 JavaScript 用量脚本，仍保留自定义脚本能力。
- Provider 表单区分阻塞错误与非阻塞警告；名称、端点或 Key 尚未补齐时可在明确确认后保存，结构错误仍会阻止提交。

## Universal Provider、MCP 与 Skills

- Universal Provider 增加 NewAPI、自定义网关、自动同步、保存并同步、复制、同步确认、后端配置预览和 API Key 遮盖。
- MCP 可从服务器上的 Claude、Codex、Gemini、OpenCode 配置导入；相同 ID 合并 App 开关，冲突时报告并保留现有配置。
- Skills 支持更新检测、单项/批量更新、`skills.sh` 搜索安装、跨 App 统一管理和精确来源记录。
- Skills 更新使用同文件系统 staging、更新前备份和失败回滚，成功后重新同步所有启用 App。

## 每 App 独立代理参数

- Claude、Codex、Gemini、OpenCode 分别保存和应用重试、首字节超时、流式空闲超时及非流式超时。
- 每 App 分别设置失败阈值、恢复成功阈值、恢复等待时间、错误率和 `circuit_min_requests`。
- 数据库升级到 Schema v6，旧全局值自动迁移到各 App，避免现有参数丢失。
- 修复代理接管期间切换 Provider 时把本地代理地址回填到持久 Provider 的问题，避免上游地址被污染并形成自指向转发环路。

## Session Manager Web MVP

- 扫描服务器主机上的 Claude、Codex、Gemini 和 OpenCode 会话，支持列表、应用筛选和全文搜索。
- 支持会话详情、消息内容、用户消息目录、单项删除和批量删除。
- 恢复操作只复制命令；Web 端不会尝试拉起服务器终端。
- 界面明确标注项目目录和会话源路径均属于服务器主机。
- 普通文件读取/删除必须位于对应应用的会话根目录内；OpenCode SQLite 仅允许访问预期数据库。
- 修复 Web Session API 尾斜杠不匹配导致的列表和单项删除 404，并完成桌面三栏及移动端纵向布局验收。
- 移动端应用切换器改为容器内横向滚动，页面宽度不再被完整标签组撑出视口。

## 兼容性说明

- 数据库 Schema 版本：`v6`。
- WebDAV v2 目录包含数据库兼容版本，v0.19.0 使用 `db-v6`。
- 仍可读取 v0.18.0 WebDAV JSON 快照；恢复后后续上传会转换为 v2。
- 官方 Claude/Codex/Gemini 账号导入逻辑未改变，中转 Provider 仍是 Web/服务器部署的主要验证路径。
