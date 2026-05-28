# cc-switch-web v0.13.0

这是 v0.13.0 正式版。此版本将 v0.13.0-preview.1 验证过的 P1/P2 功能面作为正式版本发布：OMO Slim 完整 UI、上游新版 OMO 表单、OpenCode provider preset/NPM selector/model fetch，以及 stream health check。

## 主要更新

- 新增 OMO Slim 专用配置 UI，支持 Slim agents、隐藏分类、顶层选项、本地导入和 Slim 专属对象字段。
- 更新标准 OMO 表单，对齐当前上游 schema，补齐新版 agents、顶层字段和结构化对象字段。
- 扩展 OpenCode provider preset，加入 NPM SDK selector、模型 URL 覆写、模型拉取和推荐模型元数据合并。
- 新增 stream health check，用于验证 Claude、Codex、Gemini、OpenCode provider 的流式响应可用性。
- 补齐 Amazon Bedrock：模型拉取使用本地 preset 导入，流式健康检查使用 Bedrock Runtime ConverseStream 和 AWS SigV4 签名。

## 重点验证

- OMO/OMO Slim 配置保存、导入、模型选择和生成结果。
- OpenCode provider preset 生成的 NPM SDK、options、models 和模型变体配置。
- Google/Anthropic/OpenAI-compatible/OpenAI/Amazon Bedrock 的模型拉取或推荐模型导入流程。
- Stream health check 在桌面、Web/headless 和远程服务器环境下的状态、超时、重试和错误分类。
- 现有 SQLite 运行时存储、Provider/MCP/Prompt/Skill/Proxy 设置兼容性。

## 已知边界

- OMO/OMO Slim 本身不直接暴露流式探测端点；需要测试其底层 OpenCode provider。
- Bedrock 模型列表来自内置 preset，不调用 AWS 的模型枚举接口。
