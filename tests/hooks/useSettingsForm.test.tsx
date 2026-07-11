import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import i18n from "i18next";
import { useSettingsForm } from "@/hooks/useSettingsForm";

const useSettingsQueryMock = vi.fn();

const proxyAppDefaults = {
  autoFailoverEnabled: false,
  defaultCostMultiplier: "1",
  maxRetries: 0,
  streamingFirstByteTimeout: 90,
  streamingIdleTimeout: 120,
  nonStreamingTimeout: 600,
  circuitFailureThreshold: 3,
  circuitRecoveryThreshold: 2,
  circuitRecoveryWaitSeconds: 60,
  circuitErrorRateThreshold: 80,
  circuitMinRequests: 10,
  pricingModelSource: "response",
};

vi.mock("@/lib/query", () => ({
  useSettingsQuery: (...args: unknown[]) => useSettingsQueryMock(...args),
}));

let changeLanguageSpy: ReturnType<typeof vi.spyOn<any, any>>;

beforeEach(() => {
  useSettingsQueryMock.mockReset();
  window.localStorage.clear();
  (i18n as any).language = "zh";
  changeLanguageSpy = vi
    .spyOn(i18n, "changeLanguage")
    .mockImplementation(async (lang?: string) => {
      (i18n as any).language = lang;
      return i18n.t;
    });
});

afterEach(() => {
  changeLanguageSpy.mockRestore();
});

