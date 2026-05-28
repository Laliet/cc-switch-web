import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  getStreamCheckConfig,
  saveStreamCheckConfig,
  streamCheckAllProviders,
  streamCheckProvider,
  type StreamCheckConfig,
} from "@/lib/api/model-test";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const config: StreamCheckConfig = {
  timeoutSecs: 45,
  maxRetries: 2,
  degradedThresholdMs: 6000,
  claudeModel: "claude-haiku",
  codexModel: "gpt-5.4",
  geminiModel: "gemini-flash",
  testPrompt: "Who are you?",
};

describe("stream check API", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("checks one provider", async () => {
    const response = {
      status: "operational",
      success: true,
      message: "ok",
      modelUsed: "gpt-5.4",
      testedAt: 1,
      retryCount: 0,
    };
    invokeMock.mockResolvedValueOnce(response);

    const result = await streamCheckProvider("opencode", "openai");

    expect(result).toBe(response);
    expect(invokeMock).toHaveBeenCalledWith("stream_check_provider", {
      appType: "opencode",
      providerId: "openai",
    });
  });

  it("checks all providers", async () => {
    invokeMock.mockResolvedValueOnce([]);

    await streamCheckAllProviders("claude", true);

    expect(invokeMock).toHaveBeenCalledWith("stream_check_all_providers", {
      appType: "claude",
      proxyTargetsOnly: true,
    });
  });

  it("loads and saves config", async () => {
    invokeMock.mockResolvedValueOnce(config);
    expect(await getStreamCheckConfig()).toBe(config);
    expect(invokeMock).toHaveBeenCalledWith("get_stream_check_config");

    invokeMock.mockResolvedValueOnce(undefined);
    await saveStreamCheckConfig(config);
    expect(invokeMock).toHaveBeenCalledWith("save_stream_check_config", {
      config,
    });
  });
});
