import { beforeEach, describe, expect, it, vi } from "vitest";
import { settingsApi } from "@/lib/api/settings";
import type {
  ProxyAppId,
  ProxyRecentLog,
  ProxySettings,
  ProxyStatus,
  ProxyTakeoverResult,
  ProxyTestResult,
} from "@/types";

const adapterMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock("@/lib/api/adapter", () => adapterMocks);

const createAppSettings = () => ({
  enabled: false,
  autoFailoverEnabled: false,
  maxRetries: 0,
});

const proxySettings: ProxySettings = {
  enabled: false,
  host: "127.0.0.1",
  port: 3456,
  upstreamProxy: "http://127.0.0.1:7890",
  bindApp: "claude",
  autoStart: false,
  enableLogging: true,
  liveTakeoverActive: false,
  streamingFirstByteTimeout: 90,
  streamingIdleTimeout: 120,
  nonStreamingTimeout: 180,
  circuitFailureThreshold: 3,
  circuitRecoveryThreshold: 2,
  circuitRecoveryWaitSeconds: 60,
  circuitErrorRateThreshold: 80,
  rectifyThinkingSignature: true,
  rectifyThinkingBudget: true,
  apps: {
    claude: createAppSettings(),
    codex: createAppSettings(),
    gemini: createAppSettings(),
    opencode: createAppSettings(),
  },
};

const proxyStatus: ProxyStatus = {
  running: true,
  address: "127.0.0.1",
  port: 3456,
  listenUrl: "http://127.0.0.1:3456",
  activeConnections: 2,
  totalRequests: 10,
  successRequests: 9,
  failedRequests: 1,
  successRate: 90,
  uptimeSeconds: 60,
  activeTargets: [
    {
      appType: "claude",
      providerId: "provider-1",
      providerName: "Provider One",
    },
  ],
  takeover: {
    claude: true,
    codex: false,
    gemini: false,
    opencode: false,
    omo: false,
  },
  bindApp: "claude",
};

describe("settingsApi proxy methods", () => {
  beforeEach(() => {
    adapterMocks.invoke.mockReset();
  });

  it("getProxyStatus invokes proxy_status", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(proxyStatus);

    const result = await settingsApi.getProxyStatus();

    expect(result).toBe(proxyStatus);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("proxy_status");
  });

  it("getProxyConfig invokes proxy_config", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(proxySettings);

    const result = await settingsApi.getProxyConfig();

    expect(result).toBe(proxySettings);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("proxy_config");
  });

  it("saveProxyConfig sends settings and returns saved config", async () => {
    const saved = { ...proxySettings, enabled: true };
    adapterMocks.invoke.mockResolvedValueOnce(saved);

    const result = await settingsApi.saveProxyConfig(proxySettings);

    expect(result).toBe(saved);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("save_proxy_config", {
      settings: proxySettings,
    });
  });

  it("saveProxySettings sends settings and returns boolean", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await settingsApi.saveProxySettings(proxySettings);

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("save_proxy_settings", {
      settings: proxySettings,
    });
  });

  it("startProxy sends settings and returns status", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(proxyStatus);

    const result = await settingsApi.startProxy(proxySettings);

    expect(result).toBe(proxyStatus);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("start_proxy", {
      settings: proxySettings,
    });
  });

  it("stopProxy invokes stop_proxy", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(proxyStatus);

    const result = await settingsApi.stopProxy();

    expect(result).toBe(proxyStatus);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("stop_proxy");
  });

  it("testProxy sends settings and returns test result", async () => {
    const testResult: ProxyTestResult = {
      success: true,
      message: "ok",
      baseUrl: "http://127.0.0.1:3456",
    };
    adapterMocks.invoke.mockResolvedValueOnce(testResult);

    const result = await settingsApi.testProxy(proxySettings);

    expect(result).toBe(testResult);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("test_proxy", {
      settings: proxySettings,
    });
  });

  it.each([
    ["claude", true],
    ["codex", false],
    ["gemini", true],
    ["opencode", false],
  ] as const)(
    "setProxyTakeover sends %s takeover payload",
    async (app: ProxyAppId, enabled: boolean) => {
      const takeoverResult: ProxyTakeoverResult = {
        app,
        enabled,
        status: proxyStatus,
      };
      adapterMocks.invoke.mockResolvedValueOnce(takeoverResult);

      const result = await settingsApi.setProxyTakeover(app, enabled);

      expect(result).toBe(takeoverResult);
      expect(adapterMocks.invoke).toHaveBeenCalledWith("set_proxy_takeover", {
        app,
        enabled,
      });
    },
  );

  it("restoreProxy invokes restore_proxy", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(proxyStatus);

    const result = await settingsApi.restoreProxy();

    expect(result).toBe(proxyStatus);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("restore_proxy");
  });

  it("recoverStaleProxyTakeover invokes recover_stale_proxy_takeover", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(proxyStatus);

    const result = await settingsApi.recoverStaleProxyTakeover();

    expect(result).toBe(proxyStatus);
    expect(adapterMocks.invoke).toHaveBeenCalledWith(
      "recover_stale_proxy_takeover",
    );
  });

  it("getProxyRecentLogs invokes proxy_recent_logs", async () => {
    const logs: ProxyRecentLog[] = [
      {
        at: "2026-05-07T08:00:00Z",
        app: "gemini",
        method: "POST",
        path: "/v1beta/models/gemini:generateContent?key=***",
        status: 200,
        durationMs: 12,
        error: null,
      },
    ];
    adapterMocks.invoke.mockResolvedValueOnce(logs);

    const result = await settingsApi.getProxyRecentLogs();

    expect(result).toBe(logs);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("proxy_recent_logs");
  });
});
