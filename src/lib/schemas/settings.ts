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
  defaultCostMultiplier: z.string().trim().default("1"),
  pricingModelSource: z.enum(["request", "response"]).default("response"),
});

const defaultProxyApp = () => ({
  enabled: false,
  autoFailoverEnabled: false,
  maxRetries: 0,
  defaultCostMultiplier: "1",
  pricingModelSource: "response" as const,
});

const defaultProxyApps = () => ({
  claude: defaultProxyApp(),
  codex: defaultProxyApp(),
  gemini: defaultProxyApp(),
  opencode: defaultProxyApp(),
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
  proxy: z
    .object({
      enabled: z.boolean(),
      host: z.string().trim().min(1),
      port: z.number().int().min(1).max(65535),
      upstreamProxy: z.string().trim().optional().or(z.literal("")),
      bindApp: z.enum(["claude", "codex", "gemini", "opencode"]),
      autoStart: z.boolean(),
      enableLogging: z.boolean().default(false),
      liveTakeoverActive: z.boolean().default(false),
      streamingFirstByteTimeout: z.number().int().min(1).max(3600).default(90),
      streamingIdleTimeout: z.number().int().min(1).max(3600).default(120),
      nonStreamingTimeout: z.number().int().min(1).max(3600).default(180),
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
