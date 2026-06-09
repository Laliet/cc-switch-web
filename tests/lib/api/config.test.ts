import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  getClaudeCommonConfigSnippet,
  getCommonConfigSnippet,
  setClaudeCommonConfigSnippet,
  setCommonConfigSnippet,
} from "@/lib/api/config";

const adapterMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock("@/lib/api/adapter", () => adapterMocks);

describe("config API module", () => {
  beforeEach(() => {
    adapterMocks.invoke.mockReset();
  });

  it("getClaudeCommonConfigSnippet returns snippet", async () => {
    const snippet = '{"theme":"dark"}';
    adapterMocks.invoke.mockResolvedValueOnce(snippet);

    const result = await getClaudeCommonConfigSnippet();

    expect(result).toBe(snippet);
    expect(adapterMocks.invoke).toHaveBeenCalledWith(
      "get_claude_common_config_snippet",
    );
  });

  it("setClaudeCommonConfigSnippet invokes with snippet", async () => {
    const snippet = '{"theme":"light"}';
    adapterMocks.invoke.mockResolvedValueOnce(undefined);

    await setClaudeCommonConfigSnippet(snippet);

    expect(adapterMocks.invoke).toHaveBeenCalledWith(
      "set_claude_common_config_snippet",
      { snippet },
    );
  });

  it.each([
    ["claude", '{"fonts":["inter"]}'],
    ["codex", "raw-codex-snippet"],
    ["gemini", '{"fonts":["noto"]}'],
  ] as const)(
    "getCommonConfigSnippet requests %s snippet",
    async (appType, snippet) => {
      adapterMocks.invoke.mockResolvedValueOnce(snippet);

      const result = await getCommonConfigSnippet(appType);

      expect(result).toBe(snippet);
      expect(adapterMocks.invoke).toHaveBeenCalledWith(
        "get_common_config_snippet",
        { appType },
      );
    },
  );

  it.each([
    ["claude", '{"editor":"vim"}'],
    ["codex", '{"editor":"code"}'],
    ["gemini", '{"editor":"zed"}'],
  ] as const)(
    "setCommonConfigSnippet sends %s payload",
    async (appType, snippet) => {
      adapterMocks.invoke.mockResolvedValueOnce(undefined);

      await setCommonConfigSnippet(appType, snippet);

      expect(adapterMocks.invoke).toHaveBeenCalledWith(
        "set_common_config_snippet",
        { appType, snippet },
      );
    },
  );
});
