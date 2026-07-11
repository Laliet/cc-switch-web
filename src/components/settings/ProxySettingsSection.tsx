import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ChevronDown,
  ChevronUp,
  Loader2,
  Plus,
  Play,
  RotateCcw,
  Square,
  TestTube2,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { providersApi, settingsApi } from "@/lib/api";
import type {
  FailoverQueueItem,
  Provider,
  ProxyAppId,
  ProxyRecentLog,
  ProxyRouteAppId,
  ProxySettings,
  ProxyStatus,
} from "@/types";

const PROXY_ROUTE_APPS: ProxyRouteAppId[] = [
  "claude",
  "claude-desktop",
  "codex",
  "gemini",
  "opencode",
];
const PROXY_TAKEOVER_APPS: ProxyAppId[] = [
  "claude",
  "codex",
  "gemini",
  "opencode",
];
const PROXY_TOAST_DURATION = 1800;
const PROXY_APP_NUMERIC_FIELDS = [
  [
    "streamingFirstByteTimeout",
    "firstByteTimeout",
    "首字超时（秒）",
    1,
    120,
    90,
  ],
  ["streamingIdleTimeout", "idleTimeout", "流式 idle 超时（秒）", 0, 600, 120],
  [
    "nonStreamingTimeout",
    "nonStreamingTimeout",
    "非流式总超时（秒）",
    60,
    1200,
    600,
  ],
  ["circuitFailureThreshold", "failureThreshold", "失败阈值", 1, 20, 3],
  ["circuitRecoveryThreshold", "recoveryThreshold", "恢复阈值", 1, 10, 2],
  ["circuitRecoveryWaitSeconds", "recoveryWait", "恢复等待（秒）", 1, 300, 60],
  [
    "circuitErrorRateThreshold",
    "errorRateThreshold",
    "错误率阈值（%）",
    1,
    100,
    80,
  ],
  ["circuitMinRequests", "minRequests", "最小请求数", 5, 100, 10],
] as const;

function providerHealthLabel(state?: string) {
  switch (state) {
    case "open":
      return "Open";
    case "half_open":
      return "Half-open";
    case "healthy":
      return "Healthy";
    default:
      return state ?? "";
  }
}

function providerHealthClass(state?: string) {
  switch (state) {
    case "open":
      return "border-red-200 bg-red-50 text-red-700 dark:border-red-500/30 dark:bg-red-500/10 dark:text-red-300";
    case "half_open":
      return "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300";
    case "healthy":
      return "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-300";
    default:
      return "border-muted bg-muted text-muted-foreground";
  }
}

interface ProxySettingsSectionProps {
  value: ProxySettings;
  onChange: (value: ProxySettings) => void;
}

