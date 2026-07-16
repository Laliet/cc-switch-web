import type { ProviderCategory } from "@/types";
import type { PresetTheme, TemplateValueConfig } from "./claudeProviderPresets";
import { upstreamOpenClawProviderPresetsV315 } from "./upstreamOpenClawProviderPresetsV315";

export const openclawApiProtocols = [
  { value: "openai-completions", label: "OpenAI Completions" },
  { value: "openai-responses", label: "OpenAI Responses" },
  { value: "anthropic-messages", label: "Anthropic Messages" },
  { value: "google-generative-ai", label: "Google Generative AI" },
  { value: "bedrock-converse-stream", label: "AWS Bedrock" },
] as const;

export type OpenClawApiProtocol =
  (typeof openclawApiProtocols)[number]["value"];

export interface OpenClawModelPreset {
  id: string;
  name?: string;
  alias?: string;
  contextWindow?: number;
  cost?: Record<string, number>;
  [key: string]: unknown;
}

export interface OpenClawProviderConfigPreset {
  baseUrl?: string;
  apiKey?: string;
  api?: OpenClawApiProtocol;
  models: OpenClawModelPreset[];
  headers?: Record<string, string>;
  [key: string]: unknown;
}

export interface OpenClawSuggestedDefaults {
  model?: {
    primary: string;
    fallbacks?: string[];
  };
  modelCatalog?: Record<string, { alias?: string }>;
}

export interface OpenClawProviderPreset {
  name: string;
  nameKey?: string;
  providerKey: string;
  websiteUrl: string;
  apiKeyUrl?: string;
  settingsConfig: OpenClawProviderConfigPreset;
  isOfficial?: boolean;
  isPartner?: boolean;
  partnerPromotionKey?: string;
  category?: ProviderCategory;
  templateValues?: Record<string, TemplateValueConfig>;
  theme?: PresetTheme;
  icon?: string;
  iconColor?: string;
  isCustomTemplate?: boolean;
  suggestedDefaults?: OpenClawSuggestedDefaults;
}

const apiKeyTemplate = (placeholder = "sk-...") => ({
  apiKey: {
    label: "API Key",
    placeholder,
    editorValue: "",
  },
});

/**
 * OpenClaw stores all enabled providers under `models.providers`. These
 * presets are provider fragments, matching the additive format used upstream.
 */
