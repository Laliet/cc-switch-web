import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { settingsApi, type AppId } from "@/lib/api";
import type { DirectoryAppId } from "@/config/apps";
import type { ConfigDirInfo } from "@/lib/api/settings";
import type { SettingsFormState } from "./useSettingsForm";
import { isWeb } from "@/lib/api/adapter";

type DirectoryKey = "appConfig" | DirectoryAppId;

export interface ResolvedDirectories {
  appConfig: string;
  claude: string;
  codex: string;
  gemini: string;
  opencode: string;
}

export interface ResolvedDirectoryInfoMap {
  claude?: ConfigDirInfo;
  codex?: ConfigDirInfo;
  gemini?: ConfigDirInfo;
  opencode?: ConfigDirInfo;
}

const sanitizeDir = (value?: string | null): string | undefined => {
  if (!value) return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
};

const loadPathApi = async () => {
  if (isWeb()) return null;
  try {
    return await import("@tauri-apps/api/path");
  } catch (error) {
    console.error("[useDirectorySettings] Failed to load path API", error);
    return null;
  }
};

const computeDefaultAppConfigDir = async (): Promise<string | undefined> => {
  if (isWeb()) return undefined;

  const env = typeof process !== "undefined" ? process.env : undefined;
  const fallbackHome =
    env?.VITEST === "true" ? "/home/mock" : (env?.HOME ?? "/home/mock");
  if (env?.VITEST === "true") {
    return `${fallbackHome}/.cc-switch`;
  }
  try {
    const pathApi = await loadPathApi();
    if (!pathApi) {
      return `${fallbackHome}/.cc-switch`;
    }
    const home = await pathApi.homeDir();
    return await pathApi.join(home, ".cc-switch");
  } catch (error) {
    console.error(
      "[useDirectorySettings] Failed to resolve default app config dir",
      error,
    );
    return `${fallbackHome}/.cc-switch`;
  }
};

const computeDefaultConfigDir = async (
  app: DirectoryAppId,
): Promise<string | undefined> => {
  if (isWeb()) return undefined;

  const env = typeof process !== "undefined" ? process.env : undefined;
  const fallbackHome =
    env?.VITEST === "true" ? "/home/mock" : (env?.HOME ?? "/home/mock");
  const folder =
    app === "claude"
      ? ".claude"
      : app === "codex"
        ? ".codex"
        : app === "gemini"
          ? ".gemini"
          : ".config/opencode";
  if (env?.VITEST === "true") {
    return `${fallbackHome}/${folder}`;
  }
  try {
    const pathApi = await loadPathApi();
    if (!pathApi) {
      return `${fallbackHome}/${folder}`;
    }
    const home = await pathApi.homeDir();
    if (app === "opencode") {
      return await pathApi.join(home, ".config", "opencode");
    }
    return await pathApi.join(home, folder);
  } catch (error) {
    console.error(
      "[useDirectorySettings] Failed to resolve default config dir",
      error,
    );
    return `${fallbackHome}/${folder}`;
  }
};

export interface UseDirectorySettingsProps {
  settings: SettingsFormState | null;
  onUpdateSettings: (updates: Partial<SettingsFormState>) => void;
}

export interface UseDirectorySettingsResult {
  appConfigDir?: string;
  resolvedDirs: ResolvedDirectories;
  resolvedDirInfo: ResolvedDirectoryInfoMap;
  isLoading: boolean;
  initialAppConfigDir?: string;
  updateDirectory: (app: DirectoryAppId, value?: string) => void;
  updateAppConfigDir: (value?: string) => void;
  browseDirectory: (app: DirectoryAppId) => Promise<void>;
  browseAppConfigDir: () => Promise<void>;
  resetDirectory: (app: DirectoryAppId) => Promise<void>;
  resetAppConfigDir: () => Promise<void>;
  resetAllDirectories: (
    claudeDir?: string,
    codexDir?: string,
    geminiDir?: string,
    opencodeDir?: string,
  ) => void;
  applyWslTemplate: (distro?: string) => void;
}

/**
 * useDirectorySettings - 目录管理
 * 负责：
 * - appConfigDir 状态
 * - resolvedDirs 状态
 * - 目录选择（browse）
 * - 目录重置
 * - 默认值计算
 */
