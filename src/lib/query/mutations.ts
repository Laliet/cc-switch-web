import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { providersApi, settingsApi, type AppId } from "@/lib/api";
import type { Provider, Settings } from "@/types";
import { extractErrorMessage } from "@/utils/errorUtils";
import generateUUID from "@/utils/uuid";

const getPathSeparator = (dir: string): string => {
  if (dir.includes("\\") && !dir.includes("/")) return "\\";
  return "/";
};

const joinDisplayPath = (dir: string, fileName: string): string => {
  const trimmed = dir.trim();
  if (!trimmed) return fileName;
  if (trimmed.endsWith("/") || trimmed.endsWith("\\")) {
    return `${trimmed}${fileName}`;
  }
  return `${trimmed}${getPathSeparator(trimmed)}${fileName}`;
};

const getLiveConfigPath = (appId: AppId, dir: string): string => {
  switch (appId) {
    case "claude":
      return joinDisplayPath(dir, "settings.json");
    case "codex":
      return joinDisplayPath(dir, "config.toml");
    case "gemini":
      return joinDisplayPath(dir, ".env");
    case "opencode":
      return joinDisplayPath(dir, "opencode.json");
    case "omo":
      return joinDisplayPath(dir, "oh-my-openagent.jsonc");
    case "omo-slim":
      return joinDisplayPath(dir, "oh-my-opencode-slim.jsonc");
    default:
      return dir;
  }
};

export const useAddProviderMutation = (appId: AppId) => {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: async (providerInput: Omit<Provider, "id">) => {
      const newProvider: Provider = {
        ...providerInput,
        id: generateUUID(),
        createdAt: Date.now(),
      };
      await providersApi.add(newProvider, appId);
      return newProvider;
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["providers", appId] });

      // 更新托盘菜单（失败不影响主操作）
      try {
        await providersApi.updateTrayMenu();
      } catch (trayError) {
        console.error(
          "Failed to update tray menu after adding provider",
          trayError,
        );
      }

      toast.success(
        t("notifications.providerAdded", {
          defaultValue: "供应商已添加",
        }),
      );
    },
    onError: (error: Error) => {
      toast.error(
        t("notifications.addFailed", {
          defaultValue: "添加供应商失败: {{error}}",
          error: error.message,
        }),
      );
    },
  });
};

export const useUpdateProviderMutation = (appId: AppId) => {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: async (provider: Provider) => {
      await providersApi.update(provider, appId);
      return provider;
    },
    onSuccess: async (provider) => {
      await queryClient.invalidateQueries({ queryKey: ["providers", appId] });

      try {
        const currentProviderId = await providersApi.getCurrent(appId);
        if (currentProviderId === provider.id) {
          const info = await settingsApi.getConfigDirInfo(appId);
          toast.success(
            t("notifications.updateSuccess", {
              defaultValue: "供应商更新成功",
            }),
            {
              description: t("notifications.updateCurrentProviderWithPath", {
                defaultValue: "当前 provider 已写入 {{path}}。",
                path: getLiveConfigPath(appId, info.dir),
              }),
            },
          );
          return;
        }
      } catch (error) {
        console.warn(
          "[mutations] Failed to resolve current live config path after update",
          error,
        );
      }

      toast.success(
        t("notifications.updateSuccess", {
          defaultValue: "供应商更新成功",
        }),
        {
          description: t("notifications.updateStoredOnly", {
            defaultValue:
              "仅更新了配置库；切换为当前 provider 后才会写入 live 配置。",
          }),
        },
      );
    },
    onError: (error: Error) => {
      toast.error(
        t("notifications.updateFailed", {
          defaultValue: "更新供应商失败: {{error}}",
          error: error.message,
        }),
      );
    },
  });
};

export const useDeleteProviderMutation = (appId: AppId) => {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: async (providerId: string) => {
      await providersApi.delete(providerId, appId);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["providers", appId] });

      // 更新托盘菜单（失败不影响主操作）
      try {
        await providersApi.updateTrayMenu();
      } catch (trayError) {
        console.error(
          "Failed to update tray menu after deleting provider",
          trayError,
        );
      }

      toast.success(
        t("notifications.deleteSuccess", {
          defaultValue: "供应商已删除",
        }),
      );
    },
    onError: (error: Error) => {
      toast.error(
        t("notifications.deleteFailed", {
          defaultValue: "删除供应商失败: {{error}}",
          error: error.message,
        }),
      );
    },
  });
};

export const useSwitchProviderMutation = (appId: AppId) => {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: async (providerId: string) => {
      return await providersApi.switch(providerId, appId);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["providers", appId] });

      // 更新托盘菜单（失败不影响主操作）
      try {
        await providersApi.updateTrayMenu();
      } catch (trayError) {
        console.error(
          "Failed to update tray menu after switching provider",
          trayError,
        );
      }

      let description: string | undefined;
      try {
        if (appId === "claude-desktop") {
          description = t("notifications.switchClaudeDesktopRestart", {
            defaultValue:
              "已写入 Claude Desktop 3P profile。如未生效，请完全退出并重新打开 Claude Desktop。",
          });
        } else {
          const info = await settingsApi.getConfigDirInfo(appId);
          description = t("notifications.switchSuccessWithPath", {
            defaultValue:
              "已写入 {{path}}。如未生效，请重启 {{appName}} 终端。",
            path: getLiveConfigPath(appId, info.dir),
            appName: t(`apps.${appId}`, { defaultValue: appId }),
          });
        }
      } catch (error) {
        console.warn(
          "[mutations] Failed to resolve live config path after switch",
          error,
        );
      }

      toast.success(
        t("notifications.switchSuccessTitle", {
          defaultValue: "切换供应商成功",
        }),
        description ? { description } : undefined,
      );
    },
    onError: (error: Error) => {
      const detail = extractErrorMessage(error) || t("common.unknown");

      // 标题与详情分离，便于扫描 + 一键复制
      toast.error(
        t("notifications.switchFailedTitle", { defaultValue: "切换失败" }),
        {
          description: t("notifications.switchFailed", {
            defaultValue: "切换失败：{{error}}",
            error: detail,
          }),
          duration: 6000,
          action: {
            label: t("common.copy", { defaultValue: "复制" }),
            onClick: () => {
              navigator.clipboard?.writeText(detail).catch(() => undefined);
            },
          },
        },
      );
    },
  });
};

export const useSaveSettingsMutation = () => {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: async (settings: Settings) => {
      await settingsApi.save(settings);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["settings"] });
      toast.success(
        t("notifications.settingsSaved", {
          defaultValue: "设置已保存",
        }),
      );
    },
    onError: (error: Error) => {
      toast.error(
        t("notifications.settingsSaveFailed", {
          defaultValue: "保存设置失败: {{error}}",
          error: error.message,
        }),
      );
    },
  });
};
