import { describe, expect, it } from "vitest";
import { claudeDesktopProviderPresets } from "@/config/claudeDesktopProviderPresets";

describe("claude desktop provider presets", () => {
  it("keeps OAuth proxy presets on safe Claude route ids with 1M support", () => {
    const oauthPresets = claudeDesktopProviderPresets.filter(
      (preset) =>
        preset.providerType === "github_copilot" ||
        preset.providerType === "codex_oauth",
    );

    expect(oauthPresets.map((preset) => preset.providerType)).toEqual([
      "github_copilot",
      "codex_oauth",
    ]);

    for (const preset of oauthPresets) {
      expect(preset.mode).toBe("proxy");
      expect(preset.requiresOAuth).toBe(true);
      expect(preset.modelRoutes).toHaveLength(3);
      expect(preset.modelRoutes?.map((route) => route.routeId)).toEqual([
        "claude-sonnet-4-6",
        "claude-opus-4-7",
        "claude-haiku-4-5",
      ]);
      expect(preset.modelRoutes?.every((route) => route.supports1m)).toBe(true);
      expect(
        preset.modelRoutes?.every(
          (route) => route.labelOverride === route.upstreamModel,
        ),
      ).toBe(true);
    }
  });
});
