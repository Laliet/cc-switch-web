import { invoke } from "@tauri-apps/api/core";
import { isWeb } from "./adapter";
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

// ===== Web 模式适配 =====
// 流式健康检查依赖 host 端 Tauri command（stream_check_*），仅在桌面 native 运行时可用。
// web/headless 模式下 capabilities.endpointTest === false，Tauri invoke 不可用，
// 若直接调用会抛 "Cannot read properties of undefined (reading 'invoke')"。
// 因此 web 模式下返回安全的空值/noop，保持 UI 可用（历史为空、测试按钮不触发真实请求）。
const STREAM_CHECK_DISABLED_WEB_MESSAGE =
  "Stream check is not available in web/headless mode";

const DEFAULT_WEB_STREAM_CHECK_CONFIG: StreamCheckConfig = {
  timeoutSecs: 30,
  maxRetries: 1,
  degradedThresholdMs: 3000,
  claudeModel: "",
  codexModel: "",
  geminiModel: "",
  testPrompt: "",
};

// ===== 流式健康检查 API =====

/**
 * 流式健康检查（单个供应商）
 */
export async function streamCheckProvider(
  appType: AppId,
  providerId: string,
): Promise<StreamCheckResult> {
  if (isWeb()) {
    return {
      status: "failed",
      success: false,
      message: STREAM_CHECK_DISABLED_WEB_MESSAGE,
      modelUsed: "",
      testedAt: Date.now(),
      retryCount: 0,
    };
  }
  return invoke("stream_check_provider", { appType, providerId });
}

/**
 * 批量流式健康检查
 */
export async function streamCheckAllProviders(
  appType: AppId,
  proxyTargetsOnly: boolean = false,
): Promise<Array<[string, StreamCheckResult]>> {
  if (isWeb()) {
    return [];
  }
  return invoke("stream_check_all_providers", { appType, proxyTargetsOnly });
}

/**
 * 获取流式检查配置
 */
export async function getStreamCheckConfig(): Promise<StreamCheckConfig> {
  if (isWeb()) {
    return DEFAULT_WEB_STREAM_CHECK_CONFIG;
  }
  return invoke("get_stream_check_config");
}

/**
 * 保存流式检查配置
 */
export async function saveStreamCheckConfig(
  config: StreamCheckConfig,
): Promise<void> {
  if (isWeb()) {
    return;
  }
  return invoke("save_stream_check_config", { config });
}

export async function getStreamCheckLogs(
  query: StreamCheckLogQuery = {},
): Promise<StreamCheckLog[]> {
  if (isWeb()) {
    return [];
  }
  return invoke("get_stream_check_logs", { query });
}

export async function getLatestStreamCheckLogs(
  appType?: AppId,
): Promise<StreamCheckLog[]> {
  if (isWeb()) {
    return [];
  }
  return invoke("get_latest_stream_check_logs", { appType });
}
