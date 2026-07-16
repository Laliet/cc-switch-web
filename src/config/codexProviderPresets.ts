/**
 * Codex 预设供应商配置模板
 */
import { ProviderCategory } from "../types";
import type { PresetTheme } from "./claudeProviderPresets";
import { upstreamCodexProviderPresetsV315 } from "./upstreamCodexProviderPresetsV315";

export interface CodexProviderPreset {
  name: string;
  websiteUrl: string;
  // 第三方供应商可提供单独的获取 API Key 链接
  apiKeyUrl?: string;
  auth: Record<string, any>; // 将写入 ~/.codex/auth.json
  config: string; // 将写入 ~/.codex/config.toml（TOML 字符串）
  isOfficial?: boolean; // 标识是否为官方预设
  isPartner?: boolean; // 标识是否为商业合作伙伴
  partnerPromotionKey?: string; // 合作伙伴促销信息的 i18n key
  category?: ProviderCategory; // 新增：分类
  isCustomTemplate?: boolean; // 标识是否为自定义模板
  // 新增：请求地址候选列表（用于地址管理/测速）
  endpointCandidates?: string[];
  // 新增：视觉主题配置
  theme?: PresetTheme;
  providerType?: "codex_oauth";
  requiresOAuth?: boolean;
}

function toTomlString(value: string): string {
  return JSON.stringify(value);
}

function validateAndNormalizeBaseUrl(baseUrl: string): string {
  const trimmed = (baseUrl ?? "").trim();

  if (!trimmed) {
    throw new Error("Base URL is required");
  }

  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    throw new Error("Invalid base URL format");
  }

  if (!/^https?:$/.test(parsed.protocol)) {
    throw new Error("Base URL must start with http:// or https://");
  }

  return parsed.toString().replace(/\/+$/, "");
}

/**
 * 生成第三方供应商的 auth.json
 */
export function generateThirdPartyAuth(apiKey: string): Record<string, any> {
  return {
    OPENAI_API_KEY: apiKey || "",
  };
}

/**
 * 生成第三方供应商的 config.toml
 */
export function generateThirdPartyConfig(
  providerName: string,
  baseUrl: string,
  modelName = "gpt-5-codex",
): string {
  // 清理供应商名称，确保符合TOML键名规范
  const cleanProviderName =
    providerName
      .toLowerCase()
      .replace(/[^a-z0-9_]/g, "_")
      .replace(/^_+|_+$/g, "") || "custom";

  const normalizedBaseUrl = validateAndNormalizeBaseUrl(baseUrl);
  const tomlProviderName = toTomlString(cleanProviderName);
  const tomlModelName = toTomlString(modelName);
  const tomlBaseUrl = toTomlString(normalizedBaseUrl);

  return `model_provider = ${tomlProviderName}
model = ${tomlModelName}
model_reasoning_effort = "high"
disable_response_storage = true

[model_providers.${cleanProviderName}]
name = ${tomlProviderName}
base_url = ${tomlBaseUrl}
wire_api = "responses"
requires_openai_auth = true`;
}

