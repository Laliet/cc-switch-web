export type ProxyAppId = "claude" | "codex" | "gemini" | "opencode";
export type ProxyRouteAppId = ProxyAppId | "claude-desktop";

export type ProviderCategory =
  | "official" // 官方
  | "cn_official" // 开源官方（原"国产官方"）
  | "aggregator" // 聚合网站
  | "third_party" // 第三方供应商
  | "cloud_provider"
  | "omo"
  | "omo-slim"
  | "custom"; // 自定义

export interface Provider {
  id: string;
  name: string;
  settingsConfig: Record<string, any>; // 应用配置对象：Claude 为 settings.json；Codex 为 { auth, config }
  websiteUrl?: string;
  // 新增：供应商分类（用于差异化提示/能力开关）
  category?: ProviderCategory;
  createdAt?: number; // 添加时间戳（毫秒）
  sortIndex?: number; // 排序索引（用于自定义拖拽排序）
  // 备注信息
  notes?: string;
  // 新增：是否为商业合作伙伴
  isPartner?: boolean;
  // 可选：供应商元数据（仅存于 ~/.cc-switch/config.json，不写入 live 配置）
  meta?: ProviderMeta;
}

export interface AppConfig {
  providers: Record<string, Provider>;
  current: string;
}

// 自定义端点配置
export interface CustomEndpoint {
  url: string;
  addedAt: number;
  lastUsed?: number;
}

// 端点候选项（用于端点测速弹窗）
export interface EndpointCandidate {
  id?: string;
  url: string;
  isCustom?: boolean;
}

// 用量查询脚本配置
export interface UsageScript {
  enabled: boolean; // 是否启用用量查询
  language: "javascript"; // 脚本语言
  code: string; // 脚本代码（JSON 格式配置）
  timeout?: number; // 超时时间（秒，默认 10）
  apiKey?: string; // 用量查询专用的 API Key（通用模板使用）
  baseUrl?: string; // 用量查询专用的 Base URL（通用和 NewAPI 模板使用）
  accessToken?: string; // 访问令牌（NewAPI 模板使用）
  userId?: string; // 用户ID（NewAPI 模板使用）
  autoQueryInterval?: number; // 自动查询间隔（单位：分钟，0 表示禁用）
}

// 单个套餐用量数据
export interface UsageData {
  planName?: string; // 套餐名称（可选）
  extra?: string; // 扩展字段，可自由补充需要展示的文本（可选）
  isValid?: boolean; // 套餐是否有效（可选）
  invalidMessage?: string; // 失效原因说明（可选，当 isValid 为 false 时显示）
  total?: number; // 总额度（可选）
  used?: number; // 已用额度（可选）
  remaining?: number; // 剩余额度（可选）
  unit?: string; // 单位（可选）
}

// 用量查询结果（支持多套餐）
export interface UsageResult {
  success: boolean;
  data?: UsageData[]; // 改为数组，支持返回多个套餐
  error?: string;
}

export type ClaudeDesktopMode = "direct" | "proxy";

export interface ClaudeDesktopModelRoute {
  model: string;
  labelOverride?: string;
  supports1m?: boolean;
}

export interface ProviderAuthBinding {
  // "managed" = Auth Center account, "api_key" = manual provider config.
  mode: "managed" | "api_key" | string;
  providerType?: "github_copilot" | "codex_oauth" | string;
  accountId?: string | null;
  useDefault?: boolean;
}

// 供应商元数据（字段名与后端一致，保持 snake_case）
export interface ProviderMeta {
  // 自定义端点：以 URL 为键，值为端点信息
  custom_endpoints?: Record<string, CustomEndpoint>;
  // Claude Desktop 写入模式：direct 直连 / proxy 本地路由
  claudeDesktopMode?: ClaudeDesktopMode;
  // Claude Desktop proxy 模式下，安全模型名到真实上游模型的映射
  claudeDesktopModelRoutes?: Record<string, ClaudeDesktopModelRoute>;
  // 用量查询脚本配置
  usage_script?: UsageScript;
  // 是否为官方合作伙伴
  isPartner?: boolean;
  // 合作伙伴促销 key（用于后端识别 PackyCode 等）
  partnerPromotionKey?: string;
  // 代理用量计费倍率
  costMultiplier?: string;
  // 计费用模型来源：request 或 response
  pricingModelSource?: string;
  // Claude/OpenAI 兼容供应商的 API 格式
  apiFormat?: "anthropic" | "openai_chat" | "openai_responses" | string;
  // Claude API key 字段名
  apiKeyField?: "ANTHROPIC_AUTH_TOKEN" | "ANTHROPIC_API_KEY" | string;
  // 是否把 baseUrl 视为完整 API endpoint
  isFullUrl?: boolean;
  // Responses 兼容端点的 prompt cache key
  promptCacheKey?: string;
  // Codex FAST mode 预留
  codexFastMode?: boolean;
  // 特殊 provider 类型标识
  providerType?: string;
  // 兼容上游 GitHub Copilot 账号绑定字段
  githubAccountId?: string;
  // Auth Center 账号绑定
  authBinding?: ProviderAuthBinding;
}

