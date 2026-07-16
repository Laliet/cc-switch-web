import { describe, expect, it } from "vitest";

import {
  openclawPresetSyncReportV315,
  openclawProviderPresets,
} from "@/config/openclawProviderPresets";
import {
  geminiPresetSyncReportV315,
  geminiProviderPresets,
} from "@/config/geminiProviderPresets";
import {
  codexPresetSyncReportV315,
  codexProviderPresets,
} from "@/config/codexProviderPresets";
import { upstreamOpenClawProviderPresetsV315 } from "@/config/upstreamOpenClawProviderPresetsV315";
import { upstreamGeminiProviderPresetsV315 } from "@/config/upstreamGeminiProviderPresetsV315";
import { upstreamCodexProviderPresetsV315 } from "@/config/upstreamCodexProviderPresetsV315";

describe("v3.15.0 provider preset synchronization", () => {
  it("accounts for every upstream OpenClaw preset and keeps valid unique keys", () => {
    expect(upstreamOpenClawProviderPresetsV315).toHaveLength(47);
    expect(openclawPresetSyncReportV315).toHaveLength(
      upstreamOpenClawProviderPresetsV315.length,
    );
    expect(
      openclawPresetSyncReportV315.every((entry) =>
        ["merged", "duplicate_local_preferred"].includes(entry.disposition),
      ),
    ).toBe(true);

    const keys = openclawProviderPresets.map((preset) => preset.providerKey);
    expect(new Set(keys).size).toBe(keys.length);
    for (const key of keys) {
      expect(key).toMatch(/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/);
    }
    expect(openclawProviderPresets.length).toBeGreaterThanOrEqual(
      upstreamOpenClawProviderPresetsV315.length,
    );
  });

  it("accounts for every upstream Gemini preset while preserving local entries", () => {
    expect(upstreamGeminiProviderPresetsV315).toHaveLength(16);
    expect(geminiPresetSyncReportV315).toHaveLength(
      upstreamGeminiProviderPresetsV315.length,
    );
    expect(geminiProviderPresets.length).toBeGreaterThanOrEqual(
      upstreamGeminiProviderPresetsV315.length,
    );
    expect(
      geminiPresetSyncReportV315.every((entry) =>
        ["merged", "duplicate_local_preferred", "excluded"].includes(
          entry.disposition,
        ),
      ),
    ).toBe(true);
    const keys = geminiProviderPresets.map((preset) =>
      (preset.baseURL || preset.name).trim().toLowerCase().replace(/\/+$/, ""),
    );
    expect(new Set(keys).size).toBe(keys.length);
    expect(
      geminiProviderPresets.some((preset) => preset.name === "DMXAPI"),
    ).toBe(true);
  });

  it("accounts for every upstream Codex preset while preserving local entries", () => {
    expect(upstreamCodexProviderPresetsV315).toHaveLength(27);
    expect(codexPresetSyncReportV315).toHaveLength(
      upstreamCodexProviderPresetsV315.length,
    );
    expect(codexProviderPresets.length).toBeGreaterThanOrEqual(
      upstreamCodexProviderPresetsV315.length,
    );
    expect(
      codexPresetSyncReportV315.every((entry) =>
        ["merged", "duplicate_local_preferred", "excluded"].includes(
          entry.disposition,
        ),
      ),
    ).toBe(true);
    const keys = codexProviderPresets.map((preset) =>
      preset.name.trim().toLowerCase(),
    );
    expect(new Set(keys).size).toBe(keys.length);
    expect(
      codexProviderPresets.some((preset) => preset.name === "duckcoding"),
    ).toBe(true);
  });
});