const localCodexProviderPresets: CodexProviderPreset[] = [
  {
    name: "OpenAI Official",
    websiteUrl: "https://chatgpt.com/codex",
    isOfficial: true,
    category: "official",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "openai",
      "https://api.openai.com/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://api.openai.com/v1"],
    theme: {
      icon: "codex",
      backgroundColor: "#1F2937", // gray-800
      textColor: "#FFFFFF",
    },
  },
  {
    name: "Codex OAuth",
    websiteUrl: "https://chatgpt.com/codex",
    isOfficial: true,
    category: "official",
    providerType: "codex_oauth",
    requiresOAuth: true,
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "codex_oauth",
      "https://chatgpt.com/backend-api/codex",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://chatgpt.com/backend-api/codex"],
    theme: {
      icon: "codex",
      backgroundColor: "#111827",
      textColor: "#FFFFFF",
    },
  },
  {
    name: "Azure OpenAI",
    websiteUrl:
      "https://learn.microsoft.com/azure/ai-services/openai/how-to/overview",
    category: "third_party",
    isOfficial: true,
    auth: generateThirdPartyAuth(""),
    config: `model_provider = "azure"
model = "gpt-5-codex"
model_reasoning_effort = "high"
disable_response_storage = true

[model_providers.azure]
name = "Azure OpenAI"
base_url = "https://YOUR_RESOURCE_NAME.openai.azure.com/openai"
env_key = "OPENAI_API_KEY"
query_params = { "api-version" = "2025-04-01-preview" }
wire_api = "responses"
requires_openai_auth = true`,
    endpointCandidates: ["https://YOUR_RESOURCE_NAME.openai.azure.com/openai"],
    theme: {
      icon: "codex",
      backgroundColor: "#0078D4",
      textColor: "#FFFFFF",
    },
  },
  {
    name: "AiHubMix",
    websiteUrl: "https://aihubmix.com",
    category: "aggregator",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "aihubmix",
      "https://aihubmix.com/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: [
      "https://aihubmix.com/v1",
      "https://api.aihubmix.com/v1",
    ],
  },
  {
    name: "DMXAPI",
    websiteUrl: "https://www.dmxapi.cn",
    category: "aggregator",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "dmxapi",
      "https://www.dmxapi.cn/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://www.dmxapi.cn/v1"],
  },
  {
    name: "PackyCode",
    websiteUrl: "https://www.packyapi.com",
    apiKeyUrl: "https://www.packyapi.com/register?aff=cc-switch",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "packycode",
      "https://www.packyapi.com/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: [
      "https://www.packyapi.com/v1",
      "https://api-slb.packyapi.com/v1",
    ],
    isPartner: true, // 合作伙伴
    partnerPromotionKey: "packycode", // 促销信息 i18n key
  },
  {
    name: "88code",
    websiteUrl: "https://www.88code.ai",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "88code",
      "https://www.88code.ai/openai/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://www.88code.ai/openai/v1"],
  },
  {
    name: "AICodeMirror",
    websiteUrl: "https://www.aicodemirror.com",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "aicodemirror",
      "https://api.aicodemirror.com/api/codex/backend-api/codex",
      "gpt-5-codex",
    ),
    endpointCandidates: [
      "https://api.aicodemirror.com/api/codex/backend-api/codex",
    ],
  },
  {
    name: "AIMZ",
    websiteUrl: "https://mzlone.top",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "aimz",
      "https://mzlone.top/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://mzlone.top/v1"],
  },
  {
    name: "CodeCli",
    websiteUrl: "https://code-cli.cn",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "codecli",
      "https://code-cli.cn/api/codex",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://code-cli.cn/api/codex"],
  },
  {
    name: "EasyChat",
    websiteUrl: "https://easychat.site",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "easychat",
      "https://server.easychat.site/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://server.easychat.site/v1"],
  },
  {
    name: "FoxCode",
    websiteUrl: "https://foxcode.rjj.cc",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "foxcode",
      "https://code.newcli.com/codex/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://code.newcli.com/codex/v1"],
  },
  {
    name: "GalaxyCode",
    websiteUrl: "https://nf.video",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "galaxycode",
      "https://relay.nf.video/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://relay.nf.video/v1"],
  },
  {
    name: "JikeAI",
    websiteUrl: "https://magic666.top",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "jikeai",
      "https://magic666.top/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://magic666.top/v1"],
  },
  {
    name: "Privnode",
    websiteUrl: "https://privnode.com",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "privnode",
      "https://privnode.com/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://privnode.com/v1"],
  },
  {
    name: "SSSAiCode",
    websiteUrl: "https://www.sssaicode.com",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "sssaicode",
      "https://codex2.sssaicode.com/api/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://codex2.sssaicode.com/api/v1"],
  },
  {
    name: "YesCode",
    websiteUrl: "https://co.yes.vg",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "yescode",
      "https://co.yes.vg/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://co.yes.vg/v1"],
  },
  {
    name: "duckcoding",
    websiteUrl: "https://duckcoding.com",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "duckcoding",
      "https://jp.duckcoding.com/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://jp.duckcoding.com/v1"],
  },
  {
    name: "ikuncode",
    websiteUrl: "https://api.ikuncode.cc",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "ikuncode",
      "https://api.ikuncode.cc/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://api.ikuncode.cc/v1"],
  },
  {
    name: "right",
    websiteUrl: "https://right.codes",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "right",
      "https://right.codes/codex/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://right.codes/codex/v1"],
  },
  {
    name: "uucode",
    websiteUrl: "https://www.uucode.org",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "uucode",
      "https://api.uucode.org",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://api.uucode.org"],
  },
  {
    name: "xyai",
    websiteUrl: "https://new.xychatai.com",
    category: "third_party",
    auth: generateThirdPartyAuth(""),
    config: generateThirdPartyConfig(
      "xyai",
      "https://new.xychatai.com/codex/v1",
      "gpt-5-codex",
    ),
    endpointCandidates: ["https://new.xychatai.com/codex/v1"],
  },
];

const codexPresetKey = (preset: CodexProviderPreset) =>
  preset.name.trim().toLowerCase();

const localCodexKeys = new Set(localCodexProviderPresets.map(codexPresetKey));

export const codexPresetSyncReportV315 = upstreamCodexProviderPresetsV315.map(
  (preset) => ({
    name: preset.name,
    key: codexPresetKey(preset),
    disposition: localCodexKeys.has(codexPresetKey(preset))
      ? ("duplicate_local_preferred" as const)
      : ("merged" as const),
  }),
);

export const codexProviderPresets: CodexProviderPreset[] = [
  ...localCodexProviderPresets,
  ...upstreamCodexProviderPresetsV315.filter(
    (preset) => !localCodexKeys.has(codexPresetKey(preset)),
  ),
];
