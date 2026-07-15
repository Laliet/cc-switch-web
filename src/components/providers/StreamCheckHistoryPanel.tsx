import { useMemo, useState } from "react";
import { Activity, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AppId } from "@/lib/api";
import type { HealthStatus, StreamCheckLog } from "@/lib/api/model-test";
import { useStreamCheckHistory } from "@/hooks/useStreamCheckHistory";
import { Button } from "@/components/ui/button";
import type { StreamCheckBatchProgress } from "@/hooks/useStreamCheck";

interface StreamCheckHistoryPanelProps {
  appId: AppId;
  providers: Record<string, { id: string; name: string }>;
  onCheckAll: () => void;
  batchProgress: StreamCheckBatchProgress;
}

const statusLabel: Record<HealthStatus, string> = {
  operational: "Operational",
  degraded: "Degraded",
  failed: "Failed",
};

export function StreamCheckHistoryPanel({
  appId,
  providers,
  onCheckAll,
  batchProgress,
}: StreamCheckHistoryPanelProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<HealthStatus | "">("");
  const [providerId, setProviderId] = useState("");
  const [timeRange, setTimeRange] = useState<"24h" | "7d" | "30d" | "all">(
    "7d",
  );
  const since = useMemo(() => {
    const seconds = { "24h": 86_400, "7d": 604_800, "30d": 2_592_000 };
    return timeRange === "all"
      ? undefined
      : Math.floor(Date.now() / 1000) - seconds[timeRange];
  }, [timeRange]);
  const query = useStreamCheckHistory(appId, {
    status: status || undefined,
    providerId: providerId || undefined,
    since,
    limit: 100,
  });

  const logs = useMemo(() => query.data ?? [], [query.data]);
  if (["omo", "omo-slim", "openclaw"].includes(appId)) return null;

  return (
    <section className="rounded-lg border border-border-default bg-card p-4 shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold">
            {t("streamCheck.historyTitle", {
              defaultValue: "Stream Check history",
            })}
          </h2>
          <p className="text-xs text-muted-foreground">
            {t("streamCheck.historySubtitle", {
              defaultValue: "Recent provider diagnostics",
            })}
          </p>
        </div>
        <div className="flex items-center gap-1">
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={onCheckAll}
            disabled={batchProgress.running}
          >
            <Activity
              className={batchProgress.running ? "animate-pulse" : ""}
            />
            {batchProgress.running
              ? t("streamCheck.batchProgress", {
                  defaultValue: "{{completed}} / {{total}}",
                  completed: batchProgress.completed,
                  total: batchProgress.total,
                })
              : t("streamCheck.checkAll", { defaultValue: "检查全部" })}
          </Button>
          <Button
            type="button"
            size="icon"
            variant="ghost"
            onClick={() => void query.refetch()}
            disabled={query.isFetching}
            title={t("common.refresh", { defaultValue: "Refresh" })}
          >
            <RefreshCw className={query.isFetching ? "animate-spin" : ""} />
          </Button>
        </div>
      </div>

      {batchProgress.total > 0 ? (
        <div className="mt-3 space-y-1" aria-live="polite">
          <div className="h-1.5 overflow-hidden rounded-full bg-muted">
            <div
              className="h-full bg-primary transition-[width]"
              style={{
                width: `${Math.round(
                  (batchProgress.completed / batchProgress.total) * 100,
                )}%`,
              }}
            />
          </div>
          <p className="text-xs text-muted-foreground">
            {t("streamCheck.batchSummary", {
              defaultValue: "已完成 {{completed}} / {{total}}，失败 {{failed}}",
              completed: batchProgress.completed,
              total: batchProgress.total,
              failed: batchProgress.failed,
            })}
          </p>
        </div>
      ) : null}

      <div className="mt-3 flex flex-wrap gap-2">
        <select
          className="h-8 rounded-md border border-border-default bg-background px-2 text-xs"
          value={providerId}
          onChange={(event) => setProviderId(event.target.value)}
          aria-label={t("streamCheck.providerFilter", {
            defaultValue: "Provider",
          })}
        >
          <option value="">All providers</option>
          {Object.values(providers).map((provider) => (
            <option key={provider.id} value={provider.id}>
              {provider.name}
            </option>
          ))}
        </select>
        <select
          className="h-8 rounded-md border border-border-default bg-background px-2 text-xs"
          value={timeRange}
          onChange={(event) =>
            setTimeRange(event.target.value as typeof timeRange)
          }
          aria-label={t("streamCheck.timeFilter", {
            defaultValue: "时间范围",
          })}
        >
          <option value="24h">
            {t("streamCheck.last24Hours", { defaultValue: "最近 24 小时" })}
          </option>
          <option value="7d">
            {t("streamCheck.last7Days", { defaultValue: "最近 7 天" })}
          </option>
          <option value="30d">
            {t("streamCheck.last30Days", { defaultValue: "最近 30 天" })}
          </option>
          <option value="all">
            {t("streamCheck.allTime", { defaultValue: "全部时间" })}
          </option>
        </select>
        <select
          className="h-8 rounded-md border border-border-default bg-background px-2 text-xs"
          value={status}
          onChange={(event) =>
            setStatus(event.target.value as HealthStatus | "")
          }
          aria-label={t("streamCheck.statusFilter", {
            defaultValue: "Status",
          })}
        >
          <option value="">All statuses</option>
          {(Object.keys(statusLabel) as HealthStatus[]).map((value) => (
            <option key={value} value={value}>
              {statusLabel[value]}
            </option>
          ))}
        </select>
      </div>

      {query.isError ? (
        <p className="mt-3 text-sm text-destructive">
          {query.error instanceof Error
            ? query.error.message
            : "Failed to load diagnostics"}
        </p>
      ) : logs.length === 0 ? (
        <p className="mt-3 text-sm text-muted-foreground">
          {query.isLoading ? "Loading..." : "No stream checks recorded"}
        </p>
      ) : (
        <div className="mt-3 divide-y divide-border-default">
          {logs.map((log) => (
            <LogRow key={log.id} log={log} />
          ))}
        </div>
      )}
    </section>
  );
}

function LogRow({ log }: { log: StreamCheckLog }) {
  const time = new Date(log.testedAt * 1000).toLocaleString();
  return (
    <details className="py-2 text-xs">
      <summary className="flex cursor-pointer list-none flex-wrap items-center gap-2">
        <span
          className={
            log.success
              ? "h-2 w-2 rounded-full bg-emerald-500"
              : "h-2 w-2 rounded-full bg-red-500"
          }
        />
        <span className="font-medium">{log.providerName}</span>
        <span className="text-muted-foreground">{log.status}</span>
        {log.responseTimeMs != null ? (
          <span className="text-muted-foreground">{log.responseTimeMs}ms</span>
        ) : null}
        <span className="ml-auto text-muted-foreground">{time}</span>
      </summary>
      <div className="mt-2 grid gap-1 rounded-md bg-muted/40 px-3 py-2 text-muted-foreground sm:grid-cols-2">
        <span>Model: {log.modelUsed || "-"}</span>
        <span>HTTP: {log.httpStatus ?? "-"}</span>
        <span>Retries: {log.retryCount}</span>
        <span>Error: {log.errorCategory ?? "-"}</span>
        <span className="sm:col-span-2 break-words text-foreground">
          {log.message}
        </span>
      </div>
    </details>
  );
}
