import { useMemo } from "react";
import { Activity, MoveVertical, Copy, Route } from "lucide-react";
import { useTranslation } from "react-i18next";
import type {
  DraggableAttributes,
  DraggableSyntheticListeners,
} from "@dnd-kit/core";
import type { Provider, ProxyProviderHealth } from "@/types";
import type { AppId, ProviderHealth } from "@/lib/api";
import type { StreamCheckLog } from "@/lib/api/model-test";
import { isUsageApp } from "@/config/apps";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { ProviderActions } from "@/components/providers/ProviderActions";
import UsageFooter from "@/components/UsageFooter";
import { ProviderIcon } from "@/components/ProviderIcon";
import { SubscriptionQuotaSummary } from "@/components/providers/SubscriptionQuotaSummary";
import { FailoverPriorityBadge } from "@/components/providers/FailoverPriorityBadge";
import { ProviderProxyHealthBadge } from "@/components/providers/ProviderProxyHealthBadge";

interface DragHandleProps {
  attributes: DraggableAttributes;
  listeners: DraggableSyntheticListeners;
  isDragging: boolean;
}

interface ProviderCardProps {
  provider: Provider;
  isCurrent: boolean;
  backupProviderId?: string | null;
  appId: AppId;
  isEditMode?: boolean;
  onSwitch: (provider: Provider) => void;
  onEdit: (provider: Provider) => void;
  onDelete: (provider: Provider) => void;
  onConfigureUsage: (provider: Provider) => void;
  onStreamCheck?: (provider: Provider) => void;
  isStreamChecking?: boolean;
  onOpenWebsite: (url: string) => void;
  onDuplicate: (provider: Provider) => void;
  onAutoFailover?: (targetId?: string | null) => void;
  dragHandleProps?: DragHandleProps;
  healthStatus?: ProviderHealth;
  streamCheckLog?: StreamCheckLog;
  isLiveConfigured?: boolean;
  failoverPriority?: number;
  failoverActive?: boolean;
  proxyHealth?: ProxyProviderHealth;
  isActiveRoute?: boolean;
}

const extractApiUrl = (provider: Provider, fallbackText: string) => {
  // 优先级 1: 备注
  if (provider.notes?.trim()) {
    return provider.notes.trim();
  }

  // 优先级 2: 官网地址
  if (provider.websiteUrl) {
    return provider.websiteUrl;
  }

  // 优先级 3: 从配置中提取请求地址
  const config = provider.settingsConfig;

  if (config && typeof config === "object") {
    const envBase =
      (config as Record<string, any>)?.env?.ANTHROPIC_BASE_URL ||
      (config as Record<string, any>)?.env?.GOOGLE_GEMINI_BASE_URL;
    if (typeof envBase === "string" && envBase.trim()) {
      return envBase;
    }

    const opencodeBase = (config as Record<string, any>)?.options?.baseURL;
    if (typeof opencodeBase === "string" && opencodeBase.trim()) {
      return opencodeBase;
    }

    const openclawBase = (config as Record<string, any>)?.baseUrl;
    if (typeof openclawBase === "string" && openclawBase.trim()) {
      return openclawBase;
    }

    const baseUrl = (config as Record<string, any>)?.config;

    if (typeof baseUrl === "string" && baseUrl.includes("base_url")) {
      const match = baseUrl.match(/base_url\s*=\s*['"]([^'"]+)['"]/);
      if (match?.[1]) {
        return match[1];
      }
    }
  }

  return fallbackText;
};

const LOCAL_ROUTING_APPS = new Set<AppId>([
  "claude",
  "codex",
  "gemini",
  "opencode",
]);

const supportsLocalRouting = (appId: AppId, provider: Provider) => {
  if (appId === "claude-desktop") {
    return provider.meta?.claudeDesktopMode === "proxy";
  }

  if (!LOCAL_ROUTING_APPS.has(appId)) {
    return false;
  }

  if (appId === "gemini" && provider.category === "official") {
    return false;
  }

  return true;
};

