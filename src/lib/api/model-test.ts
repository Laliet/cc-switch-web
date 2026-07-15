import { invoke } from "@tauri-apps/api/core";
import type { AppId } from "./types";

// ===== 流式健康检查类型 =====

export type HealthStatus = "operational" | "degraded" | "failed";

export interface StreamCheckConfig {
  timeoutSecs: number;
  maxRetries: number;
  degradedThresholdMs: number;
  claudeModel: string;
  codexModel: string;
  geminiModel: string;
  testPrompt: string;
}

export interface StreamCheckResult {
  status: HealthStatus;
  success: boolean;
  message: string;
  responseTimeMs?: number;
  httpStatus?: number;
  modelUsed: string;
  testedAt: number;
  retryCount: number;
  /** 细粒度错误分类，如 "modelNotFound" */
  errorCategory?: string;
}

export interface StreamCheckLog {
  id: number;
  providerId: string;
  providerName: string;
  appType: string;
  status: HealthStatus;
  success: boolean;
  message: string;
  responseTimeMs?: number;
  httpStatus?: number;
  modelUsed: string;
  retryCount: number;
  errorCategory?: string;
  testedAt: number;
}

export interface StreamCheckLogQuery {
  appType?: AppId;
  providerId?: string;
  status?: HealthStatus;
  since?: number;
  until?: number;
  limit?: number;
  offset?: number;
}

// ===== 流式健康检查 API =====

/**
 * 流式健康检查（单个供应商）
 */
export async function streamCheckProvider(
  appType: AppId,
  providerId: string,
): Promise<StreamCheckResult> {
  return invoke("stream_check_provider", { appType, providerId });
}

/**
 * 批量流式健康检查
 */
export async function streamCheckAllProviders(
  appType: AppId,
  proxyTargetsOnly: boolean = false,
): Promise<Array<[string, StreamCheckResult]>> {
  return invoke("stream_check_all_providers", { appType, proxyTargetsOnly });
}

/**
 * 获取流式检查配置
 */
export async function getStreamCheckConfig(): Promise<StreamCheckConfig> {
  return invoke("get_stream_check_config");
}

/**
 * 保存流式检查配置
 */
export async function saveStreamCheckConfig(
  config: StreamCheckConfig,
): Promise<void> {
  return invoke("save_stream_check_config", { config });
}

export async function getStreamCheckLogs(
  query: StreamCheckLogQuery = {},
): Promise<StreamCheckLog[]> {
  return invoke("get_stream_check_logs", { query });
}

export async function getLatestStreamCheckLogs(
  appType?: AppId,
): Promise<StreamCheckLog[]> {
  return invoke("get_latest_stream_check_logs", { appType });
}
