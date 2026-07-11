import { beforeEach, describe, expect, it, vi } from "vitest";
import { usageApi } from "@/lib/api/usage";
import type { UsageResult } from "@/types";

const adapterMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

const i18nMocks = vi.hoisted(() => ({
  t: vi.fn(),
}));

vi.mock("@/lib/api/adapter", () => adapterMocks);
vi.mock("@/i18n", () => ({
  default: i18nMocks,
}));

describe("usageApi", () => {
  beforeEach(() => {
    adapterMocks.invoke.mockReset();
    i18nMocks.t.mockReset();
  });

  it("query returns usage result and sends provider/app", async () => {
    const result: UsageResult = { success: true, data: [] };
    adapterMocks.invoke.mockResolvedValueOnce(result);

    const response = await usageApi.query("provider-1", "claude");

    expect(response).toBe(result);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("queryProviderUsage", {
      providerId: "provider-1",
      app: "claude",
    });
  });

  it("query falls back to i18n when error has no message", async () => {
    i18nMocks.t.mockReturnValueOnce("fallback");
    adapterMocks.invoke.mockRejectedValueOnce({});

    const response = await usageApi.query("provider-1", "codex");

    expect(response).toEqual({ success: false, error: "fallback" });
    expect(i18nMocks.t).toHaveBeenCalledWith("errors.usage_query_failed");
  });

  it("testScript returns usage result and passes script params", async () => {
    const result: UsageResult = { success: true };
    adapterMocks.invoke.mockResolvedValueOnce(result);

    const response = await usageApi.testScript(
      "provider-2",
      "gemini",
      "console.log('ok')",
      12,
      "api-key",
      "https://example.test",
      "token",
      "user-1",
      "token_plan",
    );

    expect(response).toBe(result);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("testUsageScript", {
      providerId: "provider-2",
      app: "gemini",
      scriptCode: "console.log('ok')",
      timeout: 12,
      apiKey: "api-key",
      baseUrl: "https://example.test",
      accessToken: "token",
      userId: "user-1",
      templateType: "token_plan",
    });
  });

  it("testScript returns error message when invoke throws", async () => {
    adapterMocks.invoke.mockRejectedValueOnce(new Error("boom"));

    const response = await usageApi.testScript(
      "provider-3",
      "claude",
      "console.log('fail')",
    );

    expect(response).toEqual({ success: false, error: "boom" });
    expect(i18nMocks.t).not.toHaveBeenCalled();
  });

  it("passes stats filters to usage summary invoke", async () => {
    adapterMocks.invoke.mockResolvedValueOnce({
      totalRequests: 0,
      totalCost: "0",
      totalInputTokens: 0,
      totalOutputTokens: 0,
      totalCacheCreationTokens: 0,
      totalCacheReadTokens: 0,
      successRate: 0,
      realTotalTokens: 0,
      cacheHitRate: 0,
    });

    await usageApi.getUsageSummary(1, 2, "claude", {
      providerId: "provider-1",
      model: "claude-sonnet-4",
    });

    expect(adapterMocks.invoke).toHaveBeenCalledWith("get_usage_summary", {
      startDate: 1,
      endDate: 2,
      appType: "claude",
      providerId: "provider-1",
      model: "claude-sonnet-4",
    });
  });
});
