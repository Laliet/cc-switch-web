import { invoke } from "./adapter";
import type { UsageResult } from "@/types";
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
  UsageSummary,
  UsageSummaryByApp,
} from "@/types/usage";

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
  ): Promise<UsageSummary> {
    return await invoke("get_usage_summary", { startDate, endDate, appType });
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
  ): Promise<DailyStats[]> {
    return await invoke("get_usage_trends", { startDate, endDate, appType });
  },

  async getProviderStats(
    startDate?: number,
    endDate?: number,
    appType?: string,
  ): Promise<ProviderStats[]> {
    return await invoke("get_provider_stats", { startDate, endDate, appType });
  },

  async getModelStats(
    startDate?: number,
    endDate?: number,
    appType?: string,
  ): Promise<ModelStats[]> {
    return await invoke("get_model_stats", { startDate, endDate, appType });
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
};
