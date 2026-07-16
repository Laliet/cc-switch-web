import type { ProviderCategory } from "@/types";
import { upstreamGeminiProviderPresetsV315 } from "./upstreamGeminiProviderPresetsV315";

/**
 * Gemini 预设供应商的视觉主题配置
 */
export interface GeminiPresetTheme {
  /** 图标类型：'gemini' | 'generic' */
  icon?: "gemini" | "generic";
  /** 背景色（选中状态），支持 hex 颜色 */
  backgroundColor?: string;
  /** 文字色（选中状态），支持 hex 颜色 */
  textColor?: string;
}

export interface GeminiProviderPreset {
  name: string;
  websiteUrl: string;
  apiKeyUrl?: string;
  settingsConfig: object;
  baseURL?: string;
  model?: string;
  description?: string;
  category?: ProviderCategory;
  isPartner?: boolean;
  partnerPromotionKey?: string;
  endpointCandidates?: string[];
  theme?: GeminiPresetTheme;
}

const localGeminiProviderPresets: GeminiProviderPreset[] = [
  {
    name: "Google Official",
    websiteUrl: "https://ai.google.dev/",
    apiKeyUrl: "https://aistudio.google.com/apikey",
    settingsConfig: {
      env: {
        GEMINI_MODEL: "gemini-3-pro-preview",
      },
    },
    description: "Google 官方 Gemini API (OAuth)",
    category: "official",
    partnerPromotionKey: "google-official",
    theme: {
      icon: "gemini",
      backgroundColor: "#4285F4",
      textColor: "#FFFFFF",
    },
  },
  {
    name: "PackyCode",
    websiteUrl: "https://www.packyapi.com",
    apiKeyUrl: "https://www.packyapi.com/register?aff=cc-switch",
    settingsConfig: {
      env: {
        GOOGLE_GEMINI_BASE_URL: "https://www.packyapi.com",
        GEMINI_API_KEY: "",
        GEMINI_MODEL: "gemini-3-pro-preview",
      },
    },
    baseURL: "https://www.packyapi.com",
    model: "gemini-3-pro-preview",
    description: "PackyCode",
    category: "third_party",
    isPartner: true,
    partnerPromotionKey: "packycode",
    endpointCandidates: [
      "https://api-slb.packyapi.com",
      "https://www.packyapi.com",
    ],
  },
  {
    name: "AiHubMix",
    websiteUrl: "https://aihubmix.com",
    apiKeyUrl: "https://aihubmix.com",
    settingsConfig: {
      env: {
        GOOGLE_GEMINI_BASE_URL: "https://aihubmix.com/gemini",
        GEMINI_API_KEY: "",
        GEMINI_MODEL: "gemini-3-pro-preview",
      },
    },
    baseURL: "https://aihubmix.com/gemini",
    model: "gemini-3-pro-preview",
    description: "AiHubMix",
    category: "third_party",
    endpointCandidates: [
      "https://aihubmix.com/gemini",
      "https://api.aihubmix.com/gemini",
    ],
  },
  {
    name: "DMXAPI",
    websiteUrl: "https://www.dmxapi.cn",
    apiKeyUrl: "https://www.dmxapi.cn",
    settingsConfig: {
      env: {
        GOOGLE_GEMINI_BASE_URL: "https://www.dmxapi.cn",
        GEMINI_API_KEY: "",
        GEMINI_MODEL: "gemini-3-pro-preview",
      },
    },
    baseURL: "https://www.dmxapi.cn",
    model: "gemini-3-pro-preview",
    description: "DMXAPI",
    category: "third_party",
    endpointCandidates: ["https://www.dmxapi.cn"],
  },
];

const geminiPresetKey = (preset: GeminiProviderPreset) =>
  (preset.baseURL || preset.name).trim().toLowerCase().replace(/\/+$/, "");

const localGeminiKeys = new Set(
  localGeminiProviderPresets.map(geminiPresetKey),
);

export const geminiPresetSyncReportV315 = upstreamGeminiProviderPresetsV315.map(
  (preset) => ({
    name: preset.name,
    key: geminiPresetKey(preset),
    disposition: localGeminiKeys.has(geminiPresetKey(preset))
      ? ("duplicate_local_preferred" as const)
      : ("merged" as const),
  }),
);

export const geminiProviderPresets: GeminiProviderPreset[] = [
  ...localGeminiProviderPresets,
  ...upstreamGeminiProviderPresetsV315.filter(
    (preset) => !localGeminiKeys.has(geminiPresetKey(preset)),
  ),
];

export function getGeminiPresetByName(
  name: string,
): GeminiProviderPreset | undefined {
  return geminiProviderPresets.find((preset) => preset.name === name);
}

export function getGeminiPresetByUrl(
  url: string,
): GeminiProviderPreset | undefined {
  if (!url) return undefined;
  return geminiProviderPresets.find(
    (preset) =>
      preset.baseURL &&
      url.toLowerCase().includes(preset.baseURL.toLowerCase()),
  );
}
