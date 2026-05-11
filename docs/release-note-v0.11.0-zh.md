# cc-switch-web v0.11.0

> Web/headless 本地代理、客户端接管与 OpenCode / OMO 管理改进的稳定版

**[English Version ->](release-note-v0.11.0-en.md)**

## 概览

`v0.11.0` 是 `0.11` 版本线的稳定版。本版本将 `v0.11.0-rc.1`、`v0.11.0-rc.2` 与 `v0.11.0-rc.3` 中验证过的 Web/headless 本地 HTTP 代理工作流提升为当前推荐稳定版本。

本版本适合日常使用。启用本地代理接管时，cc-switch-web 会临时修改 Claude Code、Codex、Gemini CLI、OpenCode 等受支持客户端的配置文件；停止代理或执行恢复接管时会尝试还原这些客户端配置。

## 重点更新

- 面向 Claude Code、Codex/OpenAI 兼容、Gemini、OpenCode 请求的 Web/headless 本地 HTTP 代理
- 通过一个本地代理端口完成按客户端识别的路由，并通过 cc-switch-web 切换供应商
- 设置页支持代理启动、停止、状态查看、测试、自动启动、超时参数、最近日志与逐客户端接管/恢复
- 支持 Claude、Codex、Gemini 的实时配置接管与恢复，并提供实验性 OpenCode 接管
- 新增 Claude、Codex/OpenAI 兼容、Gemini、OpenCode 的供应商端点适配与规范化
- 新增 Web/headless 代理状态、配置、设置、测试、日志、接管、恢复、陈旧接管恢复等 API
- 恢复 OMO 下 MCP 与 Skills 管理入口，并明确复用 OpenCode 共享存储

## 相比 0.10.1 的主要修复

- 根据真实服务器验证结果强化代理启动与接管体验
- 代理端口被占用、重复启动等场景给出更清晰的错误
- 防止接管请求重复触发，并修复接管成功提示重复堆叠或看起来消不掉的问题
- 保留 Claude provider 的 `env` 外层对象，并规范化根层 `ANTHROPIC_*` 片段
- 修复 Anthropic Skills 默认仓库扫描路径，并迁移已有空扫描路径条目
- 更严格地隐藏代理日志中的敏感值
- 修复代理接管运行中恢复状态不同步的问题
- 修复 Release workflow 的 release notes 处理逻辑
- 稳定 App 集成测试中 lazy-loaded 弹窗的断言时序

## 升级说明

- `v0.10.1` 用户可以直接升级到 `v0.11.0`。
- 正在使用任意 `v0.11.0-rc.*` 的用户建议升级到本稳定版。
- 在共享或远程环境启用代理接管前，请先检查接管设置。
- 除非明确需要局域网访问且理解暴露风险，否则建议保持代理监听地址为 `127.0.0.1`。

## 已知范围

- 本地代理是应用层 HTTP API 代理，不会修改系统全局代理、PAC 文件或 Clash 类规则。
- 多供应商排队故障转移、断路器 UI、用量/成本统计、跨供应商请求/流格式转换仍延后到后续版本。

## 验证

- `pnpm vitest run tests/integration/App.test.tsx`
- `pnpm vitest run`
