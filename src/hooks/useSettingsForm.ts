import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettingsQuery } from "@/lib/query";
import type { Settings } from "@/types";

type Language = "zh" | "en";

export type SettingsFormState = Omit<Settings, "language"> & {
  language: Language;
};

const normalizeLanguage = (lang?: string | null): Language => {
  if (!lang) return "zh";
  return lang === "en" ? "en" : "zh";
};

const sanitizeDir = (value?: string | null): string | undefined => {
  if (!value) return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
};

const defaultNetworkSettings = () => ({
  githubMirrorBaseUrl: "",
});

const normalizeNetworkSettings = (network?: Settings["network"]) => ({
  ...defaultNetworkSettings(),
  ...(network ?? {}),
  githubMirrorBaseUrl: network?.githubMirrorBaseUrl?.trim() ?? "",
});

const defaultProxyAppSettings = () => ({
  enabled: false,
  autoFailoverEnabled: false,
  maxRetries: 0,
  streamingFirstByteTimeout: 90,
  streamingIdleTimeout: 120,
  nonStreamingTimeout: 600,
  circuitFailureThreshold: 3,
  circuitRecoveryThreshold: 2,
  circuitRecoveryWaitSeconds: 60,
  circuitErrorRateThreshold: 80,
  circuitMinRequests: 10,
  defaultCostMultiplier: "1",
  pricingModelSource: "response",
});

const defaultProxySettings = () => ({
  enabled: false,
  host: "127.0.0.1",
  port: 3456,
  upstreamProxy: undefined,
  bindApp: "claude" as const,
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
  optimizerCacheTtl: "1h" as const,
  apps: {
    claude: defaultProxyAppSettings(),
    codex: defaultProxyAppSettings(),
    gemini: defaultProxyAppSettings(),
    opencode: defaultProxyAppSettings(),
  },
});

const normalizeProxySettings = (proxy?: Settings["proxy"]) => {
  const defaults = defaultProxySettings();
  return {
    ...defaults,
    ...(proxy ?? {}),
    upstreamProxy: sanitizeDir(proxy?.upstreamProxy),
    apps: {
      claude: {
        ...defaults.apps.claude,
        ...(proxy?.apps?.claude ?? {}),
      },
      codex: {
        ...defaults.apps.codex,
        ...(proxy?.apps?.codex ?? {}),
      },
      gemini: {
        ...defaults.apps.gemini,
        ...(proxy?.apps?.gemini ?? {}),
      },
      opencode: {
        ...defaults.apps.opencode,
        ...(proxy?.apps?.opencode ?? {}),
      },
    },
  };
};

export interface UseSettingsFormResult {
  settings: SettingsFormState | null;
  isLoading: boolean;
  initialLanguage: Language;
  updateSettings: (updates: Partial<SettingsFormState>) => void;
  resetSettings: (serverData: Settings | null) => void;
  readPersistedLanguage: () => Language;
  syncLanguage: (lang: Language) => void;
}

/**
 * useSettingsForm - 表单状态管理
 * 负责：
 * - 表单数据状态
 * - 表单字段更新
 * - 语言同步
 * - 表单重置
 */
export function useSettingsForm(): UseSettingsFormResult {
  const { i18n } = useTranslation();
  const { data, isLoading } = useSettingsQuery();

  const [settingsState, setSettingsState] = useState<SettingsFormState | null>(
    null,
  );

  const initialLanguageRef = useRef<Language>("zh");

  const readPersistedLanguage = useCallback((): Language => {
    if (typeof window !== "undefined") {
      try {
        const stored = window.localStorage.getItem("language");
        if (stored === "en" || stored === "zh") {
          return stored;
        }
      } catch {
        // localStorage 可能在隐私模式/受限环境下抛异常，忽略并回退到 i18n.language
      }
    }
    return normalizeLanguage(i18n.language);
  }, [i18n]);

  const syncLanguage = useCallback(
    (lang: Language) => {
      const current = normalizeLanguage(i18n.language);
      if (current !== lang) {
        void i18n.changeLanguage(lang);
      }
    },
    [i18n],
  );

  // 初始化设置数据
  useEffect(() => {
    if (!data) return;

    const normalizedLanguage = normalizeLanguage(
      data.language ?? readPersistedLanguage(),
    );

    const normalized: SettingsFormState = {
      ...data,
      showInTray: data.showInTray ?? true,
      minimizeToTrayOnClose: data.minimizeToTrayOnClose ?? true,
      enableClaudePluginIntegration:
        data.enableClaudePluginIntegration ?? false,
      claudeConfigDir: sanitizeDir(data.claudeConfigDir),
      codexConfigDir: sanitizeDir(data.codexConfigDir),
      geminiConfigDir: sanitizeDir(data.geminiConfigDir),
      opencodeConfigDir: sanitizeDir(data.opencodeConfigDir),
      network: normalizeNetworkSettings(data.network),
      proxy: normalizeProxySettings(data.proxy),
      language: normalizedLanguage,
    };

    setSettingsState(normalized);
    initialLanguageRef.current = normalizedLanguage;
    syncLanguage(normalizedLanguage);
  }, [data, readPersistedLanguage, syncLanguage]);

  const updateSettings = useCallback(
    (updates: Partial<SettingsFormState>) => {
      setSettingsState((prev) => {
        const base =
          prev ??
          ({
            showInTray: true,
            minimizeToTrayOnClose: true,
            enableClaudePluginIntegration: false,
            network: defaultNetworkSettings(),
            proxy: defaultProxySettings(),
            language: readPersistedLanguage(),
          } as SettingsFormState);

        const next: SettingsFormState = {
          ...base,
          ...updates,
        };

        if (updates.language) {
          const normalized = normalizeLanguage(updates.language);
          next.language = normalized;
          syncLanguage(normalized);
        }

        return next;
      });
    },
    [readPersistedLanguage, syncLanguage],
  );

  const resetSettings = useCallback(
    (serverData: Settings | null) => {
      if (!serverData) return;

      const normalizedLanguage = normalizeLanguage(
        serverData.language ?? readPersistedLanguage(),
      );

      const normalized: SettingsFormState = {
        ...serverData,
        showInTray: serverData.showInTray ?? true,
        minimizeToTrayOnClose: serverData.minimizeToTrayOnClose ?? true,
        enableClaudePluginIntegration:
          serverData.enableClaudePluginIntegration ?? false,
        claudeConfigDir: sanitizeDir(serverData.claudeConfigDir),
        codexConfigDir: sanitizeDir(serverData.codexConfigDir),
        geminiConfigDir: sanitizeDir(serverData.geminiConfigDir),
        opencodeConfigDir: sanitizeDir(serverData.opencodeConfigDir),
        network: normalizeNetworkSettings(serverData.network),
        proxy: normalizeProxySettings(serverData.proxy),
        language: normalizedLanguage,
      };

      setSettingsState(normalized);
      syncLanguage(initialLanguageRef.current);
    },
    [readPersistedLanguage, syncLanguage],
  );

  return {
    settings: settingsState,
    isLoading,
    initialLanguage: initialLanguageRef.current,
    updateSettings,
    resetSettings,
    readPersistedLanguage,
    syncLanguage,
  };
}
