import { describe, expect, it } from "vitest";
import {
  openclawApiProtocols,
  openclawProviderPresets,
} from "@/config/openclawProviderPresets";

describe("openclawProviderPresets", () => {
  it("provides unique, valid provider keys and editable provider fragments", () => {
    const keys = openclawProviderPresets.map((preset) => preset.providerKey);
    expect(new Set(keys).size).toBe(keys.length);

    for (const preset of openclawProviderPresets) {
      expect(preset.providerKey).toMatch(/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/);
      expect(Array.isArray(preset.settingsConfig.models)).toBe(true);
      expect(preset.settingsConfig.models.length).toBeGreaterThan(0);
      expect(preset.settingsConfig.apiKey).not.toBeUndefined();
    }
  });

  it("uses only protocols accepted by the OpenClaw provider schema", () => {
    const protocols = new Set(openclawApiProtocols.map(({ value }) => value));
    for (const preset of openclawProviderPresets) {
      expect(protocols.has(preset.settingsConfig.api!)).toBe(true);
    }
  });

  it("keeps suggested model references inside their provider preset", () => {
    for (const preset of openclawProviderPresets) {
      const defaults = preset.suggestedDefaults?.model;
      if (!defaults) continue;

      const modelIds = new Set(
        preset.settingsConfig.models.map(
          (model) => `${preset.providerKey}/${model.id}`,
        ),
      );
      expect(modelIds.has(defaults.primary)).toBe(true);
      for (const fallback of defaults.fallbacks ?? []) {
        expect(modelIds.has(fallback)).toBe(true);
      }
    }
  });
});
