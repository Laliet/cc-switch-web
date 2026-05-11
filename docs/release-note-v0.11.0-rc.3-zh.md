# cc-switch-web v0.11.0-rc.3

> 面向桌面与 Web/headless 场景的预发布候选版本，重点加固本地代理、客户端接管与云服务器使用体验

**[English Version ->](release-note-v0.11.0-rc.3-en.md)**

---

## 概览

`v0.11.0-rc.3` 是 cc-switch-web `0.11.0` 稳定版前的第三个候选版本。本版本在 `v0.11.0-rc.1` 与 `v0.11.0-rc.2` 的基础上，进一步修复了代理启动失败、应用接管失败、运行中恢复接管状态不一致、日志敏感信息泄露以及 Release workflow 覆盖更新日志等问题。

本版本适合从 `v0.11.0-rc.1` 或 `v0.11.0-rc.2` 升级并继续验证。使用本地代理接管功能时，cc-switch-web 会临时修改 Claude Code、Codex、Gemini CLI、OpenCode 等客户端配置；停止代理或执行恢复接管会尝试还原这些配置。

---

## 主要更新

- 新增 Web/headless 使用场景，支持在云服务器、远程主机和无桌面环境中通过浏览器管理 provider。
- 新增本地 HTTP API proxy，可按 Claude、Codex、Gemini、OpenCode 请求路径转发到对应 provider。
- 新增应用接管能力，可将受支持客户端临时写入本地代理地址。
- 新增代理状态、运行统计、最近请求日志、客户端接管目标和 per-client 测试能力。
- 新增代理 failover 能力，支持首字节超时等场景下切换备用 provider。
- 增强设置页、目录配置、Web 登录、环境变量冲突提示和导入导出体验。

---

## RC.3 修复

- 修复代理启动失败后仍可能把配置持久化为 `enabled=true` / `liveTakeoverActive=true` 的问题。
- 修复代理启动过程中多个客户端接管写入到一半失败时，已写入客户端未恢复的问题。
- 修复运行中开启或关闭接管时，先保存设置再写入真实客户端配置导致状态不一致的问题。
- 修复运行中执行恢复接管后，真实配置已恢复但 `/api/proxy/status` 仍返回旧 takeover 状态的问题。
- 修复 recent logs 的 `error` 字段可能通过 upstream error 字符串泄露 `key`、`api_key`、`access_token`、`token` 等敏感 query 的问题。
- 修复 Release workflow 使用固定 `body` 覆盖手写 GitHub Release notes 的问题。
- 修复当前进程内代理已运行时重复启动可能造成状态体验不清晰的问题。
- 修复代理端口被其他进程占用时错误提示不够明确的问题。
- 修复代理接管目标列表中 Claude 可能重复出现的问题。
- 修复流式请求首字节超时且尚未发送响应 body 时无法尝试 failover 的问题。

---

## Web Bundle 优化

- 将 settings、skills、provider add/edit form、usage script、MCP、prompt 面板改为按需加载。
- 将 Usage Script 中的 CodeMirror 编辑器改为异步加载，并提供轻量 textarea fallback。
- 将 Prettier 改为点击格式化时再动态加载。
- 为 Web 构建拆分 React、Radix、TanStack Query、i18n、icons、CodeMirror、Prettier 等 vendor chunks。
- Web 构建入口 chunk 从约 `2.16 MB` 降至约 `272 KB`，`pnpm build:web` 不再出现 Vite 大 chunk 警告。

---

## 升级建议

- 如果正在使用 `v0.11.0-rc.1` 或 `v0.11.0-rc.2`，建议升级到本候选版本继续验证。
- 如果之前启用过代理接管，升级后建议打开代理设置页检查接管状态，并在需要时执行“恢复接管”。
- Gemini OAuth provider 仍不支持代理接管；Gemini 接管请使用 API Key provider。
- OpenCode 接管仍属于实验性能力，建议保留原始配置备份。

---

## 验证

本版本相关变更已通过：

- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `pnpm build:web`
- `cargo test --manifest-path src-tauri/Cargo.toml --features web-server --test proxy_web_api`
- `git diff --check`

其中 `proxy_web_api` 覆盖了：

- 代理启动端口占用失败后 persisted config 不会错误变成 enabled。
- 运行中 restore 后 status takeover 与 active targets 会同步归零。
- upstream error recent log 不会泄露敏感 query。

---

## 已知边界

- 本地代理是应用内 HTTP API proxy，不是系统透明代理。
- 不修改 OS 全局代理设置。
- 暂不支持 PAC / Clash rule。
- OpenCode 接管仍为实验性能力。
- Gemini OAuth provider 明确不支持接管；测试 Gemini 接管请使用 API Key provider。
- 多 provider failover 队列、circuit breaker UI、跨 provider 请求/流式格式转换会放到后续版本。