export function useDirectorySettings({
  settings,
  onUpdateSettings,
}: UseDirectorySettingsProps): UseDirectorySettingsResult {
  const { t } = useTranslation();

  const [appConfigDir, setAppConfigDir] = useState<string | undefined>(
    undefined,
  );
  const [resolvedDirs, setResolvedDirs] = useState<ResolvedDirectories>({
    appConfig: "",
    claude: "",
    codex: "",
    gemini: "",
    opencode: "",
  });
  const [resolvedDirInfo, setResolvedDirInfo] =
    useState<ResolvedDirectoryInfoMap>({});
  const [isLoading, setIsLoading] = useState(true);

  const defaultsRef = useRef<ResolvedDirectories>({
    appConfig: "",
    claude: "",
    codex: "",
    gemini: "",
    opencode: "",
  });
  const initialAppConfigDirRef = useRef<string | undefined>(undefined);

  // 加载目录信息
  useEffect(() => {
    let active = true;
    setIsLoading(true);

    const loadConfigDirInfo = async (
      app: DirectoryAppId,
    ): Promise<ConfigDirInfo> => {
      const maybeGetConfigDirInfo = (
        settingsApi as {
          getConfigDirInfo?: (appId: AppId) => Promise<ConfigDirInfo>;
        }
      ).getConfigDirInfo;
      if (typeof maybeGetConfigDirInfo === "function") {
        try {
          return await maybeGetConfigDirInfo(app);
        } catch (error) {
          console.warn(
            `[useDirectorySettings] Falling back to legacy config dir API for ${app}`,
            error,
          );
        }
      }

      const dir = await settingsApi.getConfigDir(app);
      return {
        dir,
        source: "service-home-default",
        homeMismatch: false,
      };
    };

    const loadAppConfigOverride = async (): Promise<string | null> => {
      try {
        return await settingsApi.getAppConfigDirOverride();
      } catch (error) {
        if (isWeb()) {
          return null;
        }
        throw error;
      }
    };

    const load = async () => {
      try {
        const [
          overrideRaw,
          claudeInfo,
          codexInfo,
          geminiInfo,
          opencodeInfo,
          defaultAppConfig,
          defaultClaudeDir,
          defaultCodexDir,
          defaultGeminiDir,
          defaultOpencodeDir,
        ] = await Promise.all([
          loadAppConfigOverride(),
          loadConfigDirInfo("claude"),
          loadConfigDirInfo("codex"),
          loadConfigDirInfo("gemini"),
          loadConfigDirInfo("opencode"),
          computeDefaultAppConfigDir(),
          computeDefaultConfigDir("claude"),
          computeDefaultConfigDir("codex"),
          computeDefaultConfigDir("gemini"),
          computeDefaultConfigDir("opencode"),
        ]);

        if (!active) return;

        const normalizedOverride = sanitizeDir(overrideRaw ?? undefined);

        defaultsRef.current = {
          appConfig: defaultAppConfig ?? "",
          claude: defaultClaudeDir ?? "",
          codex: defaultCodexDir ?? "",
          gemini: defaultGeminiDir ?? "",
          opencode: defaultOpencodeDir ?? "",
        };

        setAppConfigDir(normalizedOverride);
        initialAppConfigDirRef.current = normalizedOverride;
        setResolvedDirInfo({
          claude: claudeInfo,
          codex: codexInfo,
          gemini: geminiInfo,
          opencode: opencodeInfo,
        });

        setResolvedDirs({
          appConfig: normalizedOverride ?? defaultsRef.current.appConfig,
          claude: claudeInfo.dir || defaultsRef.current.claude,
          codex: codexInfo.dir || defaultsRef.current.codex,
          gemini: geminiInfo.dir || defaultsRef.current.gemini,
          opencode: opencodeInfo.dir || defaultsRef.current.opencode,
        });
      } catch (error) {
        console.error(
          "[useDirectorySettings] Failed to load directory info",
          error,
        );
      } finally {
        if (active) {
          setIsLoading(false);
        }
      }
    };

    void load();
    return () => {
      active = false;
    };
  }, []);

  const updateDirectoryState = useCallback(
    (key: DirectoryKey, value?: string) => {
      const sanitized = sanitizeDir(value);
      if (key === "appConfig") {
        setAppConfigDir(sanitized);
      } else {
        onUpdateSettings(
          key === "claude"
            ? { claudeConfigDir: sanitized }
            : key === "codex"
              ? { codexConfigDir: sanitized }
              : key === "gemini"
                ? { geminiConfigDir: sanitized }
                : { opencodeConfigDir: sanitized },
        );
      }

      setResolvedDirs((prev) => ({
        ...prev,
        [key]: sanitized ?? defaultsRef.current[key],
      }));
    },
    [onUpdateSettings],
  );

  const updateAppConfigDir = useCallback(
    (value?: string) => {
      updateDirectoryState("appConfig", value);
    },
    [updateDirectoryState],
  );

  const updateDirectory = useCallback(
    (app: DirectoryAppId, value?: string) => {
      updateDirectoryState(app, value);
    },
    [updateDirectoryState],
  );

  const browseDirectory = useCallback(
    async (app: DirectoryAppId) => {
      const key: DirectoryKey = app;
      const currentValue =
        key === "claude"
          ? (settings?.claudeConfigDir ?? resolvedDirs.claude)
          : key === "codex"
            ? (settings?.codexConfigDir ?? resolvedDirs.codex)
            : key === "gemini"
              ? (settings?.geminiConfigDir ?? resolvedDirs.gemini)
              : (settings?.opencodeConfigDir ?? resolvedDirs.opencode);

      if (isWeb()) {
        const manual = window.prompt(
          t("settings.manualDirectoryInput", {
            defaultValue: "请输入目录路径",
          }),
          currentValue,
        );
        const sanitized = sanitizeDir(manual ?? undefined);
        if (sanitized) {
          updateDirectoryState(key, sanitized);
        }
        return;
      }

      try {
        const picked = await settingsApi.selectConfigDirectory(currentValue);
        const sanitized = sanitizeDir(picked ?? undefined);
        if (!sanitized) return;
        updateDirectoryState(key, sanitized);
      } catch (error) {
        console.error("[useDirectorySettings] Failed to pick directory", error);
        toast.error(
          t("settings.selectFileFailed", {
            defaultValue: "选择目录失败",
          }),
        );
      }
    },
    [settings, resolvedDirs, t, updateDirectoryState],
  );

  const browseAppConfigDir = useCallback(async () => {
    const currentValue = appConfigDir ?? resolvedDirs.appConfig;
    if (isWeb()) {
      const manual = window.prompt(
        t("settings.manualDirectoryInput", {
          defaultValue: "请输入配置目录路径",
        }),
        currentValue,
      );
      const sanitized = sanitizeDir(manual ?? undefined);
      if (sanitized) {
        updateDirectoryState("appConfig", sanitized);
      }
      return;
    }
    try {
      const picked = await settingsApi.selectConfigDirectory(currentValue);
      const sanitized = sanitizeDir(picked ?? undefined);
      if (!sanitized) return;
      updateDirectoryState("appConfig", sanitized);
    } catch (error) {
      console.error(
        "[useDirectorySettings] Failed to pick app config directory",
        error,
      );
      toast.error(
        t("settings.selectFileFailed", {
          defaultValue: "选择目录失败",
        }),
      );
    }
  }, [appConfigDir, resolvedDirs.appConfig, t, updateDirectoryState]);

  const resetDirectory = useCallback(
    async (app: DirectoryAppId) => {
      const key: DirectoryKey = app;
      if (!defaultsRef.current[key]) {
        const fallback = await computeDefaultConfigDir(app);
        if (fallback) {
          defaultsRef.current = {
            ...defaultsRef.current,
            [key]: fallback,
          };
        }
      }
      updateDirectoryState(key, undefined);
    },
    [updateDirectoryState],
  );

  const resetAppConfigDir = useCallback(async () => {
    if (!defaultsRef.current.appConfig) {
      const fallback = await computeDefaultAppConfigDir();
      if (fallback) {
        defaultsRef.current = {
          ...defaultsRef.current,
          appConfig: fallback,
        };
      }
    }
    updateDirectoryState("appConfig", undefined);
  }, [updateDirectoryState]);

  const resetAllDirectories = useCallback(
    (
      claudeDir?: string,
      codexDir?: string,
      geminiDir?: string,
      opencodeDir?: string,
    ) => {
      setAppConfigDir(initialAppConfigDirRef.current);
      setResolvedDirs({
        appConfig:
          initialAppConfigDirRef.current ?? defaultsRef.current.appConfig,
        claude: claudeDir ?? defaultsRef.current.claude,
        codex: codexDir ?? defaultsRef.current.codex,
        gemini: geminiDir ?? defaultsRef.current.gemini,
        opencode: opencodeDir ?? defaultsRef.current.opencode,
      });
    },
    [],
  );

  const applyWslTemplate = useCallback(
    (distro?: string) => {
      const normalizedDistro = (distro ?? "").trim() || "Ubuntu";
      const usernamePlaceholder = t("settings.wslTemplateUserPlaceholder", {
        defaultValue: "your-username",
      });
      const base = `\\\\wsl$\\${normalizedDistro}\\home\\<${usernamePlaceholder}>`;

      updateDirectoryState("claude", `${base}\\.claude`);
      updateDirectoryState("codex", `${base}\\.codex`);
      updateDirectoryState("gemini", `${base}\\.gemini`);
      updateDirectoryState("opencode", `${base}\\.config\\opencode`);

      toast.success(
        t("settings.wslTemplateApplied", {
          defaultValue:
            "Filled WSL template paths for {{distro}}. Save settings to apply.",
          distro: normalizedDistro,
        }),
      );
    },
    [t, updateDirectoryState],
  );

  return {
    appConfigDir,
    resolvedDirs,
    resolvedDirInfo,
    isLoading,
    initialAppConfigDir: initialAppConfigDirRef.current,
    updateDirectory,
    updateAppConfigDir,
    browseDirectory,
    browseAppConfigDir,
    resetDirectory,
    resetAppConfigDir,
    resetAllDirectories,
    applyWslTemplate,
  };
}