describe("useSettingsForm Hook", () => {
  it("should normalize settings and sync language on initialization", async () => {
    useSettingsQueryMock.mockReturnValue({
      data: {
        showInTray: undefined,
        minimizeToTrayOnClose: undefined,
        enableClaudePluginIntegration: undefined,
        claudeConfigDir: "  /Users/demo  ",
        codexConfigDir: "   ",
        language: "en",
      },
      isLoading: false,
    });

    const { result } = renderHook(() => useSettingsForm());

    await waitFor(() => {
      expect(result.current.settings).not.toBeNull();
    });

    const settings = result.current.settings!;
    expect(settings.showInTray).toBe(true);
    expect(settings.minimizeToTrayOnClose).toBe(true);
    expect(settings.enableClaudePluginIntegration).toBe(false);
    expect(settings.claudeConfigDir).toBe("/Users/demo");
    expect(settings.codexConfigDir).toBeUndefined();
    expect(settings.network).toEqual({ githubMirrorBaseUrl: "" });
    expect(settings.language).toBe("en");
    expect(result.current.initialLanguage).toBe("en");
    expect(changeLanguageSpy).toHaveBeenCalledWith("en");
  });

  it("should fill default proxy settings when proxy is missing", async () => {
    useSettingsQueryMock.mockReturnValue({
      data: {
        showInTray: true,
        minimizeToTrayOnClose: true,
        enableClaudePluginIntegration: false,
        language: "zh",
      },
      isLoading: false,
    });

    const { result } = renderHook(() => useSettingsForm());

    await waitFor(() => {
      expect(result.current.settings?.proxy).toBeDefined();
    });

    expect(result.current.settings?.proxy).toEqual({
      enabled: false,
      host: "127.0.0.1",
      port: 3456,
      upstreamProxy: undefined,
      bindApp: "claude",
      autoStart: false,
      enableLogging: false,
      liveTakeoverActive: false,
      streamingFirstByteTimeout: 90,
      streamingIdleTimeout: 120,
      nonStreamingTimeout: 600,
      circuitFailureThreshold: 3,
      circuitRecoveryThreshold: 2,
      circuitRecoveryWaitSeconds: 60,
      circuitErrorRateThreshold: 80,
      rectifyThinkingSignature: true,
      rectifyThinkingBudget: true,
      optimizerEnabled: false,
      optimizerThinking: true,
      optimizerCacheInjection: true,
      optimizerCacheTtl: "1h",
      apps: {
        claude: { ...proxyAppDefaults, enabled: false },
        codex: { ...proxyAppDefaults, enabled: false },
        gemini: { ...proxyAppDefaults, enabled: false },
        opencode: {
          ...proxyAppDefaults,
          enabled: false,
        },
      },
    });
  });

  it("should normalize network mirror settings", async () => {
    useSettingsQueryMock.mockReturnValue({
      data: {
        showInTray: true,
        minimizeToTrayOnClose: true,
        enableClaudePluginIntegration: false,
        language: "zh",
        network: {
          githubMirrorBaseUrl: " https://ghproxy.net/ ",
        },
      },
      isLoading: false,
    });

    const { result } = renderHook(() => useSettingsForm());

    await waitFor(() => {
      expect(result.current.settings?.network).toBeDefined();
    });

    expect(result.current.settings?.network).toEqual({
      githubMirrorBaseUrl: "https://ghproxy.net/",
    });
  });

  it("should fill default network settings when network is missing", async () => {
    useSettingsQueryMock.mockReturnValue({
      data: {
        showInTray: true,
        minimizeToTrayOnClose: true,
        enableClaudePluginIntegration: false,
        language: "zh",
      },
      isLoading: false,
    });

    const { result } = renderHook(() => useSettingsForm());

    await waitFor(() => {
      expect(result.current.settings?.network).toBeDefined();
    });

    expect(result.current.settings?.network).toEqual({
      githubMirrorBaseUrl: "",
    });
  });

  it("should normalize partial proxy apps and sanitize upstream proxy", async () => {
    useSettingsQueryMock.mockReturnValue({
      data: {
        showInTray: true,
        minimizeToTrayOnClose: true,
        enableClaudePluginIntegration: false,
        language: "zh",
        proxy: {
          enabled: true,
          host: "0.0.0.0",
          port: 4567,
          upstreamProxy: "  http://proxy.local:8080  ",
          bindApp: "codex",
          autoStart: true,
          enableLogging: true,
          liveTakeoverActive: true,
          streamingFirstByteTimeout: 10,
          streamingIdleTimeout: 20,
          nonStreamingTimeout: 30,
          apps: {
            claude: { enabled: true },
          },
        },
      },
      isLoading: false,
    });

    const { result } = renderHook(() => useSettingsForm());

    await waitFor(() => {
      expect(result.current.settings?.proxy?.apps.codex).toBeDefined();
    });

    expect(result.current.settings?.proxy?.upstreamProxy).toBe(
      "http://proxy.local:8080",
    );
    expect(result.current.settings?.proxy?.apps.claude).toEqual({
      ...proxyAppDefaults,
      enabled: true,
    });
    expect(result.current.settings?.proxy?.apps.codex).toEqual({
      ...proxyAppDefaults,
      enabled: false,
    });
    expect(result.current.settings?.proxy?.apps.gemini).toEqual({
      ...proxyAppDefaults,
      enabled: false,
    });
    expect(result.current.settings?.proxy?.apps.opencode).toEqual({
      ...proxyAppDefaults,
      enabled: false,
    });
  });

  it("should remove blank proxy upstream proxy", async () => {
    useSettingsQueryMock.mockReturnValue({
      data: {
        showInTray: true,
        minimizeToTrayOnClose: true,
        enableClaudePluginIntegration: false,
        language: "zh",
        proxy: {
          enabled: false,
          host: "127.0.0.1",
          port: 3456,
          upstreamProxy: "   ",
          bindApp: "claude",
          autoStart: false,
          enableLogging: false,
          liveTakeoverActive: false,
          streamingFirstByteTimeout: 90,
          streamingIdleTimeout: 120,
          nonStreamingTimeout: 600,
          apps: {
            claude: {
              enabled: false,
              autoFailoverEnabled: false,
              maxRetries: 0,
            },
            codex: {
              enabled: false,
              autoFailoverEnabled: false,
              maxRetries: 0,
            },
            gemini: {
              enabled: false,
              autoFailoverEnabled: false,
              maxRetries: 0,
            },
            opencode: {
              enabled: false,
              autoFailoverEnabled: false,
              maxRetries: 0,
            },
          },
        },
      },
      isLoading: false,
    });

    const { result } = renderHook(() => useSettingsForm());

    await waitFor(() => {
      expect(result.current.settings?.proxy).toBeDefined();
    });

    expect(result.current.settings?.proxy?.upstreamProxy).toBeUndefined();
  });

  it("should prioritize reading language from local storage in readPersistedLanguage", () => {
    useSettingsQueryMock.mockReturnValue({
      data: null,
      isLoading: false,
    });
    window.localStorage.setItem("language", "en");

    const { result } = renderHook(() => useSettingsForm());

    const lang = result.current.readPersistedLanguage();
    expect(lang).toBe("en");
    expect(changeLanguageSpy).not.toHaveBeenCalled();
  });

  it("should update fields and sync language when language changes in updateSettings", () => {
    useSettingsQueryMock.mockReturnValue({
      data: null,
      isLoading: false,
    });

    const { result } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.updateSettings({ showInTray: false });
    });

    expect(result.current.settings?.showInTray).toBe(false);

    changeLanguageSpy.mockClear();
    act(() => {
      result.current.updateSettings({ language: "en" });
    });

    expect(result.current.settings?.language).toBe("en");
    expect(changeLanguageSpy).toHaveBeenCalledWith("en");
  });

  it("should reset with server data and restore initial language in resetSettings", async () => {
    useSettingsQueryMock.mockReturnValue({
      data: {
        showInTray: true,
        minimizeToTrayOnClose: true,
        enableClaudePluginIntegration: false,
        claudeConfigDir: "/origin",
        codexConfigDir: null,
        language: "en",
      },
      isLoading: false,
    });

    const { result } = renderHook(() => useSettingsForm());

    await waitFor(() => {
      expect(result.current.settings).not.toBeNull();
    });

    changeLanguageSpy.mockClear();
    (i18n as any).language = "zh";

    act(() => {
      result.current.resetSettings({
        showInTray: false,
        minimizeToTrayOnClose: false,
        enableClaudePluginIntegration: true,
        claudeConfigDir: "  /reset  ",
        codexConfigDir: "   ",
        language: "zh",
      });
    });

    const settings = result.current.settings!;
    expect(settings.showInTray).toBe(false);
    expect(settings.minimizeToTrayOnClose).toBe(false);
    expect(settings.enableClaudePluginIntegration).toBe(true);
    expect(settings.claudeConfigDir).toBe("/reset");
    expect(settings.codexConfigDir).toBeUndefined();
    expect(settings.language).toBe("zh");
    expect(result.current.initialLanguage).toBe("en");
    expect(changeLanguageSpy).toHaveBeenCalledWith("en");
  });

  it("should not call changeLanguage repeatedly when language is consistent in syncLanguage", async () => {
    useSettingsQueryMock.mockReturnValue({
      data: {
        showInTray: true,
        minimizeToTrayOnClose: true,
        enableClaudePluginIntegration: false,
        claudeConfigDir: null,
        codexConfigDir: null,
        language: "zh",
      },
      isLoading: false,
    });

    const { result } = renderHook(() => useSettingsForm());

    await waitFor(() => {
      expect(result.current.settings).not.toBeNull();
    });

    changeLanguageSpy.mockClear();
    (i18n as any).language = "zh";

    act(() => {
      result.current.syncLanguage("zh");
    });

    expect(changeLanguageSpy).not.toHaveBeenCalled();
  });
});