export interface UniversalProviderApps {
  claude: boolean;
  codex: boolean;
  gemini: boolean;
}

export interface ClaudeModelConfig {
  model?: string;
  haikuModel?: string;
  sonnetModel?: string;
  opusModel?: string;
}

export interface CodexModelConfig {
  model?: string;
  reasoningEffort?: string;
}

export interface GeminiModelConfig {
  model?: string;
}

export interface UniversalProviderModels {
  claude?: ClaudeModelConfig;
  codex?: CodexModelConfig;
  gemini?: GeminiModelConfig;
}

export interface UniversalProvider {
  id: string;
  name: string;
  providerType: string;
  apps: UniversalProviderApps;
  baseUrl: string;
  apiKey: string;
  models: UniversalProviderModels;
  websiteUrl?: string;
  notes?: string;
  meta?: ProviderMeta;
  createdAt?: number;
  sortIndex?: number;
}

export interface OpenCodeModel {
  name?: string;
  limit?: {
    context?: number;
    output?: number;
    [key: string]: unknown;
  };
  options?: Record<string, unknown>;
  [key: string]: unknown;
}

export interface OpenCodeProviderConfig {
  npm?: string;
  name?: string;
  options: {
    baseURL?: string;
    apiKey?: string;
    headers?: Record<string, string>;
    [key: string]: unknown;
  };
  models: Record<string, OpenCodeModel>;
  [key: string]: unknown;
}

// 应用设置类型（用于设置对话框与 Tauri API）
export interface Settings {
  // 是否在系统托盘（macOS 菜单栏）显示图标
  showInTray: boolean;
  // 点击关闭按钮时是否最小化到托盘而不是关闭应用
  minimizeToTrayOnClose: boolean;
  // 启用 Claude 插件联动（写入 ~/.claude/config.json 的 primaryApiKey）
  enableClaudePluginIntegration?: boolean;
  // 覆盖 Claude Code 配置目录（可选）
  claudeConfigDir?: string;
  // 覆盖 Codex 配置目录（可选）
  codexConfigDir?: string;
  // 覆盖 Gemini 配置目录（可选）
  geminiConfigDir?: string;
  // 覆盖 OpenCode 配置目录（可选，OMO 共用）
  opencodeConfigDir?: string;
  // 首选语言（可选，默认中文）
  language?: "en" | "zh";
  // Claude 自定义端点列表
  customEndpointsClaude?: Record<string, CustomEndpoint>;
  // Codex 自定义端点列表
  customEndpointsCodex?: Record<string, CustomEndpoint>;
  // 安全设置（兼容未来扩展）
  security?: {
    auth?: {
      selectedType?: string;
    };
  };
  proxy?: ProxySettings;
  webDav?: WebDavSettings;
  network?: NetworkSettings;
  skillSyncMethod?: SkillSyncMethod;
  skillStorageLocation?: SkillStorageLocation;
}

export type SkillSyncMethod = "auto" | "symlink" | "copy";

export type SkillStorageLocation = "cc_switch" | "unified";

export interface NetworkSettings {
  githubMirrorBaseUrl: string;
}

export interface WebDavSettings {
  enabled: boolean;
  autoSync: boolean;
  baseUrl: string;
  username: string;
  password: string;
  remoteDir: string;
  profile: string;
  lastSyncConfigHash?: string;
  lastSyncAt?: string;
  lastSyncRemoteSnapshotId?: string;
}

export interface WebDavCompatibilityCheck {
  name: string;
  ok: boolean;
  message: string;
}

export interface WebDavSnapshotPreview {
  exists: boolean;
  remotePath: string;
  snapshotId?: string;
  createdAt?: string;
  configHash?: string;
  sizeBytes?: number;
  modifiedAt?: string;
  artifactList: string[];
  configVersion?: number;
  schemaVersion?: number;
  compatible: boolean;
  checks: WebDavCompatibilityCheck[];
}

export interface WebDavBackupEntry {
  id: string;
  remotePath: string;
  sizeBytes?: number;
  modifiedAt?: string;
  createdAt?: string;
  artifactList: string[];
  configVersion?: number;
  schemaVersion?: number;
  compatible: boolean;
  checks: WebDavCompatibilityCheck[];
}

export interface WebDavSyncResult {
  success: boolean;
  message: string;
  remotePath: string;
  backupId?: string;
  preview?: WebDavSnapshotPreview;
}

export interface WebDavAutoSyncResult {
  action: "uploaded" | "downloaded" | "unchanged" | "conflict" | string;
  message: string;
  localConfigHash: string;
  remotePreview?: WebDavSnapshotPreview;
  result?: WebDavSyncResult;
}

