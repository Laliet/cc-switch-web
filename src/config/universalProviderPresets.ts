import type { UniversalProvider, UniversalProviderModels } from "@/types";

export interface UniversalProviderPreset {
  name: string;
  providerType: string;
  models: UniversalProviderModels;
  websiteUrl?: string;
}

const DEFAULT_MODELS: UniversalProviderModels = {
  claude: {
    model: "claude-sonnet-4-6",
    haikuModel: "claude-haiku-4-5-20251001",
    sonnetModel: "claude-sonnet-4-6",
    opusModel: "claude-opus-4-7",
  },
  codex: { model: "gpt-5.4", reasoningEffort: "high" },
  gemini: { model: "gemini-3.1-pro" },
};

export const universalProviderPresets: UniversalProviderPreset[] = [
  {
    name: "NewAPI",
    providerType: "newapi",
    models: DEFAULT_MODELS,
    websiteUrl: "https://www.newapi.pro",
  },
  {
    name: "Custom Gateway",
    providerType: "custom_gateway",
    models: DEFAULT_MODELS,
  },
];

export function createUniversalProviderFromPreset(
  preset: UniversalProviderPreset,
): UniversalProvider {
  return {
    id: "",
    name: preset.name,
    providerType: preset.providerType,
    apps: { claude: true, codex: true, gemini: true },
    baseUrl: "",
    apiKey: "",
    models: structuredClone(preset.models),
    websiteUrl: preset.websiteUrl,
  };
}
