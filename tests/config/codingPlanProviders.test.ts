import { describe, expect, it } from "vitest";
import {
  detectBalanceProvider,
  detectCodingPlanProvider,
  injectNativeUsageScript,
} from "@/config/codingPlanProviders";
import type { Provider } from "@/types";

describe("native usage provider detection", () => {
  it("detects Coding Plan providers", () => {
    expect(
      detectCodingPlanProvider("https://api.kimi.com/coding"),
    ).toMatchObject({
      id: "kimi",
    });
    expect(
      detectCodingPlanProvider("https://open.bigmodel.cn/api/anthropic"),
    ).toMatchObject({
      id: "zhipu",
    });
    expect(
      detectCodingPlanProvider("https://api.minimax.io/anthropic"),
    ).toMatchObject({
      id: "minimax",
    });
  });

  it("detects all native balance provider families", () => {
    for (const url of [
      "https://api.deepseek.com",
      "https://api.stepfun.com",
      "https://api.siliconflow.cn",
      "https://api.siliconflow.com",
      "https://openrouter.ai/api/v1",
      "https://api.novita.ai",
    ]) {
      expect(detectBalanceProvider(url), url).toBeDefined();
    }
  });

  it("injects native usage metadata without replacing explicit configuration", () => {
    const provider = {
      name: "Kimi",
      settingsConfig: {
        env: { ANTHROPIC_BASE_URL: "https://api.kimi.com/coding" },
      },
    } as Omit<Provider, "id">;
    const injected = injectNativeUsageScript(provider);
    expect(injected.meta?.usage_script).toMatchObject({
      enabled: true,
      templateType: "token_plan",
      codingPlanProvider: "kimi",
    });

    const explicit = {
      ...provider,
      meta: {
        usage_script: {
          enabled: true,
          language: "javascript" as const,
          code: "custom",
          templateType: "custom" as const,
        },
      },
    };
    expect(injectNativeUsageScript(explicit)).toBe(explicit);
  });
});
