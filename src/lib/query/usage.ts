import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { usageApi } from "@/lib/api/usage";
import { resolveUsageRange } from "@/lib/usageRange";
import type {
  AppTypeFilter,
  LogFilters,
  ModelPricing,
  UsageStatsFilters,
  UsageRangeSelection,
} from "@/types/usage";

const effectiveApp = (appType?: AppTypeFilter) =>
  appType && appType !== "all" ? appType : undefined;

const usageBaseKey = ["usage"] as const;

const normalizeStatsFilters = (filters?: UsageStatsFilters) => ({
  providerId: filters?.providerId?.trim() || undefined,
  model: filters?.model?.trim() || undefined,
});

export const usageKeys = {
  all: usageBaseKey,
  summary: (
    range: UsageRangeSelection,
    appType?: AppTypeFilter,
    filters?: UsageStatsFilters,
  ) => [...usageBaseKey, "summary", range, appType, filters] as const,
  summaryByApp: (range: UsageRangeSelection) =>
    [...usageBaseKey, "summary-by-app", range] as const,
  trends: (
    range: UsageRangeSelection,
    appType?: AppTypeFilter,
    filters?: UsageStatsFilters,
  ) => [...usageBaseKey, "trends", range, appType, filters] as const,
  providers: (
    range: UsageRangeSelection,
    appType?: AppTypeFilter,
    filters?: UsageStatsFilters,
  ) => [...usageBaseKey, "providers", range, appType, filters] as const,
  models: (
    range: UsageRangeSelection,
    appType?: AppTypeFilter,
    filters?: UsageStatsFilters,
  ) => [...usageBaseKey, "models", range, appType, filters] as const,
  logs: (
    range: UsageRangeSelection,
    filters: LogFilters,
    page: number,
    pageSize: number,
  ) => [...usageBaseKey, "logs", range, filters, page, pageSize] as const,
  detail: (requestId: string | null) =>
    [...usageBaseKey, "detail", requestId] as const,
  pricing: [...usageBaseKey, "pricing"] as const,
  dataSources: [...usageBaseKey, "data-sources"] as const,
  dataExtent: (appType?: AppTypeFilter) =>
    [...usageBaseKey, "data-extent", appType] as const,
};

export function useUsageSummary(
  range: UsageRangeSelection,
  appType?: AppTypeFilter,
  filters?: UsageStatsFilters,
  refreshIntervalMs = 0,
) {
  const statsFilters = normalizeStatsFilters(filters);
  return useQuery({
    queryKey: usageKeys.summary(range, appType, statsFilters),
    queryFn: () => {
      const resolved = resolveUsageRange(range);
      return usageApi.getUsageSummary(
        resolved.startDate,
        resolved.endDate,
        effectiveApp(appType),
        statsFilters,
      );
    },
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useUsageSummaryByApp(
  range: UsageRangeSelection,
  refreshIntervalMs = 0,
) {
  return useQuery({
    queryKey: usageKeys.summaryByApp(range),
    queryFn: () => {
      const resolved = resolveUsageRange(range);
      return usageApi.getUsageSummaryByApp(
        resolved.startDate,
        resolved.endDate,
      );
    },
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useUsageTrends(
  range: UsageRangeSelection,
  appType?: AppTypeFilter,
  filters?: UsageStatsFilters,
  refreshIntervalMs = 0,
) {
  const statsFilters = normalizeStatsFilters(filters);
  return useQuery({
    queryKey: usageKeys.trends(range, appType, statsFilters),
    queryFn: () => {
      const resolved = resolveUsageRange(range);
      return usageApi.getUsageTrends(
        resolved.startDate,
        resolved.endDate,
        effectiveApp(appType),
        statsFilters,
      );
    },
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useProviderStats(
  range: UsageRangeSelection,
  appType?: AppTypeFilter,
  filters?: UsageStatsFilters,
  refreshIntervalMs = 0,
) {
  const statsFilters = normalizeStatsFilters(filters);
  return useQuery({
    queryKey: usageKeys.providers(range, appType, statsFilters),
    queryFn: () => {
      const resolved = resolveUsageRange(range);
      return usageApi.getProviderStats(
        resolved.startDate,
        resolved.endDate,
        effectiveApp(appType),
        statsFilters,
      );
    },
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useModelStats(
  range: UsageRangeSelection,
  appType?: AppTypeFilter,
  filters?: UsageStatsFilters,
  refreshIntervalMs = 0,
) {
  const statsFilters = normalizeStatsFilters(filters);
  return useQuery({
    queryKey: usageKeys.models(range, appType, statsFilters),
    queryFn: () => {
      const resolved = resolveUsageRange(range);
      return usageApi.getModelStats(
        resolved.startDate,
        resolved.endDate,
        effectiveApp(appType),
        statsFilters,
      );
    },
    refetchInterval: refreshIntervalMs > 0 ? refreshIntervalMs : false,
  });
}

export function useRequestLogs(
  range: UsageRangeSelection,
  filters: LogFilters,
  page: number,
  pageSize: number,
  refreshIntervalMs = 0,
) {
  return useQuery({
    queryKey: usageKeys.logs(range, filters, page, pageSize),
    queryFn: () => {
      const resolved = resolveUsageRange(range);
      return usageApi.getRequestLogs(
        {
          ...filters,
          startDate: resolved.startDate,
          endDate: resolved.endDate,
        },
        page,
        pageSize,
      );
    },
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

export function useUsageDataExtent(
  appType?: AppTypeFilter,
  refreshIntervalMs = 0,
) {
  return useQuery({
    queryKey: usageKeys.dataExtent(appType),
    queryFn: () => usageApi.getUsageDataExtent(effectiveApp(appType)),
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