const localOpenClawProviderPresets: OpenClawProviderPreset[] = [
  {
    name: "DeepSeek",
    providerKey: "deepseek",
    websiteUrl: "https://platform.deepseek.com",
    apiKeyUrl: "https://platform.deepseek.com/api_keys",
    settingsConfig: {
      baseUrl: "https://api.deepseek.com/v1",
      apiKey: "",
      api: "openai-completions",
      models: [
        {
          id: "deepseek-v4-pro",
          name: "DeepSeek V4 Pro",
          contextWindow: 1_000_000,
          cost: { input: 1.68, output: 3.36 },
        },
        {
          id: "deepseek-v4-flash",
          name: "DeepSeek V4 Flash",
          contextWindow: 1_000_000,
          cost: { input: 0.14, output: 0.28 },
        },
      ],
    },
    category: "cn_official",
    icon: "deepseek",
    iconColor: "#1E88E5",
    templateValues: apiKeyTemplate(),
    suggestedDefaults: {
      model: {
        primary: "deepseek/deepseek-v4-flash",
        fallbacks: ["deepseek/deepseek-v4-pro"],
      },
      modelCatalog: {
        "deepseek/deepseek-v4-flash": { alias: "Flash" },
        "deepseek/deepseek-v4-pro": { alias: "Pro" },
      },
    },
  },
  {
    name: "Zhipu GLM",
    providerKey: "zhipu",
    websiteUrl: "https://open.bigmodel.cn",
    apiKeyUrl: "https://open.bigmodel.cn/usercenter/apikeys",
    settingsConfig: {
      baseUrl: "https://open.bigmodel.cn/api/paas/v4",
      apiKey: "",
      api: "openai-completions",
      models: [
        {
          id: "glm-5",
          name: "GLM-5",
          contextWindow: 128_000,
          cost: { input: 0.001, output: 0.001 },
        },
      ],
    },
    category: "cn_official",
    icon: "zhipu",
    iconColor: "#0F62FE",
    templateValues: apiKeyTemplate(),
    suggestedDefaults: {
      model: { primary: "zhipu/glm-5" },
      modelCatalog: { "zhipu/glm-5": { alias: "GLM" } },
    },
  },
  {
    name: "Qwen Coder",
    providerKey: "qwen",
    websiteUrl: "https://bailian.console.aliyun.com",
    apiKeyUrl: "https://bailian.console.aliyun.com/#/api-key",
    settingsConfig: {
      baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
      apiKey: "",
      api: "openai-completions",
      models: [
        {
          id: "qwen3.5-plus",
          name: "Qwen3.5 Plus",
          contextWindow: 32_000,
          cost: { input: 0.002, output: 0.006 },
        },
      ],
    },
    category: "cn_official",
    icon: "qwen",
    iconColor: "#FF6A00",
    templateValues: apiKeyTemplate(),
    suggestedDefaults: {
      model: { primary: "qwen/qwen3.5-plus" },
      modelCatalog: { "qwen/qwen3.5-plus": { alias: "Qwen" } },
    },
  },
  {
    name: "Kimi",
    providerKey: "kimi",
    websiteUrl: "https://platform.moonshot.cn/console",
    apiKeyUrl: "https://platform.moonshot.cn/console/api-keys",
    settingsConfig: {
      baseUrl: "https://api.moonshot.cn/v1",
      apiKey: "",
      api: "openai-completions",
      models: [
        {
          id: "kimi-k2.6",
          name: "Kimi K2.6",
          contextWindow: 131_072,
          cost: { input: 0.002, output: 0.006 },
        },
      ],
    },
    category: "cn_official",
    icon: "kimi",
    iconColor: "#6366F1",
    templateValues: apiKeyTemplate(),
    suggestedDefaults: {
      model: { primary: "kimi/kimi-k2.6" },
      modelCatalog: { "kimi/kimi-k2.6": { alias: "Kimi" } },
    },
  },
  {
    name: "MiniMax",
    providerKey: "minimax",
    websiteUrl: "https://platform.minimaxi.com",
    apiKeyUrl:
      "https://platform.minimaxi.com/user-center/basic-information/interface-key",
    settingsConfig: {
      baseUrl: "https://api.minimaxi.com/v1",
      apiKey: "",
      api: "openai-completions",
      models: [
        {
          id: "MiniMax-M2.7",
          name: "MiniMax M2.7",
          contextWindow: 204_800,
          cost: { input: 0.3, output: 1.2 },
        },
      ],
    },
    category: "cn_official",
    icon: "minimax",
    iconColor: "#F43F5E",
    templateValues: apiKeyTemplate(),
    suggestedDefaults: {
      model: { primary: "minimax/MiniMax-M2.7" },
      modelCatalog: { "minimax/MiniMax-M2.7": { alias: "MiniMax" } },
    },
  },
  {
    name: "Volcengine AgentKit",
    providerKey: "volcengine",
    websiteUrl: "https://console.volcengine.com/ark",
    apiKeyUrl:
      "https://console.volcengine.com/ark/region:ark+cn-beijing/apiKey",
    settingsConfig: {
      baseUrl: "https://ark.cn-beijing.volces.com/api/coding/v3",
      apiKey: "",
      api: "openai-completions",
      models: [
        {
          id: "ark-code-latest",
          name: "Ark Code Latest",
          contextWindow: 256_000,
        },
      ],
    },
    category: "cn_official",
    icon: "huoshan",
    iconColor: "#3370FF",
    templateValues: apiKeyTemplate(),
    suggestedDefaults: {
      model: { primary: "volcengine/ark-code-latest" },
      modelCatalog: {
        "volcengine/ark-code-latest": { alias: "Ark Code" },
      },
    },
  },
  {
    name: "OpenRouter",
    providerKey: "openrouter",
    websiteUrl: "https://openrouter.ai",
    apiKeyUrl: "https://openrouter.ai/keys",
    settingsConfig: {
      baseUrl: "https://openrouter.ai/api/v1",
      apiKey: "",
      api: "openai-completions",
      models: [
        {
          id: "anthropic/claude-opus-4.7",
          name: "Claude Opus 4.7",
          contextWindow: 1_000_000,
          cost: { input: 5, output: 25 },
        },
        {
          id: "anthropic/claude-sonnet-4.6",
          name: "Claude Sonnet 4.6",
          contextWindow: 1_000_000,
          cost: { input: 3, output: 15 },
        },
      ],
    },
    category: "aggregator",
    icon: "openrouter",
    iconColor: "#6566F1",
    templateValues: apiKeyTemplate("sk-or-..."),
    suggestedDefaults: {
      model: {
        primary: "openrouter/anthropic/claude-opus-4.7",
        fallbacks: ["openrouter/anthropic/claude-sonnet-4.6"],
      },
      modelCatalog: {
        "openrouter/anthropic/claude-opus-4.7": { alias: "Opus" },
        "openrouter/anthropic/claude-sonnet-4.6": { alias: "Sonnet" },
      },
    },
  },
  {
    name: "AWS Bedrock",
    providerKey: "aws-bedrock",
    websiteUrl: "https://aws.amazon.com/bedrock/",
    settingsConfig: {
      baseUrl: "https://bedrock-runtime.us-west-2.amazonaws.com",
      apiKey: "",
      api: "bedrock-converse-stream",
      models: [
        {
          id: "anthropic.claude-opus-4-7",
          name: "Claude Opus 4.7",
          contextWindow: 1_000_000,
          cost: { input: 5, output: 25, cacheRead: 0.5, cacheWrite: 6.25 },
        },
        {
          id: "anthropic.claude-sonnet-4-6",
          name: "Claude Sonnet 4.6",
          contextWindow: 1_000_000,
          cost: { input: 3, output: 15, cacheRead: 0.3, cacheWrite: 3.75 },
        },
      ],
    },
    category: "cloud_provider",
    icon: "aws",
    iconColor: "#FF9900",
  },
  {
    name: "OpenAI Compatible",
    providerKey: "custom",
    websiteUrl: "",
    settingsConfig: {
      baseUrl: "",
      apiKey: "",
      api: "openai-completions",
      models: [{ id: "" }],
    },
    category: "custom",
    isCustomTemplate: true,
    icon: "generic",
    iconColor: "#6B7280",
    templateValues: {
      baseUrl: {
        label: "Base URL",
        placeholder: "https://api.example.com/v1",
        editorValue: "",
      },
      ...apiKeyTemplate(),
    },
  },
];

