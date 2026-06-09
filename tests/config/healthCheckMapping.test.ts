import { describe, expect, it } from "vitest";
import type { Provider } from "@/types";
import {
  getRelayPulseProvider,
  getRelayPulseProviderFromProvider,
  getRelayPulseProviderFromUrl,
  isProviderMonitored,
} from "@/config/healthCheckMapping";

describe("health check provider mapping", () => {
  it("getRelayPulseProvider matches provider names in a case-insensitive way", () => {
    expect(getRelayPulseProvider("Fox Code")).toBe("foxcode");
    expect(getRelayPulseProvider("  DUCKCoding ")).toBe("duckcoding");
    expect(getRelayPulseProvider("unknown")).toBeUndefined();
  });

  it("getRelayPulseProviderFromUrl normalizes hostnames before matching", () => {
    expect(getRelayPulseProviderFromUrl("https://www.foxcode.io/api")).toBe(
      "foxcode",
    );
    expect(getRelayPulseProviderFromUrl("https://api.augmunt.com/v1")).toBe(
      "augmunt",
    );
    expect(getRelayPulseProviderFromUrl("invalid-url")).toBeUndefined();
  });

  it("getRelayPulseProviderFromProvider extracts mapping from name, env, config and website", () => {
    const byName: Provider = {
      id: "1",
      name: "Galaxy Code",
      settingsConfig: {},
    };
    expect(getRelayPulseProviderFromProvider(byName)).toBe("galaxycode");

    const byEnv: Provider = {
      id: "2",
      name: "Unknown",
      settingsConfig: {
        env: { ANTHROPIC_BASE_URL: "https://api.privnode.com" },
      },
    };
    expect(getRelayPulseProviderFromProvider(byEnv)).toBe("privnode");

    const byConfig: Provider = {
      id: "3",
      name: "Unknown",
      settingsConfig: { config: 'base_url = "https://api.xyai.io"' },
    };
    expect(getRelayPulseProviderFromProvider(byConfig)).toBe("xyai");

    const byWebsite: Provider = {
      id: "4",
      name: "Unknown",
      settingsConfig: {},
      websiteUrl: "https://www.packyapi.com",
    };
    expect(getRelayPulseProviderFromProvider(byWebsite)).toBe("packycode");
  });

  it("isProviderMonitored reports monitoring status", () => {
    const monitored: Provider = {
      id: "5",
      name: "Right.codes",
      settingsConfig: {},
    };
    const unmonitored: Provider = {
      id: "6",
      name: "not-tracked",
      settingsConfig: {},
    };

    expect(isProviderMonitored(monitored)).toBe(true);
    expect(isProviderMonitored(unmonitored)).toBe(false);
  });
});
