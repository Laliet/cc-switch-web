import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { BarChart3, Plus, Settings, Edit3, History } from "lucide-react";
import type { Provider } from "@/types";
import type { EnvConflict } from "@/types/env";
import { useProvidersQuery } from "@/lib/query";
import {
  providersApi,
  settingsApi,
  type AppId,
  type ProviderSwitchEvent,
} from "@/lib/api";
import { checkAllEnvConflicts, checkEnvConflicts } from "@/lib/api/env";
import { useProviderActions } from "@/hooks/useProviderActions";
import { useHealthCheck } from "@/hooks/useHealthCheck";
import { extractErrorMessage } from "@/utils/errorUtils";
import {
  clearWebCredentials,
  buildWebAuthHeadersForUrl,
  buildWebApiUrl,
  isWeb,
} from "@/lib/api/adapter";
import { AppSwitcher } from "@/components/AppSwitcher";
import {
  isMcpApp,
  isPromptApp,
  isProviderApp,
  isSkillsApp,
  isUsageApp,
} from "@/config/apps";
import { ProviderList } from "@/components/providers/ProviderList";
import { ClaudeDesktopPanel } from "@/components/providers/ClaudeDesktopPanel";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { UpdateBadge } from "@/components/UpdateBadge";
import { EnvWarningBanner } from "@/components/env/EnvWarningBanner";
import { DeepLinkImportDialog } from "@/components/DeepLinkImportDialog";
import WebLoginDialog from "@/components/WebLoginDialog";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { VisuallyHidden } from "@radix-ui/react-visually-hidden";

const AddProviderDialog = lazy(() =>
  import("@/components/providers/AddProviderDialog").then((module) => ({
    default: module.AddProviderDialog,
  })),
);
const EditProviderDialog = lazy(() =>
  import("@/components/providers/EditProviderDialog").then((module) => ({
    default: module.EditProviderDialog,
  })),
);
const SettingsDialog = lazy(() =>
  import("@/components/settings/SettingsDialog").then((module) => ({
    default: module.SettingsDialog,
  })),
);
const UsageScriptModal = lazy(() => import("@/components/UsageScriptModal"));
const UsageDashboard = lazy(() => import("@/components/usage/UsageDashboard"));
const UnifiedMcpPanel = lazy(() => import("@/components/mcp/UnifiedMcpPanel"));
const PromptPanel = lazy(() => import("@/components/prompts/PromptPanel"));
const SkillsPage = lazy(() =>
  import("@/components/skills/SkillsPage").then((module) => ({
    default: module.SkillsPage,
  })),
);
const SessionManagerPage = lazy(() =>
  import("@/components/sessions/SessionManagerPage").then((module) => ({
    default: module.SessionManagerPage,
  })),
);

async function validateWebCredentials(url: string): Promise<boolean> {
  const headers = buildWebAuthHeadersForUrl(url);
  const response = await fetch(url, {
    method: "GET",
    credentials: "include",
    headers: {
      Accept: "application/json",
      ...headers,
    },
  });

  if (response.status === 401) return false;
  return response.ok;
}

const ACTIVE_APP_STORAGE_KEY = "cc-switch:active-app";
const CONFIG_DIR_WARNING_SHOWN_KEY = "cc-switch:config-dir-warning";

function isSupportedAppId(value: unknown): value is AppId {
  return isProviderApp(value);
}

function readPersistedActiveApp(): AppId {
  if (typeof window === "undefined") return "claude";
  try {
    const stored = window.localStorage.getItem(ACTIVE_APP_STORAGE_KEY);
    return isSupportedAppId(stored) ? stored : "claude";
  } catch {
    return "claude";
  }
}

