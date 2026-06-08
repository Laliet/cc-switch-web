# cc-switch-web v0.16.0 发布说明

v0.16.0 是 **Claude Desktop 增强与网络稳定性** 版本，重点补齐 Claude Desktop 供应商迁移、代理兼容、用量统计归因、WebDAV 同步安全提示和 GitHub 下载镜像配置。

## Claude Desktop

- 新增 Claude Desktop 面板，用于查看官方配置状态、导入 Claude 供应商并管理桌面端供应商。
- 支持从 Claude 供应商自动导入兼容配置，并按供应商能力选择 Direct 或 Proxy 模式。
- Claude Desktop Direct 模式同时兼容 `ANTHROPIC_AUTH_TOKEN` 与 `ANTHROPIC_API_KEY`。
- Claude Desktop Proxy 模式增强模型路由，自动处理 OpenAI 兼容接口中的 `tool_choice` 映射。
- 扩展 Claude Desktop 预设列表，覆盖更多 Anthropic、OpenAI 兼容和国内中转服务。

## 代理与用量

- 本地代理支持把供应商端点作为完整 URL 使用，适配 Vertex、Responses 等不应继续拼接请求路径的场景。
- Gemini 请求现在可以从 URI 中提取模型名，用量日志的模型归因更准确。
- 请求日志新增 session/conversation 识别，方便按会话回看代理调用。
- 流式解析与请求统计增强，提升用量 Dashboard 中按模型、供应商和数据来源展示的准确性。
- Claude Desktop 代理测试现在会按 Direct/Proxy 模式校验对应配置，而不是只走通用供应商校验。

## 设置与同步

- 设置页新增 Network 区域，可配置 GitHub ZIP 下载镜像，改善 Skills 仓库等资源下载不稳定的问题。
- WebDAV 下载远端快照前增加确认流程，应用远端配置前会展示本地备份 ID。
- WebDAV 快照预览新增配置版本、schema 版本等信息，并优化错误提示。
- 代理设置页继续补齐网络与接管相关配置，降低 Web/headless 环境下的配置歧义。

## 修复与测试

- 修复 Provider 卡片、Claude Desktop 面板、Network 设置、WebDAV 设置和用量 Dashboard 的回归用例。
- 增加代理 Web API、Claude Desktop 配置、用量类型、settings schema 和 API adapter 测试。
- 更新 README 截图与说明，使文档与 v0.15/v0.16 功能入口保持一致。

## 已知发布问题

- v0.16.0 首次 GitHub Actions 发布中，Windows job 在 MSI 已构建成功后继续尝试打 NSIS installer，下载 NSIS 依赖时遇到 `http status: 504`，导致 Windows 产物、server 二进制、`latest.json` 与 Docker 镜像未生成。
- 后续发布流程已调整为 Windows 只构建当前实际上传的 MSI 包，避免未使用的 NSIS installer 下载失败拖垮整条发布链路。
