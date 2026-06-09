import { useMemo } from "react";
import { useUsageTrends } from "@/lib/query/usage";
import type {
  AppTypeFilter,
  UsageRangeSelection,
  UsageStatsFilters,
} from "@/types/usage";
import { compactDate, formatUsd } from "./format";

interface UsageTrendChartProps {
  range: UsageRangeSelection;
  appType: AppTypeFilter;
  filters?: UsageStatsFilters;
  refreshIntervalMs: number;
}

export function UsageTrendChart({
  range,
  appType,
  filters,
  refreshIntervalMs,
}: UsageTrendChartProps) {
  const trends = useUsageTrends(range, appType, filters, refreshIntervalMs);
  const maxCost = useMemo(() => {
    return Math.max(
      0,
      ...(trends.data ?? []).map((item) =>
        Number.parseFloat(item.totalCost || "0"),
      ),
    );
  }, [trends.data]);

  return (
    <div className="rounded-lg border border-border-default bg-card p-4">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold">Usage trend</h3>
          <p className="text-xs text-muted-foreground">
            Cost and request volume by local time bucket.
          </p>
        </div>
      </div>
      <div className="flex h-56 items-end gap-2 overflow-x-auto pb-2">
        {(trends.data ?? []).length === 0 ? (
          <div className="flex h-full w-full items-center justify-center text-sm text-muted-foreground">
            No usage data
          </div>
        ) : (
          trends.data?.map((item) => {
            const cost = Number.parseFloat(item.totalCost || "0");
            const height =
              maxCost > 0 ? Math.max(6, (cost / maxCost) * 160) : 6;
            return (
              <div
                key={item.date}
                className="flex min-w-[42px] flex-1 flex-col items-center gap-2"
                title={`${item.date}\n${formatUsd(item.totalCost)}\n${item.requestCount} requests`}
              >
                <div className="flex h-40 items-end">
                  <div
                    className="w-7 rounded-t bg-blue-500/80"
                    style={{ height }}
                  />
                </div>
                <div className="text-[11px] text-muted-foreground">
                  {compactDate(item.date)}
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
