import { beforeEach, describe, expect, it, vi } from "vitest";
import { fetchModelsForConfig } from "@/lib/api/model-fetch";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("model fetch API", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("invokes fetch_models_for_config with endpoint credentials", async () => {
    invokeMock.mockResolvedValueOnce([{ id: "gpt-5.4", ownedBy: "openai" }]);

    const result = await fetchModelsForConfig(
      "https://api.example.com/v1",
      "sk-test",
      "@ai-sdk/openai-compatible",
      false,
      "https://api.example.com/v1/models",
    );

    expect(result).toEqual([{ id: "gpt-5.4", ownedBy: "openai" }]);
    expect(invokeMock).toHaveBeenCalledWith("fetch_models_for_config", {
      baseUrl: "https://api.example.com/v1",
      apiKey: "sk-test",
      npm: "@ai-sdk/openai-compatible",
      isFullUrl: false,
      modelsUrl: "https://api.example.com/v1/models",
    });
  });
});
