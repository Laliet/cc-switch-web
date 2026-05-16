# cc-switch-web v0.11.1

> Docker / Web 版白屏问题修复版

**[English Version ->](release-note-v0.11.1-en.md)**

## 概览

`v0.11.1` 是 `v0.11.0` 的 hotfix 版本，重点修复 Docker / Web 部署在登录后可能出现白屏的问题。

受影响用户会在浏览器控制台看到类似错误：

```text
vendor-i18n-*.js: Cannot read properties of undefined (reading 'createContext')
```

该问题由 Web 生产构建的手动分包策略触发：`react-i18next` 被拆入独立的 `vendor-i18n` chunk 后，运行时可能在 React 初始化顺序不稳定的情况下读取不到 `createContext`。

## 修复内容

- 将 `i18next` 与 `react-i18next` 固定合并到 `vendor-react` chunk，避免 `vendor-i18n` 独立 chunk 引发运行时初始化顺序问题
- 增加 Web Vite 分包配置回归测试，防止后续版本重新拆出 `vendor-i18n`
- 保持 `v0.11.0` 的 Web/headless 本地代理、客户端接管、OpenCode / OMO 管理能力不变

## 升级建议

- 所有使用 `v0.11.0` Docker / Web 版的用户建议升级到 `v0.11.1`
- 桌面版用户也可以升级到本版本，以保持版本一致并获得同一批发布产物
- Docker 用户可使用 `ghcr.io/laliet/cc-switch-web:0.11.1` 或 `ghcr.io/laliet/cc-switch-web:latest`

## 验证

本版本已完成以下验证：

- `pnpm vitest run tests/config/webViteConfig.test.ts`
- `pnpm typecheck`
- `pnpm build:web`
- 完整 `docker build -t cc-switch-web:test-issue21 .`
- 基于完整 Dockerfile 镜像启动容器并访问 Web 页面
- Playwright + Chrome 浏览器级验证，确认页面正常渲染且无 `createContext` / `vendor-i18n` / `Uncaught TypeError` 错误

验证结果确认入口 HTML 只加载 `vendor-react`，不再生成或加载 `vendor-i18n`。
