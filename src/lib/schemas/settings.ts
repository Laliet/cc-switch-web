import { z } from "zod";

const directorySchema = z
  .string()
  .trim()
  .min(1, "路径不能为空")
  .optional()
  .or(z.literal(""));

const proxyAppSchema = z.object({
  enabled: z.boolean().default(false),
  autoFailoverEnabled: z.boolean().default(false),
  maxRetries: z.number().int().min(0).max(10).default(0),
  streamingFirstByteTimeout: z.number().int().min(1).max(120).default(90),
  streamingIdleTimeout: z.number().int().min(0).max(600).default(120),
  nonStreamingTimeout: z.number().int().min(60).max(1200).default(600),
  circuitFailureThreshold: z.number().int().min(1).max(20).default(3),
  circuitRecoveryThreshold: z.number().int().min(1).max(10).default(2),
  circuitRecoveryWaitSeconds: z.number().int().min(1).max(300).default(60),
  circuitErrorRateThreshold: z.number().min(1).max(100).default(80),
  circuitMinRequests: z.number().int().min(1).max(100).default(10),
  defaultCostMultiplier: z.string().trim().default("1"),
  pricingModelSource: z.enum(["request", "response"]).default("response"),
});

const defaultProxyApp = () => ({
  enabled: false,
  autoFailoverEnabled: false,
  maxRetries: 0,
  streamingFirstByteTimeout: 90,
  streamingIdleTimeout: 120,
  nonStreamingTimeout: 600,
  circuitFailureThreshold: 3,
  circuitRecoveryThreshold: 2,
  circuitRecoveryWaitSeconds: 60,
  circuitErrorRateThreshold: 80,
  circuitMinRequests: 10,
  defaultCostMultiplier: "1",
  pricingModelSource: "response" as const,
});

const defaultProxyApps = () => ({
  claude: defaultProxyApp(),
  codex: defaultProxyApp(),
  gemini: defaultProxyApp(),
  opencode: defaultProxyApp(),
});

const webDavSchema = z.object({
  enabled: z.boolean().default(false),
  autoSync: z.boolean().default(false),
  baseUrl: z.string().trim().default(""),
  username: z.string().trim().default(""),
  password: z.string().default(""),
  remoteDir: z.string().trim().min(1).default("cc-switch-web"),
  profile: z.string().trim().min(1).default("default"),
  lastSyncConfigHash: z.string().trim().optional(),
  lastSyncAt: z.string().trim().optional(),
  lastSyncRemoteSnapshotId: z.string().trim().optional(),
  lastSyncStatus: z.enum(["idle", "syncing", "success", "error"]).optional(),
  lastSyncError: z.string().trim().optional(),
});

const networkSchema = z.object({
  githubMirrorBaseUrl: z
    .string()
    .trim()
    .refine(
      (value) => {
        if (!value) return true;
        try {
          const url = new URL(value);
          return url.protocol === "http:" || url.protocol === "https:";
        } catch {
          return false;
        }
      },
      { message: "GitHub 镜像地址必须是 http(s) URL" },
    )
    .default(""),
});

export const settingsSchema = z.object({
  showInTray: z.boolean(),
  minimizeToTrayOnClose: z.boolean(),
  enableClaudePluginIntegration: z.boolean().optional(),
  claudeConfigDir: directorySchema.nullable().optional(),
  codexConfigDir: directorySchema.nullable().optional(),
  geminiConfigDir: directorySchema.nullable().optional(),
  opencodeConfigDir: directorySchema.nullable().optional(),
  language: z.enum(["en", "zh"]).optional(),
  customEndpointsClaude: z.record(z.string(), z.unknown()).optional(),
  customEndpointsCodex: z.record(z.string(), z.unknown()).optional(),
  webDav: webDavSchema.optional(),
  network: networkSchema.optional(),
  skillSyncMethod: z.enum(["auto", "symlink", "copy"]).default("auto"),
  skillStorageLocation: z.enum(["cc_switch", "unified"]).default("cc_switch"),
  backupIntervalHours: z.number().int().min(0).max(8760).optional(),
  backupRetainCount: z.number().int().min(1).max(100).optional(),
  proxy: z
    .object({
      enabled: z.boolean(),
      host: z.string().trim().min(1),
      port: z.number().int().min(1).max(65535),
      upstreamProxy: z.string().trim().optional().or(z.literal("")),
      bindApp: z.enum([
        "claude",
        "claude-desktop",
        "codex",
        "gemini",
        "opencode",
      ]),
      autoStart: z.boolean(),
      enableLogging: z.boolean().default(false),
      liveTakeoverActive: z.boolean().default(false),
      streamingFirstByteTimeout: z.number().int().min(1).max(3600).default(90),
      streamingIdleTimeout: z.number().int().min(1).max(3600).default(120),
      nonStreamingTimeout: z.number().int().min(1).max(3600).default(600),
      circuitFailureThreshold: z.number().int().min(1).max(100).default(3),
      circuitRecoveryThreshold: z.number().int().min(1).max(100).default(2),
      circuitRecoveryWaitSeconds: z.number().int().min(1).max(3600).default(60),
      circuitErrorRateThreshold: z.number().min(1).max(100).default(80),
      rectifyThinkingSignature: z.boolean().default(true),
      rectifyThinkingBudget: z.boolean().default(true),
      optimizerEnabled: z.boolean().default(false),
      optimizerThinking: z.boolean().default(true),
      optimizerCacheInjection: z.boolean().default(true),
      optimizerCacheTtl: z.enum(["5m", "1h"]).default("1h"),
      apps: z
        .object({
          claude: proxyAppSchema.default(defaultProxyApp),
          codex: proxyAppSchema.default(defaultProxyApp),
          gemini: proxyAppSchema.default(defaultProxyApp),
          opencode: proxyAppSchema.default(defaultProxyApp),
        })
        .default(defaultProxyApps),
    })
    .optional(),
});

export type SettingsFormData = z.infer<typeof settingsSchema>;
