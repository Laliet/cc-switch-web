# cc-switch-web v0.15.0-rc.3 预发布说明

v0.15.0-rc.3 是 **Local Routing + Claude Desktop 对齐版** 的有效测试候选版本，重点修复 v0.15.0-rc.2 复查中发现的安全与运行时一致性问题。

## 安全修复

- Claude Desktop 本地 Gateway 增加 Bearer Token 校验，未配置、缺失或错误 token 的请求会返回 401。
- Gateway token 改为安全随机生成，并使用常量时间比较。
- Claude Desktop Gateway 的本地 `Authorization` 头不会再转发到上游，避免泄漏本地 gateway bearer token。
- Proxy 与 WebDAV 下载路径增加响应体大小限制，避免异常大响应造成内存风险。

## 稳定性修复

- Proxy 配置保存后支持运行时热更新，但仅热更新超时、日志、熔断、failover、定价与 Rectifier 等无需重启监听器的字段。
- host、port、upstream proxy 与 live takeover 状态不会被普通保存错误标记为已在运行中生效。
- live takeover / restore / recover stale takeover 在完成系统配置副作用后，会单独同步运行时 takeover 状态。
- 修复旧配置缺失 Rectifier 字段时被反序列化为关闭的问题，现在保持默认启用。

## Web 与构建修复

- WebDAV 设置 schema 增加前端校验，保留密码原始空格，同时规范化 URL、用户名、目录和 profile。
- 修复 web-server-only 与 desktop feature 下的测试、检查和发布构建边界。

## 说明

- Rectifier 当前仍是配置入口和体验对齐入口，不是完整的上游修复引擎。
- v0.15.0-rc.2 已发布但不建议作为有效测试基线；请使用 v0.15.0-rc.3 继续测试。

## 验证

- `pnpm build:web`
- `pnpm typecheck`
- `pnpm vitest run tests/lib/schemas/settings.test.ts`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --features web-server --example server`
- `cargo check --manifest-path src-tauri/Cargo.toml --features desktop`
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features web-server,test-hooks --lib`
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features web-server,test-hooks --test proxy_web_api`