export function ProxySettingsSection({
  value,
  onChange,
}: ProxySettingsSectionProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ProxyStatus | null>(null);
  const [recentLogs, setRecentLogs] = useState<ProxyRecentLog[]>([]);
  const [failoverApp, setFailoverApp] = useState<ProxyRouteAppId>("claude");
  const [failoverQueue, setFailoverQueue] = useState<FailoverQueueItem[]>([]);
  const [failoverProviders, setFailoverProviders] = useState<
    Record<string, Provider>
  >({});
  const [failoverProviderId, setFailoverProviderId] = useState("");
  const [failoverLoading, setFailoverLoading] = useState(false);
  const [logsOpen, setLogsOpen] = useState(false);
  const [logsLoading, setLogsLoading] = useState(false);
  const [busyAction, setBusyAction] = useState<
    | "load"
    | "start"
    | "stop"
    | "test"
    | `test:${ProxyRouteAppId}`
    | "restore"
    | `takeover:${ProxyAppId}`
    | `reset:${ProxyAppId}:${string}`
    | "failover"
    | null
  >(null);
  const takeoverInFlightRef = useRef<Set<ProxyAppId>>(new Set());

  const listenUrl = useMemo(() => {
    if (status?.listenUrl) return status.listenUrl;
    return `http://${value.host || "127.0.0.1"}:${value.port || 3456}`;
  }, [status?.listenUrl, value.host, value.port]);

  const loadStatus = useCallback(async () => {
    setBusyAction((current) => current ?? "load");
    try {
      setStatus(await settingsApi.getProxyStatus());
    } catch (error) {
      console.warn("Failed to load proxy status", error);
    } finally {
      setBusyAction((current) => (current === "load" ? null : current));
    }
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const loadFailoverQueue = useCallback(async () => {
    setFailoverLoading(true);
    try {
      const [queue, providerMap] = await Promise.all([
        settingsApi.getFailoverQueue(failoverApp),
        providersApi.getAll(failoverApp),
      ]);
      setFailoverQueue(queue);
      setFailoverProviders(providerMap);
      if (
        failoverProviderId &&
        (!providerMap[failoverProviderId] ||
          queue.some((item) => item.providerId === failoverProviderId))
      ) {
        setFailoverProviderId("");
      }
    } catch (error) {
      console.warn("Failed to load failover queue", error);
      setFailoverQueue([]);
      setFailoverProviders({});
    } finally {
      setFailoverLoading(false);
    }
  }, [failoverApp, failoverProviderId]);

  useEffect(() => {
    void loadFailoverQueue();
  }, [loadFailoverQueue]);

  const update = (updates: Partial<ProxySettings>) => {
    onChange({ ...value, ...updates });
  };

  const proxyConfigApp = (app: ProxyRouteAppId): ProxyAppId =>
    app === "claude-desktop" ? "claude" : app;

  const updateApp = (
    app: ProxyRouteAppId,
    updates: Partial<ProxySettings["apps"][ProxyAppId]>,
  ) => {
    const configApp = proxyConfigApp(app);
    onChange({
      ...value,
      apps: {
        ...value.apps,
        [configApp]: {
          ...value.apps[configApp],
          ...updates,
        },
      },
    });
  };

  const loadRecentLogs = async () => {
    setLogsLoading(true);
    try {
      setRecentLogs(await settingsApi.getProxyRecentLogs());
    } catch (error) {
      console.warn("Failed to load proxy recent logs", error);
      setRecentLogs([]);
    } finally {
      setLogsLoading(false);
    }
  };

  const toggleLogs = async () => {
    const nextOpen = !logsOpen;
    setLogsOpen(nextOpen);
    if (nextOpen) {
      await loadRecentLogs();
    }
  };

  const validateBeforeStart = () => {
    if (!value.host.trim()) {
      toast.error(
        t("settings.proxy.validation.hostRequired", {
          defaultValue: "请输入代理监听地址",
        }),
      );
      return false;
    }
    if (!Number.isInteger(value.port) || value.port < 1 || value.port > 65535) {
      toast.error(
        t("settings.proxy.validation.portInvalid", {
          defaultValue: "代理端口必须在 1-65535 之间",
        }),
      );
      return false;
    }
    if (value.host.trim() === "0.0.0.0" && !value.enabled) {
      toast.error(
        t("settings.proxy.validation.publicBindRequiresEnable", {
          defaultValue: "监听 0.0.0.0 前请先启用代理并确认风险",
        }),
      );
      return false;
    }
    return true;
  };

  const handleStart = async () => {
    if (!validateBeforeStart()) return;
    if (status?.running) {
      await loadStatus();
      toast.info(
        t("settings.proxy.alreadyRunning", {
          defaultValue: "代理已在运行",
        }),
        { id: "proxy-start", duration: PROXY_TOAST_DURATION },
      );
      return;
    }
    setBusyAction("start");
    try {
      const nextStatus = await settingsApi.startProxy({
        ...value,
        enabled: true,
      });
      update({ enabled: true });
      setStatus(nextStatus);
      toast.success(
        t("settings.proxy.started", { defaultValue: "代理已启动" }),
        { id: "proxy-start", duration: PROXY_TOAST_DURATION },
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.proxy.startFailed", { defaultValue: "代理启动失败" }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const handleStop = async () => {
    setBusyAction("stop");
    try {
      const nextStatus = await settingsApi.stopProxy();
      update({ enabled: false });
      setStatus(nextStatus);
      toast.success(
        t("settings.proxy.stopped", { defaultValue: "代理已停止" }),
        { id: "proxy-stop", duration: PROXY_TOAST_DURATION },
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.proxy.stopFailed", { defaultValue: "代理停止失败" }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const handleRestore = async () => {
    setBusyAction("restore");
    try {
      const nextStatus = await settingsApi.restoreProxy();
      onChange({
        ...value,
        liveTakeoverActive: false,
        apps: Object.fromEntries(
          PROXY_TAKEOVER_APPS.map((app) => [
            app,
            { ...value.apps[app], enabled: false },
          ]),
        ) as ProxySettings["apps"],
      });
      setStatus(nextStatus);
      toast.success(
        t("settings.proxy.restored", { defaultValue: "接管配置已恢复" }),
        { id: "proxy-restore", duration: PROXY_TOAST_DURATION },
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.proxy.restoreFailed", {
          defaultValue: "恢复接管配置失败",
        }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const handleTakeoverChange = async (app: ProxyAppId, enabled: boolean) => {
    if (takeoverInFlightRef.current.has(app)) return;
    const currentEnabled = value.apps[app]?.enabled ?? false;
    if (currentEnabled === enabled) return;
    if (!enabled && (value.apps[app]?.autoFailoverEnabled ?? false)) {
      toast.error(
        t("settings.proxy.failoverTakeoverRequired", {
          defaultValue: "请先关闭自动故障切换，再关闭代理接管",
        }),
        {
          description: t(`apps.${app}`, { defaultValue: app }),
        },
      );
      return;
    }

    takeoverInFlightRef.current.add(app);
    updateApp(app, { enabled });
    setBusyAction(`takeover:${app}`);
    try {
      const result = await settingsApi.setProxyTakeover(app, enabled);
      setStatus(result.status);
      toast.success(
        enabled
          ? t("settings.proxy.takeoverEnabled", {
              defaultValue: "接管已开启",
            })
          : t("settings.proxy.takeoverDisabled", {
              defaultValue: "接管已关闭",
            }),
        {
          id: `proxy-takeover-${app}`,
          duration: PROXY_TOAST_DURATION,
          description: t(`apps.${app}`, { defaultValue: app }),
        },
      );
    } catch (error) {
      updateApp(app, { enabled: !enabled });
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.proxy.takeoverFailed", {
          defaultValue: "更新接管状态失败",
        }),
        { description: message },
      );
    } finally {
      takeoverInFlightRef.current.delete(app);
      setBusyAction(null);
    }
  };

  const handleResetProviderCircuit = async (
    app: ProxyAppId,
    providerId: string,
  ) => {
    setBusyAction(`reset:${app}:${providerId}`);
    try {
      setStatus(await settingsApi.resetProviderCircuit(app, providerId));
      toast.success(
        t("settings.proxy.circuitReset", {
          defaultValue: "熔断状态已重置",
        }),
        { description: t(`apps.${app}`, { defaultValue: app }) },
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.proxy.circuitResetFailed", {
          defaultValue: "重置熔断状态失败",
        }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const handleTest = async (app?: ProxyRouteAppId) => {
    const testApp = app ?? value.bindApp;
    setBusyAction(app ? `test:${app}` : "test");
    try {
      const result = await settingsApi.testProxy({
        ...value,
        bindApp: testApp,
      });
      toast.success(
        t("settings.proxy.testSuccess", { defaultValue: "代理配置有效" }),
        {
          description:
            result.baseUrl ||
            t("settings.proxy.testedApp", {
              defaultValue: "已测试当前客户端",
              app: t(`apps.${testApp}`, { defaultValue: testApp }),
            }) ||
            result.message,
        },
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.proxy.testFailed", { defaultValue: "代理配置无效" }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const handleAddFailoverProvider = async () => {
    if (!failoverProviderId) return;
    setBusyAction("failover");
    try {
      setFailoverQueue(
        await settingsApi.addFailoverProvider(failoverApp, failoverProviderId),
      );
      setFailoverProviderId("");
      toast.success(
        t("settings.proxy.failoverQueueSaved", {
          defaultValue: "故障切换队列已更新",
        }),
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.proxy.failoverQueueFailed", {
          defaultValue: "更新故障切换队列失败",
        }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const handleRemoveFailoverProvider = async (providerId: string) => {
    setBusyAction("failover");
    try {
      setFailoverQueue(
        await settingsApi.removeFailoverProvider(failoverApp, providerId),
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.proxy.failoverQueueFailed", {
          defaultValue: "更新故障切换队列失败",
        }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const handleMoveFailoverProvider = async (
    providerId: string,
    direction: -1 | 1,
  ) => {
    const currentIndex = failoverQueue.findIndex(
      (item) => item.providerId === providerId,
    );
    const nextIndex = currentIndex + direction;
    if (
      currentIndex < 0 ||
      nextIndex < 0 ||
      nextIndex >= failoverQueue.length
    ) {
      return;
    }
    const nextIds = failoverQueue.map((item) => item.providerId);
    [nextIds[currentIndex], nextIds[nextIndex]] = [
      nextIds[nextIndex],
      nextIds[currentIndex],
    ];
    setBusyAction("failover");
    try {
      setFailoverQueue(
        await settingsApi.replaceFailoverQueue(failoverApp, nextIds),
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.proxy.failoverQueueFailed", {
          defaultValue: "更新故障切换队列失败",
        }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const handleClearFailoverQueue = async () => {
    setBusyAction("failover");
    try {
      setFailoverQueue(await settingsApi.clearFailoverQueue(failoverApp));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.proxy.failoverQueueFailed", {
          defaultValue: "更新故障切换队列失败",
        }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const isBusy = busyAction !== null;
  const isRunning = status?.running ?? false;
  const bindAppName = t(`apps.${value.bindApp}`, {
    defaultValue: value.bindApp,
  });
  const failoverProviderOptions = Object.values(failoverProviders)
    .filter(
      (provider) =>
        !failoverQueue.some((item) => item.providerId === provider.id),
    )
    .sort((a, b) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
    );

  return (
    <section className="space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="text-sm font-medium">
            {t("settings.proxy.title", { defaultValue: "Local Routing" })}
          </h3>
          <p className="text-xs text-muted-foreground">
            {t("settings.proxy.description", {
              defaultValue:
                "统一管理本地路由、应用接管、自动故障切换和请求整流。",
            })}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs text-muted-foreground">
            {isRunning
              ? t("settings.proxy.running", { defaultValue: "运行中" })
              : t("settings.proxy.stoppedStatus", { defaultValue: "已停止" })}
          </span>
          <Switch
            checked={value.enabled}
            onCheckedChange={(checked) => update({ enabled: checked })}
            aria-label={t("settings.proxy.enabled", {
              defaultValue: "启用代理",
            })}
          />
        </div>
      </div>

      <div className="grid gap-3 sm:grid-cols-4">
        <div className="rounded-md border p-3">
          <div className="text-xs text-muted-foreground">
            {t("settings.proxy.listen", { defaultValue: "监听地址" })}
          </div>
          <div className="mt-1 truncate text-sm font-medium">{listenUrl}</div>
        </div>
        <div className="rounded-md border p-3">
          <div className="text-xs text-muted-foreground">
            {t("settings.proxy.requests", { defaultValue: "请求数" })}
          </div>
          <div className="mt-1 text-sm font-medium">
            {status?.totalRequests ?? 0}
          </div>
        </div>
        <div className="rounded-md border p-3">
          <div className="text-xs text-muted-foreground">
            {t("settings.proxy.successRate", { defaultValue: "成功率" })}
          </div>
          <div className="mt-1 text-sm font-medium">
            {status?.successRate ?? 0}%
          </div>
        </div>
        <div className="rounded-md border p-3">
          <div className="text-xs text-muted-foreground">
            {t("settings.proxy.uptime", { defaultValue: "运行时长" })}
          </div>
          <div className="mt-1 text-sm font-medium">
            {status?.uptimeSeconds ?? 0}s
          </div>
        </div>
      </div>

      {status?.failoverCount ? (
        <div className="rounded-md border p-3 text-xs text-muted-foreground">
          <span className="font-medium text-foreground">
            {t("settings.proxy.failover", { defaultValue: "故障切换" })}
          </span>
          <span className="ml-2">
            {status.failoverCount}
            {status.lastFailoverFrom && status.lastFailoverTo
              ? ` · ${status.lastFailoverFrom} -> ${status.lastFailoverTo}`
              : null}
          </span>
        </div>
      ) : null}

      <div className="grid gap-3 sm:grid-cols-2">
        <div className="space-y-2">
          <Label htmlFor="cc-switch-proxy-host">
            {t("settings.proxy.host", { defaultValue: "监听地址" })}
          </Label>
          <Input
            id="cc-switch-proxy-host"
            value={value.host}
            onChange={(event) => update({ host: event.target.value })}
            placeholder="127.0.0.1"
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="cc-switch-proxy-port">
            {t("settings.proxy.port", { defaultValue: "端口" })}
          </Label>
          <Input
            id="cc-switch-proxy-port"
            type="number"
            min={1}
            max={65535}
            value={value.port}
            onChange={(event) =>
              update({ port: Number(event.target.value) || 3456 })
            }
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="cc-switch-proxy-upstream">
            {t("settings.proxy.upstreamProxy", {
              defaultValue: "上游代理",
            })}
          </Label>
          <Input
            id="cc-switch-proxy-upstream"
            value={value.upstreamProxy ?? ""}
            onChange={(event) =>
              update({ upstreamProxy: event.target.value || undefined })
            }
            placeholder="http://127.0.0.1:7890"
          />
        </div>
      </div>

      <label className="flex items-center gap-2 text-sm text-muted-foreground">
        <Switch
          checked={value.autoStart}
          onCheckedChange={(checked) => update({ autoStart: checked })}
        />
        <span>
          {t("settings.proxy.autoStart", {
            defaultValue: "随 Web server 启动",
          })}
        </span>
      </label>

      <label className="flex items-center gap-2 text-sm text-muted-foreground">
        <Switch
          checked={value.enableLogging}
          onCheckedChange={(checked) => update({ enableLogging: checked })}
        />
        <span>
          {t("settings.proxy.enableLogging", {
            defaultValue: "记录最近请求状态",
          })}
        </span>
      </label>

      <div className="space-y-2">
        <div>
          <h4 className="text-sm font-medium">
            {t("settings.proxy.takeover", { defaultValue: "应用接管" })}
          </h4>
          <p className="text-xs text-muted-foreground">
            {t("settings.proxy.takeoverDescription", {
              defaultValue:
                "选择要被 cc-switch-web 修改配置的客户端。开启后，该客户端会被写入本地代理地址；停止或恢复时会还原原配置。",
            })}
          </p>
        </div>
        <div className="grid gap-2 sm:grid-cols-2">
          {PROXY_ROUTE_APPS.map((app) => {
            const configApp = proxyConfigApp(app);
            const supportsTakeover = app !== "claude-desktop";
            const takeoverEnabled = value.apps[configApp]?.enabled ?? false;
            const autoFailoverEnabled =
              value.apps[configApp]?.autoFailoverEnabled ?? false;
            const failoverToggleDisabled =
              isBusy || (!takeoverEnabled && !autoFailoverEnabled);
            const target = status?.activeTargets?.find(
              (item) => item.appType === app,
            );
            const providerHealth = target
              ? status?.providerHealth?.find(
                  (item) =>
                    item.appType === configApp &&
                    item.providerId === target.providerId,
                )
              : undefined;
            const busy = busyAction === `takeover:${app}`;
            const testBusy = busyAction === `test:${app}`;
            const resetBusy = Boolean(
              target &&
                busyAction === `reset:${configApp}:${target.providerId}`,
            );
            const appName = t(`apps.${app}`, { defaultValue: app });
            return (
              <div
                key={app}
                className="flex flex-wrap items-center justify-between gap-3 rounded-md border p-3"
              >
                <div className="min-w-0">
                  <div className="text-sm font-medium">
                    {appName}
                    {app === "opencode" ? (
                      <span className="ml-2 text-xs text-amber-600 dark:text-amber-400">
                        {t("settings.proxy.experimental", {
                          defaultValue: "实验性",
                        })}
                      </span>
                    ) : null}
                  </div>
                  <div className="flex min-w-0 flex-wrap items-center gap-2 text-xs text-muted-foreground">
                    <span className="truncate">
                      {target?.providerName ??
                        t("settings.proxy.providerHidden", {
                          defaultValue: "使用当前供应商，API key 不显示",
                        })}
                    </span>
                    {providerHealth ? (
                      <span
                        className={`inline-flex shrink-0 items-center gap-1 rounded border px-1.5 py-0.5 text-[11px] leading-none ${providerHealthClass(
                          providerHealth.state,
                        )}`}
                        title={`failures=${providerHealth.failureCount}, window=${providerHealth.windowFailures}/${providerHealth.windowRequests}`}
                      >
                        {providerHealthLabel(providerHealth.state)}
                        {providerHealth.state !== "healthy" && target ? (
                          <button
                            type="button"
                            className="rounded-sm p-0.5 hover:bg-background/70 disabled:opacity-60"
                            disabled={isBusy}
                            onClick={() =>
                              void handleResetProviderCircuit(
                                configApp,
                                target.providerId,
                              )
                            }
                            aria-label={t("settings.proxy.resetCircuit", {
                              defaultValue: "重置熔断",
                            })}
                          >
                            <RotateCcw
                              className={`h-3 w-3 ${
                                resetBusy ? "animate-spin" : ""
                              }`}
                            />
                          </button>
                        ) : null}
                      </span>
                    ) : null}
                  </div>
                  <div className="text-xs text-muted-foreground">
                    {supportsTakeover
                      ? t(`settings.proxy.takeoverHint.${app}`, {
                          defaultValue: `${appName} 接管：让 ${appName} 走本地代理`,
                        })
                      : t("settings.proxy.takeoverHint.claudeDesktopManual", {
                          defaultValue:
                            "Claude Desktop 需要在专属面板应用 3P profile；这里仅提供路由测试和故障切换设置",
                        })}
                  </div>
                  <div className="mt-2 grid gap-2 sm:grid-cols-2">
                    <label className="flex items-center gap-2 text-xs text-muted-foreground">
                      <Switch
                        checked={autoFailoverEnabled}
                        onCheckedChange={(checked) =>
                          takeoverEnabled || !checked
                            ? updateApp(app, {
                                autoFailoverEnabled: checked,
                              })
                            : toast.error(
                                t("settings.proxy.failoverTakeoverRequired", {
                                  defaultValue:
                                    "请先开启代理接管，再启用自动故障切换",
                                }),
                                {
                                  description: t(`apps.${configApp}`, {
                                    defaultValue: configApp,
                                  }),
                                },
                              )
                        }
                        disabled={failoverToggleDisabled}
                      />
                      <span>
                        {t("settings.proxy.autoFailover", {
                          defaultValue: "自动故障切换",
                        })}
                      </span>
                    </label>
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-muted-foreground">
                        {t("settings.proxy.maxRetries", {
                          defaultValue: "重试",
                        })}
                      </span>
                      <Input
                        type="number"
                        min={0}
                        max={10}
                        value={value.apps[configApp]?.maxRetries ?? 0}
                        onChange={(event) =>
                          updateApp(app, {
                            maxRetries: Number(event.target.value) || 0,
                          })
                        }
                        className="h-8 w-20"
                      />
                    </div>
                    <div className="space-y-1">
                      <span className="text-xs text-muted-foreground">
                        {t("settings.proxy.costMultiplier", {
                          defaultValue: "计费倍率",
                        })}
                      </span>
                      <Input
                        value={
                          value.apps[configApp]?.defaultCostMultiplier ?? "1"
                        }
                        onChange={(event) =>
                          updateApp(app, {
                            defaultCostMultiplier: event.target.value,
                          })
                        }
                        className="h-8"
                      />
                    </div>
                    <div className="space-y-1">
                      <span className="text-xs text-muted-foreground">
                        {t("settings.proxy.pricingModelSource", {
                          defaultValue: "模型来源",
                        })}
                      </span>
                      <Select
                        value={
                          value.apps[configApp]?.pricingModelSource ??
                          "response"
                        }
                        onValueChange={(source) =>
                          updateApp(app, { pricingModelSource: source })
                        }
                      >
                        <SelectTrigger className="h-8">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="response">response</SelectItem>
                          <SelectItem value="request">request</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>
                    <details className="col-span-full rounded-md border border-border-default/70 p-2">
                      <summary className="cursor-pointer text-xs font-medium text-muted-foreground">
                        {t("settings.proxy.appRuntimeSettings", {
                          defaultValue: "超时与熔断参数",
                        })}
                      </summary>
                      <div className="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
                        {PROXY_APP_NUMERIC_FIELDS.map(
                          ([field, labelKey, label, min, max, fallback]) => (
                            <div key={field} className="space-y-1">
                              <Label
                                htmlFor={`proxy-${configApp}-${field}`}
                                className="text-xs"
                              >
                                {t(`settings.proxy.${labelKey}`, {
                                  defaultValue: label,
                                })}
                              </Label>
                              <Input
                                id={`proxy-${configApp}-${field}`}
                                type="number"
                                min={min}
                                max={max}
                                value={
                                  value.apps[configApp]?.[field] ?? fallback
                                }
                                onChange={(event) =>
                                  updateApp(app, {
                                    [field]:
                                      Number(event.target.value) || fallback,
                                  })
                                }
                                className="h-8"
                              />
                            </div>
                          ),
                        )}
                      </div>
                    </details>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => void handleTest(app)}
                    disabled={isBusy && !testBusy}
                    className="gap-1"
                  >
                    {testBusy ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : (
                      <TestTube2 className="h-3.5 w-3.5" />
                    )}
                    {t("settings.proxy.testApp", {
                      defaultValue: `测试 ${appName}`,
                      app: appName,
                    })}
                  </Button>
                  {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                  <Switch
                    checked={
                      supportsTakeover
                        ? (value.apps[configApp]?.enabled ?? false)
                        : false
                    }
                    onCheckedChange={(checked) =>
                      supportsTakeover
                        ? void handleTakeoverChange(configApp, checked)
                        : undefined
                    }
                    disabled={isBusy || !supportsTakeover}
                  />
                </div>
              </div>
            );
          })}
          <div className="flex items-center justify-between gap-3 rounded-md border border-dashed p-3 opacity-70">
            <div>
              <div className="text-sm font-medium">
                {t("apps.omo", { defaultValue: "OMO" })}
              </div>
              <div className="text-xs text-muted-foreground">
                {t("settings.proxy.omoUnsupported", {
                  defaultValue: "暂不支持代理接管",
                })}
              </div>
            </div>
            <Switch checked={false} disabled />
          </div>
        </div>
      </div>

      <div className="space-y-3 rounded-md border p-3">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h4 className="text-sm font-medium">
              {t("settings.proxy.failoverQueue", {
                defaultValue: "故障切换队列",
              })}
            </h4>
            <p className="text-xs text-muted-foreground">
              {t("settings.proxy.failoverQueueDescription", {
                defaultValue:
                  "代理故障切换优先读取 SQLite 队列；队列为空时才回退到备用供应商。",
              })}
            </p>
          </div>
          <Select
            value={failoverApp}
            onValueChange={(app) => setFailoverApp(app as ProxyRouteAppId)}
          >
            <SelectTrigger className="h-8 w-[150px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PROXY_ROUTE_APPS.map((app) => (
                <SelectItem key={app} value={app}>
                  {t(`apps.${app}`, { defaultValue: app })}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Select
            value={failoverProviderId}
            onValueChange={setFailoverProviderId}
            disabled={failoverLoading || failoverProviderOptions.length === 0}
          >
            <SelectTrigger className="h-9 min-w-[220px]">
              <SelectValue
                placeholder={t("settings.proxy.selectProvider", {
                  defaultValue: "选择 Provider",
                })}
              />
            </SelectTrigger>
            <SelectContent>
              {failoverProviderOptions.map((provider) => (
                <SelectItem key={provider.id} value={provider.id}>
                  {provider.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => void handleAddFailoverProvider()}
            disabled={isBusy || !failoverProviderId}
            className="gap-1"
          >
            <Plus className="h-3.5 w-3.5" />
            {t("common.add", { defaultValue: "新增" })}
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => void handleClearFailoverQueue()}
            disabled={isBusy || failoverQueue.length === 0}
            className="gap-1"
          >
            <Trash2 className="h-3.5 w-3.5" />
            {t("common.clear", { defaultValue: "清空" })}
          </Button>
        </div>
        {failoverLoading ? (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
            {t("common.loading", { defaultValue: "加载中" })}
          </div>
        ) : failoverQueue.length === 0 ? (
          <div className="text-xs text-muted-foreground">
            {t("settings.proxy.failoverQueueEmpty", {
              defaultValue: "当前队列为空",
            })}
          </div>
        ) : (
          <div className="space-y-2">
            {failoverQueue.map((item, index) => (
              <div
                key={item.providerId}
                className="grid items-center gap-2 rounded-md border px-3 py-2 text-sm sm:grid-cols-[32px_1fr_auto]"
              >
                <div className="text-xs text-muted-foreground">{index + 1}</div>
                <div className="min-w-0">
                  <div className="truncate font-medium">
                    {item.providerName}
                  </div>
                  <div className="truncate text-xs text-muted-foreground">
                    {item.providerId}
                  </div>
                </div>
                <div className="flex items-center gap-1">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() =>
                      void handleMoveFailoverProvider(item.providerId, -1)
                    }
                    disabled={isBusy || index === 0}
                  >
                    <ChevronUp className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() =>
                      void handleMoveFailoverProvider(item.providerId, 1)
                    }
                    disabled={isBusy || index === failoverQueue.length - 1}
                  >
                    <ChevronDown className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() =>
                      void handleRemoveFailoverProvider(item.providerId)
                    }
                    disabled={isBusy}
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="space-y-3 rounded-md border p-3">
        <div>
          <h4 className="text-sm font-medium">
            {t("settings.proxy.advanced", { defaultValue: "高级设置" })}
          </h4>
        </div>
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="space-y-2">
            <Label>
              {t("settings.proxy.bindApp", {
                defaultValue: "默认绑定客户端",
              })}
            </Label>
            <Select
              value={value.bindApp}
              onValueChange={(app) =>
                update({ bindApp: app as ProxyRouteAppId })
              }
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {PROXY_ROUTE_APPS.map((app) => (
                  <SelectItem key={app} value={app}>
                    {t(`apps.${app}`, { defaultValue: app })}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              {t("settings.proxy.bindAppDescription", {
                defaultValue:
                  "当代理无法根据请求路径判断目标客户端时，默认按这个客户端的当前 provider 转发。普通用户一般不用改；如果你只测试某一个客户端，可以选成对应客户端。",
              })}
            </p>
          </div>
          <div className="space-y-2 rounded-md border p-3">
            <h5 className="text-sm font-medium">
              {t("settings.proxy.rectifier", {
                defaultValue: "Rectifier",
              })}
            </h5>
            <label className="flex items-center justify-between gap-3 text-sm text-muted-foreground">
              <span>
                {t("settings.proxy.rectifyThinkingSignature", {
                  defaultValue: "修复 thinking signature",
                })}
              </span>
              <Switch
                checked={value.rectifyThinkingSignature}
                onCheckedChange={(checked) =>
                  update({ rectifyThinkingSignature: checked })
                }
              />
            </label>
            <label className="flex items-center justify-between gap-3 text-sm text-muted-foreground">
              <span>
                {t("settings.proxy.rectifyThinkingBudget", {
                  defaultValue: "修复 thinking budget",
                })}
              </span>
              <Switch
                checked={value.rectifyThinkingBudget}
                onCheckedChange={(checked) =>
                  update({ rectifyThinkingBudget: checked })
                }
              />
            </label>
          </div>
          <div className="space-y-3 rounded-md border p-3">
            <div>
              <h5 className="text-sm font-medium">
                {t("settings.proxy.optimizer", {
                  defaultValue: "Bedrock optimizer",
                })}
              </h5>
              <p className="mt-1 text-xs text-muted-foreground">
                {t("settings.proxy.optimizerDescription", {
                  defaultValue:
                    "仅对 CLAUDE_CODE_USE_BEDROCK=1 的 Anthropic 请求生效，默认按上游保持关闭。",
                })}
              </p>
            </div>
            <label className="flex items-center justify-between gap-3 text-sm text-muted-foreground">
              <span>
                {t("settings.proxy.optimizerEnabled", {
                  defaultValue: "启用 Bedrock 请求优化",
                })}
              </span>
              <Switch
                checked={value.optimizerEnabled}
                onCheckedChange={(checked) =>
                  update({ optimizerEnabled: checked })
                }
              />
            </label>
            <label className="flex items-center justify-between gap-3 text-sm text-muted-foreground">
              <span>
                {t("settings.proxy.optimizerThinking", {
                  defaultValue: "Thinking 配置优化",
                })}
              </span>
              <Switch
                checked={value.optimizerThinking}
                disabled={!value.optimizerEnabled}
                onCheckedChange={(checked) =>
                  update({ optimizerThinking: checked })
                }
              />
            </label>
            <label className="flex items-center justify-between gap-3 text-sm text-muted-foreground">
              <span>
                {t("settings.proxy.optimizerCacheInjection", {
                  defaultValue: "Prompt cache 断点注入",
                })}
              </span>
              <Switch
                checked={value.optimizerCacheInjection}
                disabled={!value.optimizerEnabled}
                onCheckedChange={(checked) =>
                  update({ optimizerCacheInjection: checked })
                }
              />
            </label>
            <div className="space-y-1">
              <Label>
                {t("settings.proxy.optimizerCacheTtl", {
                  defaultValue: "Cache TTL",
                })}
              </Label>
              <Select
                value={value.optimizerCacheTtl === "5m" ? "5m" : "1h"}
                onValueChange={(ttl) =>
                  update({ optimizerCacheTtl: ttl as "5m" | "1h" })
                }
                disabled={!value.optimizerEnabled}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1h">1h</SelectItem>
                  <SelectItem value="5m">5m</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>
      </div>

      {value.host.trim() === "0.0.0.0" ? (
        <p className="text-xs text-amber-600 dark:text-amber-400">
          {t("settings.proxy.publicBindWarning", {
            defaultValue:
              "当前会暴露到所有网卡。请只在可信内网或 TLS 反代后使用。",
          })}
        </p>
      ) : null}

      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          variant="outline"
          onClick={() => void handleTest()}
          disabled={isBusy}
          className="gap-2"
        >
          {busyAction === "test" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <TestTube2 className="h-4 w-4" />
          )}
          {t("settings.proxy.test", {
            defaultValue: `测试绑定客户端：${bindAppName}`,
            app: bindAppName,
          })}
        </Button>
        <Button
          type="button"
          onClick={handleStart}
          disabled={isBusy}
          className="gap-2"
        >
          {busyAction === "start" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Play className="h-4 w-4" />
          )}
          {t("settings.proxy.start", { defaultValue: "启动代理" })}
        </Button>
        <Button
          type="button"
          variant="outline"
          onClick={handleStop}
          disabled={isBusy || !isRunning}
          className="gap-2"
        >
          {busyAction === "stop" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Square className="h-4 w-4" />
          )}
          {t("settings.proxy.stop", { defaultValue: "停止代理" })}
        </Button>
        <Button
          type="button"
          variant="outline"
          onClick={handleRestore}
          disabled={isBusy}
          className="gap-2"
        >
          {busyAction === "restore" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <RotateCcw className="h-4 w-4" />
          )}
          {t("settings.proxy.restore", { defaultValue: "恢复接管" })}
        </Button>
        <span className="text-xs text-muted-foreground">{listenUrl}</span>
      </div>

      {status?.lastError ? (
        <p className="text-xs text-red-500 dark:text-red-400">
          {status.lastError}
        </p>
      ) : null}

      <div className="rounded-md border">
        <button
          type="button"
          className="flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm font-medium"
          onClick={() => void toggleLogs()}
          aria-expanded={logsOpen}
        >
          <span>
            {t("settings.proxy.recentLogs", {
              defaultValue: "最近请求",
            })}
          </span>
          {logsOpen ? (
            <ChevronUp className="h-4 w-4" />
          ) : (
            <ChevronDown className="h-4 w-4" />
          )}
        </button>
        {logsOpen ? (
          <div className="border-t px-3 py-2">
            {logsLoading ? (
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
                {t("settings.proxy.loadingLogs", {
                  defaultValue: "加载中",
                })}
              </div>
            ) : recentLogs.length === 0 ? (
              <div className="text-xs text-muted-foreground">
                {t("settings.proxy.noRecentLogs", {
                  defaultValue: "暂无最近请求",
                })}
              </div>
            ) : (
              <div className="space-y-2">
                {recentLogs.slice(-5).map((log, index) => (
                  <div
                    key={`${log.at}-${index}`}
                    className="grid gap-1 text-xs sm:grid-cols-[80px_1fr_72px]"
                  >
                    <div className="font-medium">{log.method}</div>
                    <div className="truncate text-muted-foreground">
                      {log.app} {log.path}
                    </div>
                    <div className="text-muted-foreground">
                      {log.status ?? "-"} · {log.durationMs}ms
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        ) : null}
      </div>
    </section>
  );
}
