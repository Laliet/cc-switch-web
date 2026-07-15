import { CSS } from "@dnd-kit/utilities";
import { DndContext, closestCenter } from "@dnd-kit/core";
import {
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { AlertTriangle, PowerOff } from "lucide-react";
import { useEffect, useMemo, useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { Provider } from "@/types";
import { providersApi, type AppId, type ProviderHealth } from "@/lib/api";
import { useDragSort } from "@/hooks/useDragSort";
import { ProviderCard } from "@/components/providers/ProviderCard";
import { ProviderEmptyState } from "@/components/providers/ProviderEmptyState";
import { Button } from "@/components/ui/button";
import { useStreamCheck } from "@/hooks/useStreamCheck";
import { useLatestStreamCheckHistory } from "@/hooks/useStreamCheckHistory";
import { StreamCheckHistoryPanel } from "./StreamCheckHistoryPanel";
import { useOpenClawStatusQuery } from "@/lib/query";

interface ProviderListProps {
  providers: Record<string, Provider>;
  currentProviderId: string;
  backupProviderId?: string | null;
  healthMap?: Record<string, ProviderHealth>;
  appId: AppId;
  isEditMode?: boolean;
  onSwitch: (provider: Provider) => void;
  onEdit: (provider: Provider) => void;
  onDelete: (provider: Provider) => void;
  onDuplicate: (provider: Provider) => void;
  onConfigureUsage?: (provider: Provider) => void;
  onOpenWebsite: (url: string) => void;
  onCreate?: () => void;
  isLoading?: boolean;
  onAutoFailover?: (targetId?: string | null) => void;
  onOmoDisabled?: () => void | Promise<void>;
}

export function ProviderList({
  providers,
  currentProviderId,
  backupProviderId,
  healthMap,
  appId,
  isEditMode = false,
  onSwitch,
  onEdit,
  onDelete,
  onDuplicate,
  onConfigureUsage,
  onOpenWebsite,
  onCreate,
  isLoading = false,
  onAutoFailover,
  onOmoDisabled,
}: ProviderListProps) {
  const { t } = useTranslation();
  const { sortedProviders, sensors, handleDragEnd } = useDragSort(
    providers,
    appId,
  );
  const [omoPluginEnabled, setOmoPluginEnabled] = useState<boolean | null>(
    null,
  );
  const { checkProvider, checkProviders, isChecking, batchProgress } =
    useStreamCheck(appId);
  const { data: latestStreamChecks } = useLatestStreamCheckHistory(appId);
  const { data: openClawStatus } = useOpenClawStatusQuery(appId === "openclaw");
  const [isDisablingOmo, setIsDisablingOmo] = useState(false);
  const [omoDisableError, setOmoDisableError] = useState<string | null>(null);
  const openClawDefaultProviderId = openClawStatus?.defaultModel?.primary.split(
    "/",
    1,
  )[0];
  const effectiveCurrentProviderId =
    appId === "openclaw" && openClawStatus
      ? (openClawDefaultProviderId ?? "")
      : currentProviderId;
  const openClawLiveProviderIds = new Set(
    openClawStatus?.providers.map((provider) => provider.id) ?? [],
  );
  const latestStreamCheckByProvider = useMemo(
    () =>
      new Map(
        (latestStreamChecks ?? []).map((log) => [log.providerId, log] as const),
      ),
    [latestStreamChecks],
  );

  useEffect(() => {
    let cancelled = false;
    if (appId !== "omo" && appId !== "omo-slim") {
      setOmoPluginEnabled(null);
      return;
    }

    const getPluginStatus =
      appId === "omo-slim"
        ? providersApi.getOmoSlimPluginStatus
        : providersApi.getOmoPluginStatus;

    getPluginStatus()
      .then((enabled) => {
        if (!cancelled) setOmoPluginEnabled(enabled);
      })
      .catch(() => {
        if (!cancelled) setOmoPluginEnabled(false);
      });

    return () => {
      cancelled = true;
    };
  }, [appId]);

  const handleDisableCurrentOmo = async () => {
    setIsDisablingOmo(true);
    setOmoDisableError(null);
    try {
      if (appId === "omo-slim") {
        await providersApi.disableCurrentOmoSlim();
      } else {
        await providersApi.disableCurrentOmo();
      }
      setOmoPluginEnabled(false);
      await onOmoDisabled?.();
    } catch (error) {
      setOmoDisableError(error instanceof Error ? error.message : "禁用失败");
    } finally {
      setIsDisablingOmo(false);
    }
  };

  if (isLoading) {
    return (
      <div className="space-y-3">
        {[0, 1, 2].map((index) => (
          <div
            key={index}
            className="h-28 w-full rounded-lg border border-dashed border-muted-foreground/40 bg-muted/40"
          />
        ))}
      </div>
    );
  }

  if (sortedProviders.length === 0) {
    return <ProviderEmptyState onCreate={onCreate} />;
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragEnd={handleDragEnd}
    >
      <SortableContext
        items={sortedProviders.map((provider) => provider.id)}
        strategy={verticalListSortingStrategy}
      >
        <div className="space-y-3">
          {appId === "openclaw" && openClawStatus ? (
            <div className="flex flex-col gap-2 border-y border-border-default bg-muted/30 px-4 py-3 text-sm sm:flex-row sm:items-center sm:justify-between">
              <div className="min-w-0">
                <span className="font-medium">
                  {t("openclaw.defaultModelValue", {
                    defaultValue: "默认模型：{{model}}",
                    model: openClawStatus.defaultModel?.primary ?? "-",
                  })}
                </span>
                <span className="ml-2 text-muted-foreground">
                  {t("openclaw.liveProviders", {
                    defaultValue: "已写入 {{live}} / {{total}} 个 Provider",
                    live: openClawStatus.providers.length,
                    total: sortedProviders.length,
                  })}
                </span>
              </div>
              {openClawStatus.warnings.length > 0 ? (
                <span
                  className="inline-flex items-center gap-1 text-amber-700 dark:text-amber-300"
                  title={openClawStatus.warnings
                    .map((warning) => warning.message)
                    .join("\n")}
                >
                  <AlertTriangle className="h-4 w-4" />
                  {t("openclaw.configWarnings", {
                    defaultValue: "{{count}} 项配置告警",
                    count: openClawStatus.warnings.length,
                  })}
                </span>
              ) : null}
            </div>
          ) : null}
          <StreamCheckHistoryPanel
            appId={appId}
            providers={providers}
            onCheckAll={() => void checkProviders(sortedProviders)}
            batchProgress={batchProgress}
          />
          {(appId === "omo" || appId === "omo-slim") && (
            <div className="flex flex-col gap-3 rounded-lg border border-border-default bg-muted/40 px-4 py-3 text-sm sm:flex-row sm:items-center sm:justify-between">
              <div>
                <span className="font-medium">
                  {appId === "omo-slim"
                    ? "oh-my-opencode-slim@latest"
                    : "oh-my-openagent@latest"}
                </span>
                <span className="ml-2 text-muted-foreground">
                  {omoPluginEnabled === null
                    ? "正在读取 opencode.json plugin 状态..."
                    : omoPluginEnabled
                      ? "已在 opencode.json plugin 中启用"
                      : "未在 opencode.json plugin 中启用"}
                </span>
                {omoDisableError ? (
                  <span className="ml-2 text-red-500">{omoDisableError}</span>
                ) : null}
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="self-start sm:self-auto"
                onClick={handleDisableCurrentOmo}
                disabled={isDisablingOmo}
              >
                <PowerOff className="h-3.5 w-3.5" />
                {isDisablingOmo
                  ? "正在禁用"
                  : appId === "omo-slim"
                    ? "禁用当前 OMO Slim"
                    : "禁用当前 OMO"}
              </Button>
            </div>
          )}
          {sortedProviders.map((provider) => (
            <SortableProviderCard
              key={provider.id}
              provider={provider}
              isCurrent={provider.id === effectiveCurrentProviderId}
              isLiveConfigured={
                appId === "openclaw"
                  ? openClawLiveProviderIds.has(provider.id)
                  : undefined
              }
              backupProviderId={backupProviderId}
              appId={appId}
              isEditMode={isEditMode}
              onSwitch={onSwitch}
              onEdit={onEdit}
              onDelete={onDelete}
              onDuplicate={onDuplicate}
              onConfigureUsage={onConfigureUsage}
              onStreamCheck={
                appId === "omo" || appId === "omo-slim" || appId === "openclaw"
                  ? undefined
                  : (provider) => void checkProvider(provider.id, provider.name)
              }
              isStreamChecking={isChecking(provider.id)}
              onOpenWebsite={onOpenWebsite}
              onAutoFailover={onAutoFailover}
              healthStatus={healthMap?.[provider.id]}
              streamCheckLog={latestStreamCheckByProvider.get(provider.id)}
            />
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );
}

interface SortableProviderCardProps {
  provider: Provider;
  isCurrent: boolean;
  backupProviderId?: string | null;
  healthStatus?: ProviderHealth;
  streamCheckLog?: import("@/lib/api/model-test").StreamCheckLog;
  isLiveConfigured?: boolean;
  appId: AppId;
  isEditMode: boolean;
  onSwitch: (provider: Provider) => void;
  onEdit: (provider: Provider) => void;
  onDelete: (provider: Provider) => void;
  onDuplicate: (provider: Provider) => void;
  onConfigureUsage?: (provider: Provider) => void;
  onStreamCheck?: (provider: Provider) => void;
  isStreamChecking?: boolean;
  onOpenWebsite: (url: string) => void;
  onAutoFailover?: (targetId?: string | null) => void;
}

function SortableProviderCard({
  provider,
  isCurrent,
  backupProviderId,
  healthStatus,
  streamCheckLog,
  isLiveConfigured,
  appId,
  isEditMode,
  onSwitch,
  onEdit,
  onDelete,
  onDuplicate,
  onConfigureUsage,
  onStreamCheck,
  isStreamChecking,
  onOpenWebsite,
  onAutoFailover,
}: SortableProviderCardProps) {
  const {
    setNodeRef,
    attributes,
    listeners,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: provider.id });

  const style: CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div ref={setNodeRef} style={style}>
      <ProviderCard
        provider={provider}
        isCurrent={isCurrent}
        backupProviderId={backupProviderId}
        appId={appId}
        isEditMode={isEditMode}
        onSwitch={onSwitch}
        onEdit={onEdit}
        onDelete={onDelete}
        onDuplicate={onDuplicate}
        onConfigureUsage={
          onConfigureUsage ? (item) => onConfigureUsage(item) : () => undefined
        }
        onStreamCheck={onStreamCheck}
        isStreamChecking={isStreamChecking}
        onOpenWebsite={onOpenWebsite}
        onAutoFailover={onAutoFailover}
        healthStatus={healthStatus}
        streamCheckLog={streamCheckLog}
        isLiveConfigured={isLiveConfigured}
        dragHandleProps={{
          attributes,
          listeners,
          isDragging,
        }}
      />
    </div>
  );
}
