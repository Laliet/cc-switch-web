import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { usageApi } from "@/lib/api/usage";
import { resolveUsageRange } from "@/lib/usageRange";
import type {
  AppTypeFilter,
  LogFilters,
  ModelPricing,
  UsageRangeSelection,
} from "@/types/usage";

const effectiveApp = (appType?: AppTypeFilter) =>
  appType && appType !== "all" ? appType : undefined;

const usageBaseKey = ["usage"] as const;

export const usageKeys = {
  all: usageBaseKey,
  summary: (range: UsageRangeSelection, appType?: AppTypeFilter) =>
    [...usageBaseKey, "summary", range, appType] as const,
  summaryByApp: (range: UsageRangeSelection) =>
    [...usageBaseKey, "summary-by-app", range] as const,
  trends: (range: UsageRangeSelection, appType?: AppTypeFilter) =>
    [...usageBaseKey, "trends", range, appType] as const,
  providers: (range: UsageRangeSelection, appType?: AppTypeFilter) =>
    [...usageBaseKey, "providers", range, appType] as const,
  models: (range: UsageRangeSelection, appType?: AppTypeFilter) =>
    [...usageBaseKey, "models", range, appType] as const,
  logs: (filters: LogFilters, page: number, pageSize: number) =>
    [...usageBaseKey, "logs", filters, page, pageSize] as const,
  detail: (requestId: string | null) =>
    [...usageBaseKey, "detail", requestId] as const,
  pricing: [...usageBaseKey, "pricing"] as const,
  dataSources: [...usageBaseKey, "data-sources"] as const,
};

export function useUsageSummary(
  range: UsageRangeSelection,
  appType?: AppTypeFilter,
  refreshIntervalMs = 0,
) {
  const resolved = resolveUsageRange(range);
  return useQuery({
    queryKey: usageKeys.summary(range, appType),
    queryFn: () =>
      usageApi.getUsageSummary(
        resolved.startDate,
        resolved.endDate,
        effectiveApp(appType),
      ),
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useUsageSummaryByApp(
  range: UsageRangeSelection,
  refreshIntervalMs = 0,
) {
  const resolved = resolveUsageRange(range);
  return useQuery({
    queryKey: usageKeys.summaryByApp(range),
    queryFn: () =>
      usageApi.getUsageSummaryByApp(resolved.startDate, resolved.endDate),
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useUsageTrends(
  range: UsageRangeSelection,
  appType?: AppTypeFilter,
  refreshIntervalMs = 0,
) {
  const resolved = resolveUsageRange(range);
  return useQuery({
    queryKey: usageKeys.trends(range, appType),
    queryFn: () =>
      usageApi.getUsageTrends(
        resolved.startDate,
        resolved.endDate,
        effectiveApp(appType),
      ),
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useProviderStats(
  range: UsageRangeSelection,
  appType?: AppTypeFilter,
  refreshIntervalMs = 0,
) {
  const resolved = resolveUsageRange(range);
  return useQuery({
    queryKey: usageKeys.providers(range, appType),
    queryFn: () =>
      usageApi.getProviderStats(
        resolved.startDate,
        resolved.endDate,
        effectiveApp(appType),
      ),
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useModelStats(
  range: UsageRangeSelection,
  appType?: AppTypeFilter,
  refreshIntervalMs = 0,
) {
  const resolved = resolveUsageRange(range);
  return useQuery({
    queryKey: usageKeys.models(range, appType),
    queryFn: () =>
      usageApi.getModelStats(
        resolved.startDate,
        resolved.endDate,
        effectiveApp(appType),
      ),
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useRequestLogs(
  filters: LogFilters,
  page: number,
  pageSize: number,
  refreshIntervalMs = 0,
) {
  return useQuery({
    queryKey: usageKeys.logs(filters, page, pageSize),
    queryFn: () => usageApi.getRequestLogs(filters, page, pageSize),
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useRequestDetail(requestId: string | null) {
  return useQuery({
    queryKey: usageKeys.detail(requestId),
    queryFn: () => usageApi.getRequestDetail(requestId ?? ""),
    enabled: Boolean(requestId),
  });
}

export function useModelPricing() {
  return useQuery({
    queryKey: usageKeys.pricing,
    queryFn: usageApi.getModelPricing,
  });
}

export function useDataSources(refreshIntervalMs = 0) {
  return useQuery({
    queryKey: usageKeys.dataSources,
    queryFn: usageApi.getDataSourceBreakdown,
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useUpdateModelPricing() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (record: ModelPricing) => usageApi.updateModelPricing(record),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: usageKeys.all });
    },
  });
}

export function useDeleteModelPricing() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (modelId: string) => usageApi.deleteModelPricing(modelId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: usageKeys.all });
    },
  });
}
