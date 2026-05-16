# cc-switch-web v0.11.1

> Hotfix for Docker / Web blank-page startup after login

**[中文更新说明 Chinese Documentation ->](release-note-v0.11.1-zh.md)**

## Overview

`v0.11.1` is a hotfix release for `v0.11.0`. It fixes a Docker / Web deployment issue where the app could render a blank page after login.

Affected users could see a browser console error similar to:

```text
vendor-i18n-*.js: Cannot read properties of undefined (reading 'createContext')
```

The issue was caused by the Web production manual chunking strategy. `react-i18next` was split into a separate `vendor-i18n` chunk, which could execute before React was initialized reliably enough for `createContext`.

## Fixes

- Keep `i18next` and `react-i18next` in the `vendor-react` chunk to avoid the separate `vendor-i18n` runtime ordering issue
- Add regression coverage for the Web Vite chunking configuration so `vendor-i18n` is not reintroduced
- Preserve the `v0.11.0` Web/headless local proxy, client takeover, and OpenCode / OMO management behavior

## Upgrade Notes

- All `v0.11.0` Docker / Web users should upgrade to `v0.11.1`
- Desktop users can also upgrade to keep builds aligned with the same release line
- Docker users can use `ghcr.io/laliet/cc-switch-web:0.11.1` or `ghcr.io/laliet/cc-switch-web:latest`

## Validation

This release was validated with:

- `pnpm vitest run tests/config/webViteConfig.test.ts`
- `pnpm typecheck`
- `pnpm build:web`
- Full `docker build -t cc-switch-web:test-issue21 .`
- Container startup from the full Dockerfile image and Web page access
- Playwright + Chrome browser-level smoke test confirming the app renders without `createContext`, `vendor-i18n`, or `Uncaught TypeError` errors

The validation confirmed that the entry HTML loads `vendor-react` and no longer emits or loads `vendor-i18n`.
