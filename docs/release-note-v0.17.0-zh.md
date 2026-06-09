# cc-switch-web v0.17.0 发布说明

v0.17.0 是 **认证中心与第三方代理增强** 版本，重点补齐 GitHub Copilot、Codex OAuth 托管登录能力，并把 Claude Desktop 第三方代理配置与本地代理 token 注入逻辑对齐到已验证的 cc-switch 处理方式。

## 认证中心

- 新增 Auth Center，用于集中管理 GitHub Copilot 与 Codex OAuth 账号。
- 支持 GitHub Copilot 设备码登录、token 刷新、live models 拉取与 usage 查询。
- 支持 Codex OAuth 设备码登录、token 刷新、live models 拉取与 usage 查询。
- 支持导入、删除、登出、设置默认账号，并允许供应商绑定默认账号或指定账号。
- 托管认证 token 现在会加密落库，并保留旧明文 token 的读取兼容。

## 代理与供应商

- GitHub Copilot 与 Codex OAuth 供应商可以通过托管账号解析真实 token，再注入到实际代理请求。
- Codex OAuth 代理请求会注入 `Authorization`、`originator` 与账号相关 header，处理逻辑与上游 cc-switch 保持一致。
- Claude Desktop 第三方代理预设新增 GitHub Copilot 与 Codex OAuth 入口，写入配置时会根据托管账号状态提示可用性。
- Codex 相关 Claude Desktop 预设模型路由同步上游 cc-switch 默认配置。

## Web 与桌面端

- 设置页新增 Auth Center 区块，提供设备码登录、账号列表、默认账号切换和用量查询入口。
- 供应商表单支持托管认证模式，并在 GitHub Copilot、Codex、Claude Desktop 相关配置中展示账号绑定状态。
- Web API 新增托管认证账号管理、设备码登录轮询、live models 与 usage 查询接口。
- 模型拉取接口增强，支持从托管认证账号获取凭据后查询远端模型列表。

## 测试与发布

- 增加认证中心、托管 token 加密、代理 token 注入、Claude Desktop 代理配置和 Web API 覆盖。
- 最终发布前已执行完整前端、Rust、Tauri 与 Web 构建验证；真实账号登录、真实 token 刷新、真实 Claude Desktop 重启验证不纳入本次自动验证范围。
