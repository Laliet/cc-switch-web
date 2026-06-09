import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  fetchCodexOauthModels,
  fetchGithubCopilotModels,
  fetchModelsForConfig,
} from "@/lib/api/model-fetch";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api/adapter", () => ({
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

  it("invokes get_codex_oauth_models with selected managed account", async () => {
    invokeMock.mockResolvedValueOnce([
      { id: "gpt-5-codex", ownedBy: "openai" },
    ]);

    const result = await fetchCodexOauthModels("account-1");

    expect(result).toEqual([{ id: "gpt-5-codex", ownedBy: "openai" }]);
    expect(invokeMock).toHaveBeenCalledWith("get_codex_oauth_models", {
      accountId: "account-1",
    });
  });

  it("invokes get_github_copilot_models with default managed account", async () => {
    invokeMock.mockResolvedValueOnce([
      { id: "claude-sonnet-4.6", ownedBy: "github-copilot" },
    ]);

    const result = await fetchGithubCopilotModels();

    expect(result).toEqual([
      { id: "claude-sonnet-4.6", ownedBy: "github-copilot" },
    ]);
    expect(invokeMock).toHaveBeenCalledWith("get_github_copilot_models", {
      accountId: null,
    });
  });
});
