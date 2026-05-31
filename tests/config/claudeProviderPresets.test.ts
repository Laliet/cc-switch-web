import { describe, expect, it } from "vitest";
import { providerPresets } from "@/config/claudeProviderPresets";

describe("claude provider presets", () => {
  it("keeps 0.15.0 priority presets in author-defined partner-first order", () => {
    const expected = [
      "PatewayAI",
      "火山Agentplan",
      "BytePlus",
      "Baidu Qianfan Coding Plan",
      "ClaudeAPI",
      "ClaudeCN",
      "RunAPI",
      "RelaxyCode",
      "Compshare",
    ];
    const names = providerPresets.map((preset) => preset.name);
    const actual = names.filter((name) => expected.includes(name));

    expect(actual).toEqual(expected);
  });

  it("defines usable Claude role model env for priority presets", () => {
    const priority = new Set([
      "PatewayAI",
      "火山Agentplan",
      "BytePlus",
      "Baidu Qianfan Coding Plan",
      "ClaudeAPI",
      "ClaudeCN",
      "RunAPI",
      "RelaxyCode",
      "Compshare",
    ]);

    for (const preset of providerPresets.filter((item) =>
      priority.has(item.name),
    )) {
      const env = (preset.settingsConfig as { env?: Record<string, unknown> })
        .env;
      expect(env?.ANTHROPIC_BASE_URL).toEqual(expect.any(String));
      expect(env?.ANTHROPIC_AUTH_TOKEN).toBe("");
      expect(env?.ANTHROPIC_DEFAULT_HAIKU_MODEL).toEqual(expect.any(String));
      expect(env?.ANTHROPIC_DEFAULT_SONNET_MODEL).toEqual(expect.any(String));
      expect(env?.ANTHROPIC_DEFAULT_OPUS_MODEL).toEqual(expect.any(String));
    }
  });
});