function AppContent() {
  const { t } = useTranslation();

  const [activeApp, setActiveApp] = useState<AppId>(() =>
    readPersistedActiveApp(),
  );
  const [isEditMode, setIsEditMode] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isAddOpen, setIsAddOpen] = useState(false);
  const [isMcpOpen, setIsMcpOpen] = useState(false);
  const [isPromptOpen, setIsPromptOpen] = useState(false);
  const [isSkillsOpen, setIsSkillsOpen] = useState(false);
  const [isUsageDashboardOpen, setIsUsageDashboardOpen] = useState(false);
  const [isSessionManagerOpen, setIsSessionManagerOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<Provider | null>(null);
  const [usageProvider, setUsageProvider] = useState<Provider | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<Provider | null>(null);
  const [envConflicts, setEnvConflicts] = useState<EnvConflict[]>([]);
  const [showEnvBanner, setShowEnvBanner] = useState(false);
  const [claudeDesktopStatusRefreshKey, setClaudeDesktopStatusRefreshKey] =
    useState(0);
  const promptSupported = isPromptApp(activeApp);
  const mcpSupported = isMcpApp(activeApp);
  const skillsSupported = isSkillsApp(activeApp);
  const usageSupported = isUsageApp(activeApp);

  const { data, isLoading, refetch } = useProvidersQuery(activeApp);
  const providers = useMemo(() => data?.providers ?? {}, [data]);
  const currentProviderId = data?.currentProviderId ?? "";
  const backupProviderId = data?.backupProviderId ?? null;

  const { healthMap, refetch: refetchHealth } = useHealthCheck(
    activeApp,
    providers,
  );
  const lastFailoverCheckRef = useRef<string | null>(null);
  const configDirWarningRef = useRef<Set<string>>(new Set());

  const handleSwitchApp = useCallback((app: AppId) => {
    setUsageProvider(null);
    setActiveApp(app);
    if (typeof window === "undefined") return;
    try {
      window.localStorage.setItem(ACTIVE_APP_STORAGE_KEY, app);
    } catch (error) {
      console.warn("[App] Failed to persist active app", error);
    }
  }, []);

  useEffect(() => {
    if (!promptSupported && isPromptOpen) {
      setIsPromptOpen(false);
    }
    if (!mcpSupported && isMcpOpen) {
      setIsMcpOpen(false);
    }
    if (!skillsSupported && isSkillsOpen) {
      setIsSkillsOpen(false);
    }
    if (!usageSupported && usageProvider) {
      setUsageProvider(null);
    }
  }, [
    isMcpOpen,
    isPromptOpen,
    isSkillsOpen,
    mcpSupported,
    promptSupported,
    skillsSupported,
    usageProvider,
    usageSupported,
  ]);

  // 🎯 使用 useProviderActions Hook 统一管理所有 Provider 操作
  const {
    addProvider,
    updateProvider,
    switchProvider,
    deleteProvider,
    saveUsageScript,
  } = useProviderActions(activeApp);

  const handleSwitchProvider = useCallback(
    async (provider: Provider) => {
      const result = await switchProvider(provider);
      if (activeApp === "claude-desktop") {
        setClaudeDesktopStatusRefreshKey((key) => key + 1);
      }
      return result;
    },
    [activeApp, switchProvider],
  );

  // 监听来自托盘菜单的切换事件
  useEffect(() => {
    let cancelled = false;
    let unsubscribe: (() => void) | undefined;

    const setupListener = async () => {
      try {
        const unlisten = await providersApi.onSwitched(
          async (event: ProviderSwitchEvent) => {
            if (event.appType === activeApp) {
              await refetch();
            }
            if (event.appType === "claude-desktop") {
              setClaudeDesktopStatusRefreshKey((key) => key + 1);
            }
          },
        );

        if (cancelled) {
          unlisten();
          return;
        }

        unsubscribe = unlisten;
      } catch (error) {
        console.error("[App] Failed to subscribe provider switch event", error);
      }
    };

    void setupListener();
    return () => {
      cancelled = true;
      unsubscribe?.();
    };
  }, [activeApp, refetch]);

  // 应用启动时检测所有应用的环境变量冲突
  useEffect(() => {
    const checkEnvOnStartup = async () => {
      try {
        const allConflicts = await checkAllEnvConflicts();
        const flatConflicts = Object.values(allConflicts).flat();

        if (flatConflicts.length > 0) {
          setEnvConflicts(flatConflicts);
          setShowEnvBanner(true);
        }
      } catch (error) {
        console.error(
          "[App] Failed to check environment conflicts on startup:",
          error,
        );
      }
    };

    checkEnvOnStartup();
  }, []);

  useEffect(() => {
    if (!isWeb()) return;

    let cancelled = false;

    const loadConfigDirInfo = async () => {
      try {
        const info = await settingsApi.getConfigDirInfo(activeApp);
        if (cancelled || !info.homeMismatch || info.source === "override") {
          return;
        }

        const warningKey = `${activeApp}:${info.source}:${info.dir}`;
        if (configDirWarningRef.current.has(warningKey)) {
          return;
        }

        configDirWarningRef.current.add(warningKey);
        try {
          const stored = window.sessionStorage.getItem(
            CONFIG_DIR_WARNING_SHOWN_KEY,
          );
          if (stored?.split("|").includes(warningKey)) {
            return;
          }
          const nextValue = stored ? `${stored}|${warningKey}` : warningKey;
          window.sessionStorage.setItem(
            CONFIG_DIR_WARNING_SHOWN_KEY,
            nextValue,
          );
        } catch {
          // ignore session storage failures
        }

        toast.warning(
          t("notifications.webHomeMismatchTitle", {
            defaultValue: "检测到配置目录偏差",
          }),
          {
            description: t("notifications.webHomeMismatch", {
              defaultValue:
                "Web 服务 HOME ({{serviceHome}}) 与账号 HOME ({{accountHome}}) 不一致；当前 {{appName}} 会写入 {{path}}。",
              serviceHome: info.serviceHome ?? "-",
              accountHome: info.accountHome ?? "-",
              appName: t(`apps.${activeApp}`, { defaultValue: activeApp }),
              path: info.dir,
            }),
            duration: 7000,
          },
        );
      } catch (error) {
        console.warn("[App] Failed to load config dir info", error);
      }
    };

    void loadConfigDirInfo();
    return () => {
      cancelled = true;
    };
  }, [activeApp, t]);

  // 切换应用时检测当前应用的环境变量冲突
  useEffect(() => {
    const checkEnvOnSwitch = async () => {
      try {
        const conflicts = await checkEnvConflicts(activeApp);

        if (conflicts.length > 0) {
          // 合并新检测到的冲突
          setEnvConflicts((prev) => {
            const existingKeys = new Set(
              prev.map((c) => `${c.varName}:${c.sourcePath}`),
            );
            const newConflicts = conflicts.filter(
              (c) => !existingKeys.has(`${c.varName}:${c.sourcePath}`),
            );
            return [...prev, ...newConflicts];
          });
          setShowEnvBanner(true);
        }
      } catch (error) {
        console.error(
          "[App] Failed to check environment conflicts on app switch:",
          error,
        );
      }
    };

    checkEnvOnSwitch();
  }, [activeApp]);

  // 打开网站链接
  const handleOpenWebsite = async (url: string) => {
    try {
      await settingsApi.openExternal(url);
    } catch (error) {
      const detail =
        extractErrorMessage(error) ||
        t("notifications.openLinkFailed", {
          defaultValue: "链接打开失败",
        });
      toast.error(detail);
    }
  };

  // 编辑供应商
  const handleEditProvider = async (provider: Provider) => {
    try {
      await updateProvider(provider);
      setEditingProvider(null);
    } catch (error) {
      console.error("[App] Failed to update provider", error);
      const detail = extractErrorMessage(error) || t("common.unknown");
      toast.error(
        t("provider.updateFailed", {
          defaultValue: "更新供应商失败：{{error}}",
          error: detail,
        }),
      );
    }
  };

  // 确认删除供应商
  const handleConfirmDelete = async () => {
    if (!confirmDelete) return;
    try {
      await deleteProvider(confirmDelete.id);
      setConfirmDelete(null);
    } catch (error) {
      console.error("[App] Failed to delete provider", error);
      const detail = extractErrorMessage(error) || t("common.unknown");
      toast.error(
        t("provider.deleteFailed", {
          defaultValue: "删除供应商失败：{{error}}",
          error: detail,
        }),
      );
    }
  };

  // 设置备用供应商
  const handleSetBackup = async (id: string | null) => {
    try {
      await providersApi.setBackup(id, activeApp);
      await refetch();
      const name = id ? (providers[id]?.name ?? id) : t("common.none");
      toast.success(
        t("provider.backupUpdated", {
          defaultValue: "已更新备用供应商为 {{name}}",
          name,
        }),
      );
    } catch (error) {
      const detail = extractErrorMessage(error) || t("common.unknown");
      toast.error(
        t("provider.backupUpdateFailed", {
          defaultValue: "备用供应商更新失败：{{error}}",
          error: detail,
        }),
      );
    }
  };

  // 自动故障切换
  const handleAutoFailover = useCallback(
    async (targetId?: string | null) => {
      try {
        const targetProviderId = targetId ?? backupProviderId;
        if (!targetProviderId || !currentProviderId) return;

        const targetProvider = providers[targetProviderId];
        if (!targetProvider) return;

        const ensureHealthMap = async () => {
          if (
            healthMap[currentProviderId] !== undefined &&
            healthMap[targetProviderId] !== undefined
          ) {
            return healthMap;
          }
          const refreshed = await refetchHealth();
          return refreshed.data ?? healthMap;
        };

        const latestHealthMap = await ensureHealthMap();
        const currentHealth = latestHealthMap?.[currentProviderId];
        const backupHealth = latestHealthMap?.[targetProviderId];

        if (!currentHealth || currentHealth.isHealthy) return;

        if (!backupHealth || !backupHealth.isHealthy) {
          toast.warning(
            t("provider.backupUnavailable", {
              defaultValue: "备用供应商也不可用，保持当前供应商",
            }),
          );
          return;
        }

        await handleSwitchProvider(targetProvider);
        await refetch();
        toast.warning(
          t("provider.autoFailover", {
            defaultValue: "已自动切换到备用供应商：{{name}}",
            name: targetProvider.name,
          }),
        );
      } catch (error) {
        const detail = extractErrorMessage(error) || t("common.unknown");
        toast.error(
          t("provider.autoFailoverFailed", {
            defaultValue: "自动切换备用失败：{{error}}",
            error: detail,
          }),
        );
        return;
      }
    },
    [
      backupProviderId,
      currentProviderId,
      healthMap,
      providers,
      refetch,
      refetchHealth,
      handleSwitchProvider,
      t,
    ],
  );

  // 健康状态变更时触发自动故障切换检查
  useEffect(() => {
    if (!currentProviderId || !backupProviderId) {
      lastFailoverCheckRef.current = null;
      return;
    }

    const currentHealth = healthMap[currentProviderId];
    if (!currentHealth || currentHealth.isHealthy) {
      lastFailoverCheckRef.current = null;
      return;
    }

    const backupHealthStatus =
      (backupProviderId && healthMap[backupProviderId]?.status) || "unknown";
    const statusKey = `${currentProviderId}:${currentHealth.status}:${backupProviderId}:${backupHealthStatus}`;

    if (lastFailoverCheckRef.current === statusKey) return;
    lastFailoverCheckRef.current = statusKey;
    void handleAutoFailover(backupProviderId);
  }, [backupProviderId, currentProviderId, handleAutoFailover, healthMap]);

  // 复制供应商
  const handleDuplicateProvider = async (provider: Provider) => {
    // 1️⃣ 计算新的 sortIndex：如果原供应商有 sortIndex，则复制它
    const newSortIndex =
      provider.sortIndex !== undefined ? provider.sortIndex + 1 : undefined;

    const duplicatedProvider: Omit<Provider, "id" | "createdAt"> = {
      name: `${provider.name} copy`,
      settingsConfig: JSON.parse(JSON.stringify(provider.settingsConfig)), // 深拷贝
      websiteUrl: provider.websiteUrl,
      category: provider.category,
      sortIndex: newSortIndex, // 复制原 sortIndex + 1
      meta: provider.meta
        ? JSON.parse(JSON.stringify(provider.meta))
        : undefined, // 深拷贝
    };

    // 2️⃣ 如果原供应商有 sortIndex，需要将后续所有供应商的 sortIndex +1
    if (provider.sortIndex !== undefined) {
      const updates = Object.values(providers)
        .filter(
          (p) =>
            p.sortIndex !== undefined &&
            p.sortIndex >= newSortIndex! &&
            p.id !== provider.id,
        )
        .map((p) => ({
          id: p.id,
          sortIndex: p.sortIndex! + 1,
        }));

      // 先更新现有供应商的 sortIndex，为新供应商腾出位置
      if (updates.length > 0) {
        try {
          await providersApi.updateSortOrder(updates, activeApp);
        } catch (error) {
          console.error("[App] Failed to update sort order", error);
          toast.error(
            t("provider.sortUpdateFailed", {
              defaultValue: "排序更新失败",
            }),
          );
          return; // 如果排序更新失败，不继续添加
        }
      }
    }

    // 3️⃣ 添加复制的供应商
    try {
      await addProvider(duplicatedProvider);
    } catch (error) {
      console.error("[App] Failed to duplicate provider", error);
      const detail = extractErrorMessage(error) || t("common.unknown");
      toast.error(
        t("provider.duplicateFailed", {
          defaultValue: "复制供应商失败：{{error}}",
          error: detail,
        }),
      );
    }
  };

  // 导入配置成功后刷新
  const handleImportSuccess = async () => {
    await refetch();
    try {
      await providersApi.updateTrayMenu();
    } catch (error) {
      console.error("[App] Failed to refresh tray menu", error);
    }
  };

  return (
    <div className="flex h-screen flex-col bg-gray-50 dark:bg-gray-950">
      {/* 环境变量警告横幅 */}
      {showEnvBanner && envConflicts.length > 0 && (
        <EnvWarningBanner
          conflicts={envConflicts}
          onDismiss={() => setShowEnvBanner(false)}
          onDeleted={async () => {
            // 删除后重新检测
            try {
              const allConflicts = await checkAllEnvConflicts();
              const flatConflicts = Object.values(allConflicts).flat();
              setEnvConflicts(flatConflicts);
              if (flatConflicts.length === 0) {
                setShowEnvBanner(false);
              }
            } catch (error) {
              console.error(
                "[App] Failed to re-check conflicts after deletion:",
                error,
              );
            }
          }}
        />
      )}

      <header className="flex-shrink-0 border-b border-gray-200 bg-white px-4 py-4 dark:border-gray-800 dark:bg-gray-900 sm:px-6">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-1">
            <a
              href="https://github.com/farion1231/cc-switch"
              target="_blank"
              rel="noreferrer"
              className="text-xl font-semibold text-blue-500 transition-colors hover:text-blue-600 dark:text-blue-400 dark:hover:text-blue-300"
            >
              CC Switch
            </a>
            <Button
              variant="ghost"
              size="icon"
              onClick={() => setIsSettingsOpen(true)}
              title={t("common.settings")}
              className="ml-2"
            >
              <Settings className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              onClick={() => setIsEditMode(!isEditMode)}
              title={t(
                isEditMode ? "header.exitEditMode" : "header.enterEditMode",
              )}
              className={
                isEditMode
                  ? "text-blue-500 hover:text-blue-600 dark:text-blue-400 dark:hover:text-blue-300"
                  : ""
              }
            >
              <Edit3 className="h-4 w-4" />
            </Button>
            <UpdateBadge onClick={() => setIsSettingsOpen(true)} />
          </div>

          <div className="flex min-w-0 max-w-full flex-wrap items-center gap-2">
            <div className="flex min-w-0 max-w-full flex-col gap-1">
              <AppSwitcher activeApp={activeApp} onSwitch={handleSwitchApp} />
              <span className="text-xs text-muted-foreground">
                {t("apps.scopeHint", {
                  defaultValue:
                    "Switches the current management view only. Live config changes happen when you switch providers.",
                })}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground">
                {t("provider.backupLabel", { defaultValue: "备用" })}
              </span>
              <Select
                value={backupProviderId ?? "none"}
                onValueChange={(val) => {
                  const next = val === "none" ? null : val;
                  void handleSetBackup(next);
                }}
              >
                <SelectTrigger className="w-[180px]">
                  <SelectValue
                    placeholder={t("provider.backupPlaceholder", {
                      defaultValue: "选择备用供应商",
                    })}
                  />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">
                    {t("common.none", { defaultValue: "无" })}
                  </SelectItem>
                  {Object.values(providers).map((p) => (
                    <SelectItem key={p.id} value={p.id}>
                      {p.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            {promptSupported ? (
              <Button
                variant="mcp"
                onClick={() => setIsPromptOpen(true)}
                className="min-w-[80px]"
              >
                {t("prompts.manage")}
              </Button>
            ) : null}
            {mcpSupported ? (
              <Button
                variant="mcp"
                onClick={() => setIsMcpOpen(true)}
                className="min-w-[80px]"
              >
                MCP
              </Button>
            ) : null}
            {skillsSupported ? (
              <Button
                variant="mcp"
                onClick={() => setIsSkillsOpen(true)}
                className="min-w-[80px]"
              >
                {t("skills.manage")}
              </Button>
            ) : null}
            <Button
              variant="outline"
              onClick={() => setIsSessionManagerOpen(true)}
              className="min-w-[92px]"
            >
              <History className="h-4 w-4" />
              {t("sessionManager.manage")}
            </Button>
            <Button
              variant="outline"
              onClick={() => setIsUsageDashboardOpen(true)}
              className="min-w-[92px]"
            >
              <BarChart3 className="h-4 w-4" />
              {t("usage.dashboard", { defaultValue: "用量" })}
            </Button>
            <Button onClick={() => setIsAddOpen(true)}>
              <Plus className="h-4 w-4" />
              {t("header.addProvider")}
            </Button>
          </div>
        </div>
      </header>

      <main className="flex-1 overflow-y-scroll">
        <div className="mx-auto max-w-4xl px-6 py-6">
          {activeApp === "claude-desktop" ? (
            <ClaudeDesktopPanel
              onProvidersChanged={handleImportSuccess}
              refreshKey={claudeDesktopStatusRefreshKey}
            />
          ) : null}
          <ProviderList
            providers={providers}
            currentProviderId={currentProviderId}
            backupProviderId={backupProviderId}
            healthMap={healthMap}
            appId={activeApp}
            isLoading={isLoading}
            isEditMode={isEditMode}
            onSwitch={handleSwitchProvider}
            onEdit={setEditingProvider}
            onDelete={setConfirmDelete}
            onDuplicate={handleDuplicateProvider}
            onConfigureUsage={setUsageProvider}
            onOpenWebsite={handleOpenWebsite}
            onCreate={() => setIsAddOpen(true)}
            onAutoFailover={handleAutoFailover}
            onOmoDisabled={async () => {
              await refetch();
            }}
          />
        </div>
      </main>

      <Suspense fallback={null}>
        {isAddOpen ? (
          <AddProviderDialog
            open={isAddOpen}
            onOpenChange={setIsAddOpen}
            appId={activeApp}
            onSubmit={addProvider}
          />
        ) : null}

        {editingProvider ? (
          <EditProviderDialog
            open={Boolean(editingProvider)}
            provider={editingProvider}
            onOpenChange={(open) => {
              if (!open) {
                setEditingProvider(null);
              }
            }}
            onSubmit={handleEditProvider}
            appId={activeApp}
          />
        ) : null}
      </Suspense>

      {usageProvider && usageSupported ? (
        <Suspense fallback={null}>
          <UsageScriptModal
            provider={usageProvider}
            appId={activeApp}
            isOpen={Boolean(usageProvider)}
            onClose={() => setUsageProvider(null)}
            onSave={(script) => {
              void saveUsageScript(usageProvider, script);
            }}
          />
        </Suspense>
      ) : null}

      <ConfirmDialog
        isOpen={Boolean(confirmDelete)}
        title={t("confirm.deleteProvider")}
        message={
          confirmDelete
            ? t("confirm.deleteProviderMessage", {
                name: confirmDelete.name,
              })
            : ""
        }
        onConfirm={() => void handleConfirmDelete()}
        onCancel={() => setConfirmDelete(null)}
      />

      {isSettingsOpen ? (
        <Suspense fallback={null}>
          <SettingsDialog
            open={isSettingsOpen}
            onOpenChange={setIsSettingsOpen}
            onImportSuccess={handleImportSuccess}
            onProvidersChanged={handleImportSuccess}
          />
        </Suspense>
      ) : null}

      <Dialog
        open={isUsageDashboardOpen}
        onOpenChange={setIsUsageDashboardOpen}
      >
        <DialogContent className="max-w-[min(1200px,96vw)]">
          <DialogHeader>
            <DialogTitle>
              {t("usage.title", { defaultValue: "Usage Dashboard" })}
            </DialogTitle>
            <DialogDescription>
              {t("usage.subtitle", {
                defaultValue: "Request logs, pricing, and cost analytics.",
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="overflow-y-auto px-6 py-5">
            <Suspense fallback={null}>
              <UsageDashboard />
            </Suspense>
          </div>
        </DialogContent>
      </Dialog>

      {isPromptOpen && promptSupported ? (
        <Suspense fallback={null}>
          <PromptPanel
            open={isPromptOpen}
            onOpenChange={setIsPromptOpen}
            appId={activeApp}
          />
        </Suspense>
      ) : null}

      {isMcpOpen && mcpSupported ? (
        <Suspense fallback={null}>
          <UnifiedMcpPanel open={isMcpOpen} onOpenChange={setIsMcpOpen} />
        </Suspense>
      ) : null}

      {isSkillsOpen && skillsSupported ? (
        <Dialog open={isSkillsOpen} onOpenChange={setIsSkillsOpen}>
          <DialogContent className="max-w-4xl max-h-[85vh] min-h-[600px] flex flex-col p-0">
            <DialogHeader className="sr-only">
              <VisuallyHidden>
                <DialogTitle>{t("skills.title")}</DialogTitle>
                <DialogDescription>
                  {t("skills.description", {
                    defaultValue: "管理技能库条目并配置仓库来源。",
                  })}
                </DialogDescription>
              </VisuallyHidden>
            </DialogHeader>
            <Suspense fallback={null}>
              <SkillsPage
                onClose={() => setIsSkillsOpen(false)}
                appId={activeApp}
              />
            </Suspense>
          </DialogContent>
        </Dialog>
      ) : null}
      {isSessionManagerOpen ? (
        <Suspense fallback={null}>
          <SessionManagerPage
            open={isSessionManagerOpen}
            onOpenChange={setIsSessionManagerOpen}
          />
        </Suspense>
      ) : null}
      <DeepLinkImportDialog />
    </div>
  );
}

function App() {
  const web = isWeb();
  const [isAuthed, setIsAuthed] = useState(!web);
  const [isChecking, setIsChecking] = useState(web);

  useEffect(() => {
    if (!web) return;

    let cancelled = false;
    const check = async () => {
      try {
        const url = buildWebApiUrl("/settings");
        const ok = await validateWebCredentials(url);
        if (cancelled) return;

        if (!ok) {
          clearWebCredentials();
          setIsAuthed(false);
          return;
        }

        setIsAuthed(true);
      } catch {
        if (!cancelled) setIsAuthed(false);
      } finally {
        if (!cancelled) setIsChecking(false);
      }
    };

    void check();
    return () => {
      cancelled = true;
    };
  }, [web]);

  if (web && (isChecking || !isAuthed)) {
    return (
      <div className="min-h-screen bg-background">
        <WebLoginDialog
          open={!isChecking && !isAuthed}
          onLoginSuccess={() => setIsAuthed(true)}
        />
      </div>
    );
  }

  return <AppContent />;
}

export default App;
