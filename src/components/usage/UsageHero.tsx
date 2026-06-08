import { Activity, Coins, Gauge, Zap } from "lucide-react";
import { useUsageSummary, useUsageSummaryByApp } from "@/lib/query/usage";
import {
  usageAppLabel,
  type AppTypeFilter,
  type UsageRangeSelection,
  type UsageStatsFilters,
} from "@/types/usage";
import { formatNumber, formatPercent, formatUsd } from "./format";

interface UsageHeroProps {
  range: UsageRangeSelection;
  appType: AppTypeFilter;
  filters?: UsageStatsFilters;
  refreshIntervalMs: number;
}

export function UsageHero({
  range,
  appType,
  filters,
  refreshIntervalMs,
}: UsageHeroProps) {
  const summary = useUsageSummary(range, appType, filters, refreshIntervalMs);
  const byApp = useUsageSummaryByApp(range, refreshIntervalMs);
  const data = summary.data;

  const cards = [
    {
      label: "Total cost",
      value: formatUsd(data?.totalCost ?? "0"),
      icon: Coins,
    },
    {
      label: "Requests",
      value: formatNumber(data?.totalRequests ?? 0),
      icon: Activity,
    },
    {
      label: "Real tokens",
      value: formatNumber(data?.realTotalTokens ?? 0),
      icon: Zap,
    },
    {
      label: "Success rate",
      value: formatPercent(data?.successRate ?? 0),
      icon: Gauge,
    },
  ];

  return (
    <div className="space-y-4">
      <div className="grid gap-3 md:grid-cols-4">
        {cards.map((card) => {
          const Icon = card.icon;
          return (
            <div
              key={card.label}
              className="rounded-lg border border-border-default bg-card p-4 shadow-sm"
            >
              <div className="mb-3 flex items-center justify-between text-sm text-muted-foreground">
                <span>{card.label}</span>
                <Icon className="h-4 w-4" />
              </div>
              <div className="text-2xl font-semibold tracking-normal">
                {summary.isLoading ? "..." : card.value}
              </div>
            </div>
          );
        })}
      </div>

      <div className="rounded-lg border border-border-default bg-card p-4">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
          <div>
            <h3 className="text-sm font-semibold">Token breakdown</h3>
            <p className="text-xs text-muted-foreground">
              Fresh input excludes cached reads for OpenAI-style protocols.
            </p>
          </div>
          <div className="text-sm text-muted-foreground">
            Cache hit {formatPercent((data?.cacheHitRate ?? 0) * 100)}
          </div>
        </div>
        <div className="grid gap-2 text-sm md:grid-cols-4">
          <Metric label="Input" value={data?.totalInputTokens ?? 0} />
          <Metric label="Output" value={data?.totalOutputTokens ?? 0} />
          <Metric
            label="Cache create"
            value={data?.totalCacheCreationTokens ?? 0}
          />
          <Metric label="Cache read" value={data?.totalCacheReadTokens ?? 0} />
        </div>
        {appType === "all" && byApp.data && byApp.data.length > 0 ? (
          <div className="mt-4 flex flex-wrap gap-2">
            {byApp.data.map((item) => (
              <span
                key={item.appType}
                className="rounded-md border border-border-default bg-muted/40 px-2 py-1 text-xs"
              >
                {usageAppLabel(item.appType)}:{" "}
                {formatUsd(item.summary.totalCost)}
              </span>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-md bg-muted/40 px-3 py-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="font-medium">{formatNumber(value)}</div>
    </div>
  );
}