const inferUpstreamProviderKey = (
  preset: (typeof upstreamOpenClawProviderPresetsV315)[number],
): string => {
  const primary = preset.suggestedDefaults?.model?.primary;
  if (primary?.includes("/")) {
    return primary.slice(0, primary.indexOf("/"));
  }
  if (preset.name === "AWS Bedrock") return "aws-bedrock";
  if (preset.name === "OpenAI Compatible") return "custom";
  return preset.name
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "");
};

const upstreamOpenClawPresets = upstreamOpenClawProviderPresetsV315.map(
  (preset): OpenClawProviderPreset => ({
    ...preset,
    providerKey: inferUpstreamProviderKey(preset),
  }),
);

const localOpenClawKeys = new Set(
  localOpenClawProviderPresets.map((preset) => preset.providerKey),
);

export const openclawPresetSyncReportV315 = upstreamOpenClawPresets.map(
  (preset) => ({
    providerKey: preset.providerKey,
    name: preset.name,
    disposition: localOpenClawKeys.has(preset.providerKey)
      ? ("duplicate_local_preferred" as const)
      : ("merged" as const),
  }),
);

export const openclawProviderPresets: OpenClawProviderPreset[] = [
  ...localOpenClawProviderPresets,
  ...upstreamOpenClawPresets.filter(
    (preset) => !localOpenClawKeys.has(preset.providerKey),
  ),
];
