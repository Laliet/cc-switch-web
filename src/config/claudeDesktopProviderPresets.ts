import type { ProviderCategory } from "@/types";
import type { PresetTheme, TemplateValueConfig } from "./claudeProviderPresets";

export type ClaudeDesktopApiFormat =
  | "anthropic"
  | "openai_chat"
  | "openai_responses"
  | "gemini_native";

export interface ClaudeDesktopRoutePreset {
  routeId: string;
  upstreamModel: string;
  labelOverride?: string;
  supports1m?: boolean;
}

export interface ClaudeDesktopProviderPreset {
  name: string;
  websiteUrl: string;
  apiKeyUrl?: string;
  category?: ProviderCategory;
  settingsConfig: { env: { ANTHROPIC_BASE_URL: string } };
  isOfficial?: boolean;
  templateValues?: Record<string, TemplateValueConfig>;
  theme?: PresetTheme;
  baseUrl: string;
  mode: "direct" | "proxy";
  apiFormat?: ClaudeDesktopApiFormat;
  apiKeyField?: "ANTHROPIC_AUTH_TOKEN" | "ANTHROPIC_API_KEY";
  modelRoutes?: ClaudeDesktopRoutePreset[];
  endpointCandidates?: string[];
  isPartner?: boolean;
  partnerPromotionKey?: string;
}

export const CLAUDE_DESKTOP_ROLE_ROUTE_IDS = {
  sonnet: "claude-sonnet-4-6",
  opus: "claude-opus-4-7",
  haiku: "claude-haiku-4-5",
} as const;

const passthroughRoutes = (supports1m = false): ClaudeDesktopRoutePreset[] => [
  {
    routeId: CLAUDE_DESKTOP_ROLE_ROUTE_IDS.sonnet,
    upstreamModel: CLAUDE_DESKTOP_ROLE_ROUTE_IDS.sonnet,
    supports1m,
  },
  {
    routeId: CLAUDE_DESKTOP_ROLE_ROUTE_IDS.opus,
    upstreamModel: CLAUDE_DESKTOP_ROLE_ROUTE_IDS.opus,
    supports1m,
  },
  {
    routeId: CLAUDE_DESKTOP_ROLE_ROUTE_IDS.haiku,
    upstreamModel: CLAUDE_DESKTOP_ROLE_ROUTE_IDS.haiku,
    supports1m,
  },
];

const presetConfig = (baseUrl: string) => ({
  env: {
    ANTHROPIC_BASE_URL: baseUrl,
  },
});

const brandedRoutes = (
  model: string,
  supports1m = false,
): ClaudeDesktopRoutePreset[] => [
  {
    routeId: CLAUDE_DESKTOP_ROLE_ROUTE_IDS.sonnet,
    upstreamModel: model,
    labelOverride: model,
    supports1m,
  },
];

export const claudeDesktopProviderPresets: ClaudeDesktopProviderPreset[] = [
  {
    name: "PatewayAI",
    websiteUrl: "https://pateway.ai",
    category: "third_party",
    baseUrl: "https://api.pateway.ai",
    settingsConfig: presetConfig("https://api.pateway.ai"),
    mode: "direct",
    apiFormat: "anthropic",
    modelRoutes: passthroughRoutes(),
    isPartner: true,
    partnerPromotionKey: "patewayai",
  },
  {
    name: "火山Agentplan",
    websiteUrl: "https://www.volcengine.com/activity/agentplan",
    category: "cn_official",
    baseUrl: "https://ark.cn-beijing.volces.com/api/coding",
    settingsConfig: presetConfig("https://ark.cn-beijing.volces.com/api/coding"),
    mode: "proxy",
    apiFormat: "anthropic",
    modelRoutes: brandedRoutes("ark-code-latest"),
    isPartner: true,
    partnerPromotionKey: "volcengine_agentplan",
  },
  {
    name: "BytePlus",
    websiteUrl: "https://www.byteplus.com/en/product/modelark",
    category: "cn_official",
    baseUrl: "https://ark.ap-southeast.bytepluses.com/api/coding",
    settingsConfig: presetConfig("https://ark.ap-southeast.bytepluses.com/api/coding"),
    mode: "proxy",
    apiFormat: "anthropic",
    modelRoutes: brandedRoutes("ark-code-latest"),
    isPartner: true,
    partnerPromotionKey: "byteplus",
  },
  {
    name: "Baidu Qianfan Coding Plan",
    websiteUrl: "https://cloud.baidu.com/product/qianfan_modelbuilder",
    category: "cn_official",
    baseUrl: "https://qianfan.baidubce.com/anthropic/coding",
    settingsConfig: presetConfig("https://qianfan.baidubce.com/anthropic/coding"),
    mode: "proxy",
    apiFormat: "anthropic",
    modelRoutes: brandedRoutes("qianfan-code-latest"),
  },
  {
    name: "ClaudeAPI",
    websiteUrl: "https://claudeapi.com",
    category: "third_party",
    baseUrl: "https://gw.claudeapi.com",
    settingsConfig: presetConfig("https://gw.claudeapi.com"),
    mode: "direct",
    apiFormat: "anthropic",
    modelRoutes: passthroughRoutes(),
    isPartner: true,
    partnerPromotionKey: "claudeapi",
  },
  {
    name: "ClaudeCN",
    websiteUrl: "https://claudecn.top",
    category: "third_party",
    baseUrl: "https://claudecn.top",
    settingsConfig: presetConfig("https://claudecn.top"),
    mode: "direct",
    apiFormat: "anthropic",
    modelRoutes: passthroughRoutes(),
    isPartner: true,
    partnerPromotionKey: "claudecn",
  },
  {
    name: "RunAPI",
    websiteUrl: "https://runapi.co",
    category: "aggregator",
    baseUrl: "https://runapi.co",
    settingsConfig: presetConfig("https://runapi.co"),
    mode: "direct",
    apiFormat: "anthropic",
    modelRoutes: passthroughRoutes(),
    isPartner: true,
    partnerPromotionKey: "runapi",
  },
  {
    name: "RelaxyCode",
    websiteUrl: "https://www.relaxycode.com",
    category: "third_party",
    baseUrl: "https://www.relaxycode.com",
    settingsConfig: presetConfig("https://www.relaxycode.com"),
    mode: "direct",
    apiFormat: "anthropic",
    modelRoutes: passthroughRoutes(),
  },
  {
    name: "Compshare",
    websiteUrl: "https://www.compshare.cn",
    category: "aggregator",
    baseUrl: "https://api.modelverse.cn",
    settingsConfig: presetConfig("https://api.modelverse.cn"),
    mode: "direct",
    apiFormat: "anthropic",
    modelRoutes: passthroughRoutes(),
    isPartner: true,
    partnerPromotionKey: "ucloud",
  },
];
