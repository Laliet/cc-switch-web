import { TEMPLATE_TYPES } from "@/config/constants";
import type { Provider, UsageScript } from "@/types";

export const CODING_PLAN_PROVIDERS = [
  { id: "kimi", label: "Kimi For Coding", pattern: /api\.kimi\.com\/coding/i },
  { id: "zhipu", label: "Zhipu GLM", pattern: /bigmodel\.cn|api\.z\.ai/i },
  {
    id: "minimax",
    label: "MiniMax Coding Plan",
    pattern: /api\.minimaxi\.com|api\.minimax\.io/i,
  },
] as const;

export const BALANCE_PROVIDERS = [
  { id: "deepseek", label: "DeepSeek", pattern: /api\.deepseek\.com/i },
  { id: "stepfun", label: "StepFun", pattern: /api\.stepfun\.(ai|com)/i },
  {
    id: "siliconflow",
    label: "SiliconFlow",
    pattern: /api\.siliconflow\.(cn|com)/i,
  },
  { id: "openrouter", label: "OpenRouter", pattern: /openrouter\.ai/i },
  { id: "novita", label: "Novita AI", pattern: /api\.novita\.ai/i },
] as const;

export function detectCodingPlanProvider(baseUrl?: string | null) {
  return CODING_PLAN_PROVIDERS.find((provider) =>
    provider.pattern.test(baseUrl ?? ""),
  );
}

export function detectBalanceProvider(baseUrl?: string | null) {
  return BALANCE_PROVIDERS.find((provider) =>
    provider.pattern.test(baseUrl ?? ""),
  );
}

function providerBaseUrl(provider: Pick<Provider, "settingsConfig">): string {
  const env = provider.settingsConfig?.env ?? {};
  return (
    env.ANTHROPIC_BASE_URL ??
    env.OPENAI_BASE_URL ??
    env.CODEX_BASE_URL ??
    env.OPENROUTER_BASE_URL ??
    env.GOOGLE_GEMINI_BASE_URL ??
    env.GEMINI_API_BASE_URL ??
    ""
  );
}

export function injectNativeUsageScript<T extends Omit<Provider, "id">>(
  provider: T,
): T {
  if (provider.meta?.usage_script) return provider;
  const baseUrl = providerBaseUrl(provider as Pick<Provider, "settingsConfig">);
  const codingPlan = detectCodingPlanProvider(baseUrl);
  const balance = detectBalanceProvider(baseUrl);
  if (!codingPlan && !balance) return provider;
  const usageScript: UsageScript = {
    enabled: true,
    language: "javascript",
    code: "",
    timeout: 10,
    templateType: codingPlan
      ? TEMPLATE_TYPES.TOKEN_PLAN
      : TEMPLATE_TYPES.BALANCE,
    codingPlanProvider: codingPlan?.id,
  };
  return {
    ...provider,
    meta: {
      ...(provider.meta ?? {}),
      usage_script: usageScript,
    },
  };
}