export function ProviderCard({
  provider,
  isCurrent,
  backupProviderId,
  appId,
  isEditMode = false,
  onSwitch,
  onEdit,
  onDelete,
  onConfigureUsage,
  onStreamCheck,
  isStreamChecking = false,
  onOpenWebsite,
  onDuplicate,
  onAutoFailover,
  dragHandleProps,
  healthStatus,
  streamCheckLog,
  isLiveConfigured,
  failoverPriority,
  failoverActive = false,
  proxyHealth,
  isActiveRoute = false,
}: ProviderCardProps) {
  const { t } = useTranslation();

  const fallbackUrlText = t("provider.notConfigured", {
    defaultValue: "未配置接口地址",
  });

  const displayUrl = useMemo(() => {
    return extractApiUrl(provider, fallbackUrlText);
  }, [provider, fallbackUrlText]);

  // 判断是否为可点击的 URL（备注不可点击）
  const isClickableUrl = useMemo(() => {
    // 如果有备注，则不可点击
    if (provider.notes?.trim()) {
      return false;
    }
    // 如果显示的是回退文本，也不可点击
    if (displayUrl === fallbackUrlText) {
      return false;
    }
    // 其他情况（官网地址或请求地址）可点击
    return true;
  }, [provider.notes, displayUrl, fallbackUrlText]);

  const usageEnabled = provider.meta?.usage_script?.enabled ?? false;
  const usageSupported = isUsageApp(appId);
  const routingSupported = supportsLocalRouting(appId, provider);
  const showSubscriptionQuota =
    provider.category === "official" ||
    provider.meta?.providerType === "codex_oauth";

  const handleOpenWebsite = () => {
    if (!isClickableUrl) {
      return;
    }
    onOpenWebsite(displayUrl);
  };

  const healthIndicator = useMemo(() => {
    if (!healthStatus) return undefined;

    const statusLabelMap: Record<ProviderHealth["status"], string> = {
      available: t("provider.health.available", { defaultValue: "可用" }),
      degraded: t("provider.health.degraded", { defaultValue: "降级" }),
      unavailable: t("provider.health.unavailable", { defaultValue: "不可用" }),
      unknown: t("provider.health.unknown", { defaultValue: "未知" }),
    };

    const indicatorColor =
      {
        available: "bg-green-500",
        degraded: "bg-yellow-500",
        unavailable: "bg-red-500",
        unknown: "bg-gray-400",
      }[healthStatus.status] ?? "bg-gray-400";

    const availability =
      typeof healthStatus.availability === "number"
        ? healthStatus.availability
        : undefined;
    const availabilityText =
      typeof availability === "number"
        ? `${availability.toFixed(1)}%`
        : undefined;
    const availabilityDisplay = availabilityText ?? "--%";

    const tooltipParts = [
      `${t("provider.health.statusLabel", { defaultValue: "状态" })}: ${
        statusLabelMap[healthStatus.status] ?? statusLabelMap.unknown
      }`,
      `${t("provider.health.latency", { defaultValue: "延迟" })}: ${Math.round(healthStatus.latency)}ms`,
      `${t("provider.health.availability24h", { defaultValue: "24小时可用率" })}: ${
        availabilityText ??
        t("provider.health.availabilityUnknown", {
          defaultValue: "暂无可用率数据",
        })
      }`,
      healthStatus.lastChecked > 0
        ? `${t("provider.health.lastChecked", { defaultValue: "最近检查" })}: ${new Date(
            healthStatus.lastChecked,
          ).toLocaleString()}`
        : undefined,
    ].filter(Boolean);

    const tooltip = tooltipParts.join(" · ");

    return { indicatorColor, tooltip, availabilityText, availabilityDisplay };
  }, [healthStatus, t]);

  const streamCheckIndicator = useMemo(() => {
    if (!streamCheckLog) return undefined;

    const statusLabel = {
      operational: t("streamCheck.statusOperational", {
        defaultValue: "Operational",
      }),
      degraded: t("streamCheck.statusDegraded", {
        defaultValue: "Degraded",
      }),
      failed: t("streamCheck.statusFailed", { defaultValue: "Failed" }),
    }[streamCheckLog.status];
    const detail = [
      streamCheckLog.responseTimeMs != null
        ? `${streamCheckLog.responseTimeMs}ms`
        : undefined,
      !streamCheckLog.success
        ? (streamCheckLog.errorCategory ??
          (streamCheckLog.httpStatus != null
            ? `HTTP ${streamCheckLog.httpStatus}`
            : undefined))
        : undefined,
    ]
      .filter(Boolean)
      .join(" · ");
    const tooltip = [
      `${t("streamCheck.latestStatus", { defaultValue: "Latest check" })}: ${statusLabel}`,
      streamCheckLog.responseTimeMs != null
        ? `${streamCheckLog.responseTimeMs}ms`
        : undefined,
      streamCheckLog.errorCategory,
      new Date(streamCheckLog.testedAt * 1000).toLocaleString(),
    ]
      .filter(Boolean)
      .join(" · ");
    const color = {
      operational: "text-green-600 dark:text-green-300",
      degraded: "text-amber-700 dark:text-amber-300",
      failed: "text-red-600 dark:text-red-300",
    }[streamCheckLog.status];

    return { color, detail, statusLabel, tooltip };
  }, [streamCheckLog, t]);

  return (
    <div
      className={cn(
        "rounded-lg bg-card p-4 shadow-sm",
        "transition-[border-color,background-color,box-shadow,ring] duration-200",
        isCurrent
          ? "border border-border-default bg-primary/5 ring-2 ring-blue-500/30 dark:ring-blue-400/30"
          : "border border-border-default hover:border-border-hover",
        dragHandleProps?.isDragging &&
          "cursor-grabbing border-active border-border-dragging shadow-lg",
      )}
    >
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-1 items-center gap-2">
          <div
            className={cn(
              "flex items-center gap-1 overflow-hidden",
              "transition-[max-width,opacity] duration-200 ease-in-out",
              isEditMode ? "max-w-20 opacity-100" : "max-w-0 opacity-0",
            )}
            aria-hidden={!isEditMode}
          >
            <Button
              type="button"
              size="icon"
              variant="ghost"
              className={cn(
                "flex-shrink-0 cursor-grab active:cursor-grabbing",
                dragHandleProps?.isDragging && "cursor-grabbing",
              )}
              aria-label={t("provider.dragHandle")}
              disabled={!isEditMode}
              {...(dragHandleProps?.attributes ?? {})}
              {...(dragHandleProps?.listeners ?? {})}
            >
              <MoveVertical className="h-4 w-4" />
            </Button>

            <Button
              type="button"
              size="icon"
              variant="ghost"
              className="flex-shrink-0"
              onClick={() => onDuplicate(provider)}
              disabled={!isEditMode}
              aria-label={t("provider.duplicate")}
              title={t("provider.duplicate")}
            >
              <Copy className="h-4 w-4" />
            </Button>
          </div>

          <ProviderIcon
            name={provider.name}
            websiteUrl={provider.websiteUrl}
            size={32}
          />

          <div className="space-y-1">
            <div className="flex flex-wrap items-center gap-2 min-h-[20px]">
              <h3 className="text-base font-semibold leading-none">
                {provider.name}
              </h3>
              {healthIndicator && (
                <span
                  className="inline-flex items-center gap-1 text-xs text-muted-foreground"
                  title={healthIndicator.tooltip}
                  aria-label={healthIndicator.tooltip}
                >
                  <span
                    className={cn(
                      "h-2 w-2 rounded-full",
                      healthIndicator.indicatorColor,
                    )}
                    aria-hidden="true"
                  />
                  <span className="leading-none">
                    {healthIndicator.availabilityDisplay}
                  </span>
                </span>
              )}
              {isActiveRoute ? (
                <span
                  className="inline-flex items-center gap-1 rounded border border-emerald-500/30 bg-emerald-500/10 px-1.5 py-0.5 text-xs font-medium text-emerald-700 dark:text-emerald-300"
                  title={t("provider.activeRouteHint", {
                    defaultValue: "本地代理当前实际路由到此 Provider",
                  })}
                >
                  <Route className="h-3 w-3" aria-hidden="true" />
                  {t("provider.activeRoute", { defaultValue: "路由中" })}
                </span>
              ) : null}
              {typeof failoverPriority === "number" ? (
                <FailoverPriorityBadge
                  priority={failoverPriority}
                  active={failoverActive}
                />
              ) : null}
              {proxyHealth ? (
                <ProviderProxyHealthBadge health={proxyHealth} />
              ) : null}
              {streamCheckIndicator ? (
                <span
                  className={cn(
                    "inline-flex min-w-0 max-w-full items-center gap-1 text-xs",
                    streamCheckIndicator.color,
                  )}
                  title={streamCheckIndicator.tooltip}
                  aria-label={streamCheckIndicator.tooltip}
                >
                  <Activity
                    className="h-3.5 w-3.5 flex-none"
                    aria-hidden="true"
                  />
                  <span>{streamCheckIndicator.statusLabel}</span>
                  {streamCheckIndicator.detail ? (
                    <span className="max-w-40 truncate">
                      · {streamCheckIndicator.detail}
                    </span>
                  ) : null}
                </span>
              ) : null}
              {provider.category === "third_party" &&
                provider.meta?.isPartner && (
                  <span
                    className="text-yellow-500 dark:text-yellow-400"
                    title={t("provider.officialPartner", {
                      defaultValue: "官方合作伙伴",
                    })}
                  >
                    ⭐
                  </span>
                )}
              {routingSupported ? (
                <span
                  className="rounded-full border border-blue-500/20 bg-blue-500/10 px-2 py-0.5 text-xs font-medium text-blue-600 dark:text-blue-300"
                  title={t("provider.routingSupportHint", {
                    defaultValue: "可通过 Local Routing 接管",
                  })}
                >
                  {t("provider.routingSupport", {
                    defaultValue: "Routing",
                  })}
                </span>
              ) : null}
              {appId === "openclaw" && typeof isLiveConfigured === "boolean" ? (
                <span
                  className={cn(
                    "rounded-full border px-2 py-0.5 text-xs font-medium",
                    isLiveConfigured
                      ? "border-green-500/20 bg-green-500/10 text-green-600 dark:text-green-300"
                      : "border-amber-500/20 bg-amber-500/10 text-amber-700 dark:text-amber-300",
                  )}
                >
                  {isLiveConfigured
                    ? t("openclaw.liveConfigured", { defaultValue: "已写入" })
                    : t("openclaw.storedOnly", { defaultValue: "仅配置库" })}
                </span>
              ) : null}
              {showSubscriptionQuota ? (
                <SubscriptionQuotaSummary appId={appId} />
              ) : null}
              <span
                className={cn(
                  "rounded-full bg-green-500/10 px-2 py-0.5 text-xs font-medium text-green-500 dark:text-green-400 transition-opacity duration-200",
                  isCurrent ? "opacity-100" : "opacity-0 pointer-events-none",
                )}
              >
                {appId === "openclaw"
                  ? t("openclaw.defaultModel", { defaultValue: "默认模型" })
                  : t("provider.currentlyUsing")}
              </span>
            </div>

            {displayUrl && (
              <button
                type="button"
                onClick={handleOpenWebsite}
                className={cn(
                  "inline-flex items-center text-sm max-w-[280px]",
                  isClickableUrl
                    ? "text-blue-500 transition-colors hover:underline dark:text-blue-400 cursor-pointer"
                    : "text-muted-foreground cursor-default",
                )}
                title={displayUrl}
                disabled={!isClickableUrl}
              >
                <span className="truncate">{displayUrl}</span>
              </button>
            )}
          </div>
        </div>

        <div className="flex items-center gap-3">
          {usageSupported ? (
            <UsageFooter
              provider={provider}
              providerId={provider.id}
              appId={appId}
              usageEnabled={usageEnabled}
              isCurrent={isCurrent}
              backupProviderId={backupProviderId ?? null}
              onAutoFailover={onAutoFailover}
              inline={true}
            />
          ) : null}

          <ProviderActions
            isCurrent={isCurrent}
            canDeleteCurrent={appId === "openclaw"}
            switchMode={appId === "openclaw" ? "default-model" : "provider"}
            onSwitch={() => onSwitch(provider)}
            onEdit={() => onEdit(provider)}
            onConfigureUsage={() => onConfigureUsage(provider)}
            onStreamCheck={
              onStreamCheck ? () => onStreamCheck(provider) : undefined
            }
            isStreamChecking={isStreamChecking}
            onDelete={() => onDelete(provider)}
            showUsageActions={usageSupported}
          />
        </div>
      </div>
    </div>
  );
}
