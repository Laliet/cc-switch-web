import type { OpenCodeModel, OpenCodeProviderConfig } from "@/types";

export const OPENCODE_DEFAULT_NPM = "@ai-sdk/openai-compatible";

export const OPENCODE_DEFAULT_CONFIG = JSON.stringify(
  {
    npm: OPENCODE_DEFAULT_NPM,
    options: {
      baseURL: "",
      apiKey: "",
      setCacheKey: true,
    },
    models: {},
  },
  null,
  2,
);

export const OPENCODE_KNOWN_OPTION_KEYS = [
  "baseURL",
  "apiKey",
  "headers",
  "isFullUrl",
  "modelsUrl",
] as const;
export const OPENCODE_KNOWN_MODEL_KEYS = ["name", "limit", "options"] as const;

export function isKnownOpencodeOptionKey(key: string): boolean {
  return OPENCODE_KNOWN_OPTION_KEYS.includes(
    key as (typeof OPENCODE_KNOWN_OPTION_KEYS)[number],
  );
}

export function isKnownModelKey(key: string): boolean {
  return OPENCODE_KNOWN_MODEL_KEYS.includes(
    key as (typeof OPENCODE_KNOWN_MODEL_KEYS)[number],
  );
}

export function parseOpencodeConfig(
  settingsConfig?: Record<string, unknown>,
): OpenCodeProviderConfig {
  const parsed = (settingsConfig ?? JSON.parse(OPENCODE_DEFAULT_CONFIG)) as
    | Partial<OpenCodeProviderConfig>
    | undefined;

  return {
    ...parsed,
    npm:
      typeof parsed?.npm === "string" && parsed.npm.trim()
        ? parsed.npm
        : OPENCODE_DEFAULT_NPM,
    options:
      parsed?.options && typeof parsed.options === "object"
        ? parsed.options
        : {},
    models:
      parsed?.models && typeof parsed.models === "object" ? parsed.models : {},
  };
}

export function parseOpencodeConfigStrict(
  settingsConfig?: Record<string, unknown>,
): OpenCodeProviderConfig {
  const parsed = JSON.parse(
    settingsConfig ? JSON.stringify(settingsConfig) : OPENCODE_DEFAULT_CONFIG,
  ) as Partial<OpenCodeProviderConfig>;

  return {
    ...parsed,
    npm:
      typeof parsed.npm === "string" && parsed.npm.trim()
        ? parsed.npm
        : OPENCODE_DEFAULT_NPM,
    options:
      parsed.options && typeof parsed.options === "object"
        ? parsed.options
        : {},
    models:
      parsed.models && typeof parsed.models === "object" ? parsed.models : {},
  };
}

export function parseExtraOptions(
  options: OpenCodeProviderConfig["options"],
): Record<string, string> {
  const extra: Record<string, string> = {};
  for (const [key, value] of Object.entries(options ?? {})) {
    if (!isKnownOpencodeOptionKey(key)) {
      extra[key] = typeof value === "string" ? value : JSON.stringify(value);
    }
  }
  return extra;
}

export const toOpencodeExtraOptions = parseExtraOptions;

export function parseModelExtraFields(
  model: OpenCodeModel,
): Record<string, string> {
  const extra: Record<string, string> = {};
  for (const [key, value] of Object.entries(model)) {
    if (!isKnownModelKey(key)) {
      extra[key] = typeof value === "string" ? value : JSON.stringify(value);
    }
  }
  return extra;
}

export const getModelExtraFields = parseModelExtraFields;
