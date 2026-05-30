# cc-switch-web v0.14.0 预发布说明

v0.14.0 聚焦补齐 Usage Dashboard、request logs 与 pricing/cost 闭环。

## 新增

- 新增完整 Usage Dashboard，支持查看总成本、请求数、真实 token 消耗、成功率、cache hit rate 与应用维度拆分。
- 新增请求趋势图，按当前时间范围展示代理请求成本变化。
- 新增 request logs 表格，支持按 Provider、Model、状态码、时间范围筛选，并支持分页。
- 新增请求详情面板，展示单次请求的 token、成本、延迟、流式状态、错误信息与数据来源。
- 新增 Provider / Model 统计表，便于定位高成本模型、低成功率 Provider 和异常延迟。
- 新增 Dashboard 内模型定价维护，并在更新模型价格后尝试回填历史零成本代理日志。
- 新增 Claude、Codex、Gemini 本地 session log 导入，支持增量同步、跨来源去重和基于模型定价的成本计算。
- 新增桌面端 Tauri commands 与 Web/headless `/api/usage/*` API。

## 说明

- Dashboard 统计会合并实时代理请求日志、历史 daily rollups 和已导入的 Claude/Codex/Gemini session logs。
- Session log 导入会先与 proxy 日志做指纹去重，避免同一次请求重复计费。
