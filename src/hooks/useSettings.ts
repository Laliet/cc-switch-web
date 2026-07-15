import { useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { providersApi, settingsApi } from "@/lib/api";
import { isWeb } from "@/lib/api/adapter";
import type { DirectoryAppId } from "@/config/apps";
import { syncCurrentProvidersLiveSafe } from "@/utils/postChangeSync";
import {
  useCapabilitiesQuery,
  useSettingsQuery,
  useSaveSettingsMutation,
} from "@/lib/query";
import type { Settings } from "@/types";
import { useSettingsForm, type SettingsFormState } from "./useSettingsForm";
import {
  useDirectorySettings,
  type ResolvedDirectories,
  type ResolvedDirectoryInfoMap,
} from "./useDirectorySettings";
import { useSettingsMetadata } from "./useSettingsMetadata";

type Language = "zh" | "en";

interface SaveResult {
  requiresRestart: boolean;
}

export interface UseSettingsResult {
  settings: SettingsFormState | null;
  isLoading: boolean;
  isSaving: boolean;
  isPortable: boolean;
  appConfigDir?: string;
  resolvedDirs: ResolvedDirectories;
  resolvedDirInfo: ResolvedDirectoryInfoMap;
  requiresRestart: boolean;
  updateSettings: (updates: Partial<SettingsFormState>) => void;
  updateDirectory: (app: DirectoryAppId, value?: string) => void;
  updateAppConfigDir: (value?: string) => void;
  browseDirectory: (app: DirectoryAppId) => Promise<void>;
  browseAppConfigDir: () => Promise<void>;
  resetDirectory: (app: DirectoryAppId) => Promise<void>;
  resetAppConfigDir: () => Promise<void>;
  applyWslTemplate: (distro?: string) => void;
  saveSettings: () => Promise<SaveResult | null>;
  resetSettings: () => void;
  acknowledgeRestart: () => void;
}

export type { SettingsFormState, ResolvedDirectories };
export type { ResolvedDirectoryInfoMap };

const sanitizeDir = (value?: string | null): string | undefined => {
  if (!value) return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
};

/**
 * useSettings - 组合层
 * 负责：
 * - 组合 useSettingsForm、useDirectorySettings、useSettingsMetadata
 * - 保存设置逻辑
 * - 重置设置逻辑
 */
export function useSettings(): UseSettingsResult {
  const { t } = useTranslation();
  const { data } = useSettingsQuery();
  const saveMutation = useSaveSettingsMutation();
  const { data: capabilities } = useCapabilitiesQuery();

  // 1️⃣ 表单状态管理
  const {
    settings,
    isLoading: isFormLoading,
    initialLanguage,
    updateSettings,
    resetSettings: resetForm,
    syncLanguage,
  } = useSettingsForm();

  // 2️⃣ 目录管理
  const {
    appConfigDir,
    resolvedDirs,
    resolvedDirInfo,
    isLoading: isDirectoryLoading,
    initialAppConfigDir,
    updateDirectory,
    updateAppConfigDir,
    browseDirectory,
    browseAppConfigDir,
    resetDirectory,
    resetAppConfigDir,
    resetAllDirectories,
    applyWslTemplate,
  } = useDirectorySettings({
    settings,
    onUpdateSettings: updateSettings,
  });

  // 3️⃣ 元数据管理
  const {
    isPortable,
    requiresRestart,
    isLoading: isMetadataLoading,
    acknowledgeRestart,
    setRequiresRestart,
  } = useSettingsMetadata();

  // 重置设置
  const resetSettings = useCallback(() => {
    resetForm(data ?? null);
    syncLanguage(initialLanguage);
    resetAllDirectories(
      sanitizeDir(data?.claudeConfigDir),
      sanitizeDir(data?.codexConfigDir),
      sanitizeDir(data?.geminiConfigDir),
      sanitizeDir(data?.opencodeConfigDir),
    );
    setRequiresRestart(false);
  }, [
    data,
    initialLanguage,
    resetForm,
    syncLanguage,
    resetAllDirectories,
    setRequiresRestart,
  ]);

  // 保存设置
  const saveSettings = useCallback(async (): Promise<SaveResult | null> => {
    if (!settings) return null;
    try {
      const sanitizedAppDir = sanitizeDir(appConfigDir);
      const sanitizedClaudeDir = sanitizeDir(settings.claudeConfigDir);
      const sanitizedCodexDir = sanitizeDir(settings.codexConfigDir);
      const sanitizedGeminiDir = sanitizeDir(settings.geminiConfigDir);
      const sanitizedOpencodeDir = sanitizeDir(settings.opencodeConfigDir);
      const previousAppDir = initialAppConfigDir;
      const previousClaudeDir = sanitizeDir(data?.claudeConfigDir);
      const previousCodexDir = sanitizeDir(data?.codexConfigDir);
      const previousGeminiDir = sanitizeDir(data?.geminiConfigDir);
      const previousOpencodeDir = sanitizeDir(data?.opencodeConfigDir);

      const payload: Settings = {
        ...settings,
        claudeConfigDir: sanitizedClaudeDir,
        codexConfigDir: sanitizedCodexDir,
        geminiConfigDir: sanitizedGeminiDir,
        opencodeConfigDir: sanitizedOpencodeDir,
        language: settings.language,
      };

      await saveMutation.mutateAsync(payload);

      if (!isWeb() && capabilities?.features.configDirOverride === true) {
        await settingsApi.setAppConfigDirOverride(sanitizedAppDir ?? null);
      }

      if (capabilities?.features.claudePluginIntegration === true) {
        try {
          if (payload.enableClaudePluginIntegration) {
            await settingsApi.applyClaudePluginConfig({ official: false });
          } else {
            await settingsApi.applyClaudePluginConfig({ official: true });
          }
        } catch (error) {
          console.warn(
            "[useSettings] Failed to sync Claude plugin config",
            error,
          );
          toast.error(
            t("notifications.syncClaudePluginFailed", {
              defaultValue: "同步 Claude 插件失败",
            }),
          );
        }
      }

      try {
        if (typeof window !== "undefined") {
          window.localStorage.setItem("language", payload.language as Language);
        }
      } catch (error) {
        console.warn(
          "[useSettings] Failed to persist language preference",
          error,
        );
      }

      if (capabilities?.features.tray === true) {
        try {
          await providersApi.updateTrayMenu();
        } catch (error) {
          console.warn("[useSettings] Failed to refresh tray menu", error);
        }
      }

      // 如果 Claude/Codex/Gemini 的目录覆盖发生变化，则立即将“当前使用的供应商”写回对应应用的 live 配置
      const claudeDirChanged = sanitizedClaudeDir !== previousClaudeDir;
      const codexDirChanged = sanitizedCodexDir !== previousCodexDir;
      const geminiDirChanged = sanitizedGeminiDir !== previousGeminiDir;
      const opencodeDirChanged = sanitizedOpencodeDir !== previousOpencodeDir;
      if (
        claudeDirChanged ||
        codexDirChanged ||
        geminiDirChanged ||
        opencodeDirChanged
      ) {
        const syncResult = await syncCurrentProvidersLiveSafe();
        if (!syncResult.ok) {
          console.warn(
            "[useSettings] Failed to sync current providers after directory change",
            syncResult.error,
          );
        }
      }

      const appDirChanged = sanitizedAppDir !== (previousAppDir ?? undefined);
      setRequiresRestart(appDirChanged);

      return { requiresRestart: appDirChanged };
    } catch (error) {
      console.error("[useSettings] Failed to save settings", error);
      throw error;
    }
  }, [
    appConfigDir,
    capabilities?.features.configDirOverride,
    capabilities?.features.claudePluginIntegration,
    capabilities?.features.tray,
    data,
    initialAppConfigDir,
    saveMutation,
    settings,
    setRequiresRestart,
    t,
  ]);

  const isLoading = useMemo(
    () => isFormLoading || isDirectoryLoading || isMetadataLoading,
    [isFormLoading, isDirectoryLoading, isMetadataLoading],
  );

  return {
    settings,
    isLoading,
    isSaving: saveMutation.isPending,
    isPortable,
    appConfigDir,
    resolvedDirs,
    resolvedDirInfo,
    requiresRestart,
    updateSettings,
    updateDirectory,
    updateAppConfigDir,
    browseDirectory,
    browseAppConfigDir,
    resetDirectory,
    resetAppConfigDir,
    applyWslTemplate,
    saveSettings,
    resetSettings,
    acknowledgeRestart,
  };
}
