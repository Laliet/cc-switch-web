import { invoke } from "./adapter";
import type { UsageResult } from "@/types";
import type { TemplateType } from "@/config/constants";
import type { AppId } from "./types";
import i18n from "@/i18n";
import type {
  DailyStats,
  DataSourceSummary,
  LogFilters,
  ModelPricing,
  ModelStats,
  PaginatedLogs,
  ProviderLimitStatus,
  ProviderStats,
  RequestLog,
  SessionSyncResult,
  UsageDataExtent,
  UsageSummary,
  UsageStatsFilters,
  UsageSummaryByApp,
} from "@/types/usage";

const statsArgs = (
  startDate?: number,
  endDate?: number,
  appType?: string,
  filters?: UsageStatsFilters,
) => ({
  startDate,
  endDate,
  appType,
  providerId: filters?.providerId,
  model: filters?.model,
});

export const usageApi = {
  async query(providerId: string, appId: AppId): Promise<UsageResult> {
    try {
      return await invoke("queryProviderUsage", {
        providerId: providerId,
        app: appId,
      });
    } catch (error: unknown) {
      // 提取错误消息：优先使用后端返回的错误信息
      const message =
        typeof error === "string"
          ? error
          : error instanceof Error
            ? error.message
            : "";

      // 如果没有错误消息，使用国际化的默认提示
      return {
        success: false,
        error: message || i18n.t("errors.usage_query_failed"),
      };
    }
  },

  async testScript(
    providerId: string,
    appId: AppId,
    scriptCode: string,
    timeout?: number,
    apiKey?: string,
    baseUrl?: string,
    accessToken?: string,
    userId?: string,
    templateType?: TemplateType,
  ): Promise<UsageResult> {
    try {
      return await invoke("testUsageScript", {
        providerId: providerId,
        app: appId,
        scriptCode: scriptCode,
        timeout: timeout,
        apiKey: apiKey,
        baseUrl: baseUrl,
        accessToken: accessToken,
        userId: userId,
        templateType: templateType,
      });
    } catch (error: unknown) {
      const message =
        typeof error === "string"
          ? error
          : error instanceof Error
            ? error.message
            : "";

      return {
        success: false,
        error: message || i18n.t("errors.usage_query_failed"),
      };
    }
  },

  async getUsageSummary(
    startDate?: number,
    endDate?: number,
    appType?: string,
    filters?: UsageStatsFilters,
  ): Promise<UsageSummary> {
    return await invoke(
      "get_usage_summary",
      statsArgs(startDate, endDate, appType, filters),
    );
  },

  async getUsageSummaryByApp(
    startDate?: number,
    endDate?: number,
  ): Promise<UsageSummaryByApp[]> {
    return await invoke("get_usage_summary_by_app", { startDate, endDate });
  },

  async getUsageTrends(
    startDate?: number,
    endDate?: number,
    appType?: string,
    filters?: UsageStatsFilters,
  ): Promise<DailyStats[]> {
    return await invoke(
      "get_usage_trends",
      statsArgs(startDate, endDate, appType, filters),
    );
  },

  async getProviderStats(
    startDate?: number,
    endDate?: number,
    appType?: string,
    filters?: UsageStatsFilters,
  ): Promise<ProviderStats[]> {
    return await invoke(
      "get_provider_stats",
      statsArgs(startDate, endDate, appType, filters),
    );
  },

  async getModelStats(
    startDate?: number,
    endDate?: number,
    appType?: string,
    filters?: UsageStatsFilters,
  ): Promise<ModelStats[]> {
    return await invoke(
      "get_model_stats",
      statsArgs(startDate, endDate, appType, filters),
    );
  },

  async getRequestLogs(
    filters: LogFilters,
    page = 0,
    pageSize = 20,
  ): Promise<PaginatedLogs> {
    return await invoke("get_request_logs", { filters, page, pageSize });
  },

  async getRequestDetail(requestId: string): Promise<RequestLog | null> {
    return await invoke("get_request_detail", { requestId });
  },

  async getModelPricing(): Promise<ModelPricing[]> {
    return await invoke("get_model_pricing");
  },

  async updateModelPricing(record: ModelPricing): Promise<number> {
    return await invoke("update_model_pricing", {
      modelId: record.modelId,
      displayName: record.displayName,
      inputCost: record.inputCostPerMillion,
      outputCost: record.outputCostPerMillion,
      cacheReadCost: record.cacheReadCostPerMillion,
      cacheCreationCost: record.cacheCreationCostPerMillion,
    });
  },

  async deleteModelPricing(modelId: string): Promise<boolean> {
    return await invoke("delete_model_pricing", { modelId });
  },

  async checkProviderLimits(
    providerId: string,
    appType: string,
  ): Promise<ProviderLimitStatus> {
    return await invoke("check_provider_limits", { providerId, appType });
  },

  async syncSessionUsage(): Promise<SessionSyncResult> {
    return await invoke("sync_session_usage");
  },

  async getDataSourceBreakdown(): Promise<DataSourceSummary[]> {
    return await invoke("get_usage_data_sources");
  },

  async getUsageDataExtent(appType?: string): Promise<UsageDataExtent> {
    return await invoke("get_usage_data_extent", { appType });
  },
};