export interface ProxySettings {
  enabled: boolean;
  host: string;
  port: number;
  upstreamProxy?: string;
  bindApp: ProxyRouteAppId;
  autoStart: boolean;
  enableLogging: boolean;
  liveTakeoverActive: boolean;
  streamingFirstByteTimeout: number;
  streamingIdleTimeout: number;
  nonStreamingTimeout: number;
  circuitFailureThreshold: number;
  circuitRecoveryThreshold: number;
  circuitRecoveryWaitSeconds: number;
  circuitErrorRateThreshold: number;
  rectifyThinkingSignature: boolean;
  rectifyThinkingBudget: boolean;
  optimizerEnabled: boolean;
  optimizerThinking: boolean;
  optimizerCacheInjection: boolean;
  optimizerCacheTtl: "5m" | "1h" | string;
  apps: Record<ProxyAppId, ProxyAppSettings>;
}

export interface ProxyAppSettings {
  enabled: boolean;
  autoFailoverEnabled: boolean;
  maxRetries: number;
  defaultCostMultiplier?: string;
  pricingModelSource?: string;
}

export interface FailoverQueueItem {
  providerId: string;
  providerName: string;
  position: number;
}

export interface ModelPricingRecord {
  modelId: string;
  displayName: string;
  inputCostPerMillion: string;
  outputCostPerMillion: string;
  cacheReadCostPerMillion: string;
  cacheCreationCostPerMillion: string;
}

export interface ProxyStatus {
  running: boolean;
  address: string;
  port: number;
  listenUrl?: string;
  activeConnections: number;
  totalRequests: number;
  successRequests: number;
  failedRequests: number;
  successRate: number;
  uptimeSeconds: number;
  activeTargets: ProxyActiveTarget[];
  takeover: Record<ProxyAppId | "omo", boolean>;
  bindApp: ProxyRouteAppId;
  lastRequestAt?: string;
  lastError?: string;
  failoverCount?: number;
  lastFailoverAt?: string;
  lastFailoverFrom?: string;
  lastFailoverTo?: string;
  providerHealth?: ProxyProviderHealth[];
}

export interface ProxyActiveTarget {
  appType: ProxyRouteAppId;
  providerId: string;
  providerName: string;
}

export interface ProxyProviderHealth {
  appType: ProxyAppId;
  providerId: string;
  state: "healthy" | "open" | "half_open" | string;
  failureCount: number;
  recoverySuccessCount: number;
  windowRequests: number;
  windowFailures: number;
  lastFailureSecondsAgo?: number;
  openedSecondsAgo?: number;
}

export interface ProxyTestResult {
  success: boolean;
  message: string;
  baseUrl?: string;
}

export interface ProxyTakeoverResult {
  app: ProxyAppId;
  enabled: boolean;
  status: ProxyStatus;
}

export interface ProxyRecentLog {
  at: string;
  app: string;
  method: string;
  path: string;
  status?: number | null;
  durationMs: number;
  error?: string | null;
}

export interface UsageRequestLog {
  requestId: string;
  providerId: string;
  providerName?: string | null;
  appType: string;
  model: string;
  requestModel?: string | null;
  costMultiplier: string;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  inputCostUsd: string;
  outputCostUsd: string;
  cacheReadCostUsd: string;
  cacheCreationCostUsd: string;
  totalCostUsd: string;
  isStreaming: boolean;
  latencyMs: number;
  firstTokenMs?: number | null;
  durationMs?: number | null;
  statusCode: number;
  errorMessage?: string | null;
  sessionId?: string | null;
  providerType?: string | null;
  createdAt: number;
  dataSource?: string | null;
  isUnpriced: boolean;
}

// MCP 服务器连接参数（宽松：允许扩展字段）
export interface McpServerSpec {
  // 可选：社区常见 .mcp.json 中 stdio 配置可不写 type
  type?: "stdio" | "http" | "sse";
  // stdio 字段
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  cwd?: string;
  // http 和 sse 字段
  url?: string;
  headers?: Record<string, string>;
  // 通用字段
  [key: string]: any;
}

// v3.7.0: MCP 服务器应用启用状态
export interface McpApps {
  claude: boolean;
  codex: boolean;
  gemini: boolean;
  opencode: boolean;
}

// MCP 服务器条目（v3.7.0 统一结构）
export interface McpServer {
  id: string;
  name: string;
  server: McpServerSpec;
  apps: McpApps; // v3.7.0: 标记应用到哪些客户端
  description?: string;
  tags?: string[];
  homepage?: string;
  docs?: string;
  // 兼容旧字段（v3.6.x 及以前）
  enabled?: boolean; // 已废弃，v3.7.0 使用 apps 字段
  source?: string;
  [key: string]: any;
}

// MCP 服务器映射（id -> McpServer）
export type McpServersMap = Record<string, McpServer>;

// MCP 配置状态
export interface McpStatus {
  userConfigPath: string;
  userConfigExists: boolean;
  serverCount: number;
}

// 新：来自 config.json 的 MCP 列表响应
export interface McpConfigResponse {
  configPath: string;
  servers: Record<string, McpServer>;
}
