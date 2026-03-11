/**
 * Relay-Pulse 健康检查 API 模块
 * 使用 https://relaypulse.top 公开 API 获取供应商健康状态
 */

import type { AppId } from "./types";
import { buildWebAuthHeadersForUrl, invoke, isWeb } from "./adapter";

const RELAY_PULSE_API = "/api/health/status";
const CACHE_TTL = 60 * 1000; // 1 分钟缓存
const HEALTHCHECK_TIMEOUT_MS = 10_000;

/** 健康状态枚举 */
export type HealthStatus = "available" | "degraded" | "unavailable" | "unknown";

/** 单个供应商的健康信息 */
export interface ProviderHealth {
  /** 是否健康（available 或 degraded 视为健康） */
  isHealthy: boolean;
  /** 健康状态 */
  status: HealthStatus;
  /** 响应延迟（毫秒） */
  latency: number;
  /** 最后检查时间（Unix 时间戳毫秒） */
  lastChecked: number;
  /** 24小时可用率（百分比） */
  availability?: number;
}

/** Relay-Pulse API 响应结构 */
interface RelayPulseResponse {
  meta: { period: string; count: number };
  data?: RelayPulseMonitor[];
  groups?: RelayPulseGroup[];
}

interface RelayPulseMonitor {
  provider: string;
  provider_url: string;
  service: string; // "cc" | "cx"
  category: string;
  current_status: {
    status: number; // 1=正常, 2=降级, 0=不可用
    latency: number;
    timestamp: number;
  };
  timeline: Array<{
    availability: number;
  }>;
}

interface RelayPulseLayer {
  current_status?: {
    status?: number;
    latency?: number;
    timestamp?: number;
  };
  timeline?: Array<{
    availability: number;
  }>;
}

interface RelayPulseGroup {
  provider: string;
  service: string;
  current_status?: number;
  timeline?: Array<{
    availability: number;
  }>;
  layers?: RelayPulseLayer[];
}

// 健康状态缓存
let healthCache: Map<string, ProviderHealth> = new Map();
let lastFetchTime = 0;

/**
 * 将 relay-pulse status 数值转换为状态枚举
 */
function statusToHealth(status: number): HealthStatus {
  switch (status) {
    case 1:
      return "available";
    case 2:
      return "degraded";
    case 0:
      return "unavailable";
    default:
      return "unknown";
  }
}

/**
 * 计算 24 小时平均可用率
 */
function calculateAvailability(
  timeline: Array<{ availability: number }>,
): number | undefined {
  if (!timeline || timeline.length === 0) return undefined;
  const validPoints = timeline.filter((t) => t.availability >= 0);
  if (validPoints.length === 0) return undefined;
  const sum = validPoints.reduce((acc, t) => acc + t.availability, 0);
  return sum / validPoints.length;
}

function createTimeoutError(timeoutMs: number): Error {
  const error = new Error(`Request timed out after ${timeoutMs}ms`);
  (error as { name?: string }).name = "AbortError";
  return error;
}

function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    return promise;
  }

  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(createTimeoutError(timeoutMs));
    }, timeoutMs);

    promise.then(resolve, reject).finally(() => clearTimeout(timer));
  });
}

export { statusToHealth, calculateAvailability, mergeHealth };

function toProviderHealth(
  status: { status?: number; latency?: number; timestamp?: number } | undefined,
  timeline?: Array<{ availability: number }>,
): ProviderHealth | null {
  if (!status || typeof status.status !== "number") {
    return null;
  }

  const healthStatus = statusToHealth(status.status);
  return {
    isHealthy: healthStatus === "available" || healthStatus === "degraded",
    status: healthStatus,
    latency:
      typeof status.latency === "number" && Number.isFinite(status.latency)
        ? status.latency
        : 0,
    lastChecked:
      typeof status.timestamp === "number" && Number.isFinite(status.timestamp)
        ? status.timestamp * 1000
        : Date.now(),
    availability: calculateAvailability(timeline ?? []),
  };
}

function normalizeRelayPulseResponse(
  payload: RelayPulseResponse,
): Map<string, ProviderHealth> {
  const nextCache = new Map<string, ProviderHealth>();

  const mergeEntry = (key: string, health: ProviderHealth | null) => {
    if (!health) return;
    nextCache.set(key, mergeHealth(nextCache.get(key), health));
  };

  if (Array.isArray(payload.groups) && payload.groups.length > 0) {
    for (const group of payload.groups) {
      const key = `${group.provider.toLowerCase()}/${group.service}`;
      const layers = Array.isArray(group.layers) ? group.layers : [];

      if (layers.length > 0) {
        for (const layer of layers) {
          mergeEntry(
            key,
            toProviderHealth(layer.current_status, layer.timeline),
          );
        }
        continue;
      }

      mergeEntry(
        key,
        toProviderHealth(
          typeof group.current_status === "number"
            ? { status: group.current_status }
            : undefined,
          group.timeline,
        ),
      );
    }

    return nextCache;
  }

  for (const monitor of payload.data ?? []) {
    const key = `${monitor.provider.toLowerCase()}/${monitor.service}`;
    mergeEntry(key, toProviderHealth(monitor.current_status, monitor.timeline));
  }

  return nextCache;
}

function mergeHealth(
  existing: ProviderHealth | undefined,
  incoming: ProviderHealth,
): ProviderHealth {
  if (!existing) return incoming;

  const statusPriority: Record<HealthStatus, number> = {
    unavailable: 0,
    degraded: 1,
    unknown: 2,
    available: 3,
  };

  const worseStatus =
    statusPriority[incoming.status] < statusPriority[existing.status]
      ? incoming.status
      : existing.status;

  return {
    isHealthy: worseStatus === "available" || worseStatus === "degraded",
    status: worseStatus,
    latency: Math.max(existing.latency, incoming.latency),
    lastChecked: Math.max(existing.lastChecked, incoming.lastChecked),
    availability:
      existing.availability !== undefined || incoming.availability !== undefined
        ? Math.min(existing.availability ?? 100, incoming.availability ?? 100)
        : undefined,
  };
}

/**
 * 获取所有供应商的健康状态（带缓存）
 */
export async function fetchAllHealthStatus(): Promise<
  Map<string, ProviderHealth>
> {
  const now = Date.now();

  // 检查缓存是否有效
  if (now - lastFetchTime < CACHE_TTL && healthCache.size > 0) {
    return healthCache;
  }

  try {
    let data: RelayPulseResponse;
    if (!isWeb()) {
      // GUI 模式：通过 Tauri 命令代理请求 Relay-Pulse（无需 Authorization header）
      data = await withTimeout(
        invoke<RelayPulseResponse>("check_relay_pulse"),
        HEALTHCHECK_TIMEOUT_MS,
      );
    } else {
      // Web 模式：通过内置 web-server 代理路由访问（支持 Basic Auth）
      const controller = new AbortController();
      const timer = setTimeout(
        () => controller.abort(),
        HEALTHCHECK_TIMEOUT_MS,
      );

      try {
        const headers: Record<string, string> = {
          Accept: "application/json",
          ...buildWebAuthHeadersForUrl(RELAY_PULSE_API),
        };
        const response = await fetch(RELAY_PULSE_API, {
          headers,
          signal: controller.signal,
        });

        if (!response.ok) {
          console.warn(`[HealthCheck] API returned ${response.status}`);
          return healthCache; // 返回旧缓存
        }

        data = await response.json();
      } finally {
        clearTimeout(timer);
      }
    }

    lastFetchTime = now;

    healthCache = normalizeRelayPulseResponse(data);
    return healthCache;
  } catch (error) {
    if ((error as any)?.name === "AbortError") {
      console.warn(
        `[HealthCheck] Request timed out after ${HEALTHCHECK_TIMEOUT_MS}ms`,
      );
    } else {
      console.warn("[HealthCheck] Failed to fetch health status:", error);
    }
    return healthCache; // 返回旧缓存
  }
}

/**
 * 检查单个供应商的健康状态
 * @param relayPulseProvider relay-pulse 中的供应商名称（小写）
 * @param service 服务类型 "cc" 或 "cx"
 */
export async function checkProviderHealth(
  relayPulseProvider: string,
  service: "cc" | "cx" = "cc",
): Promise<ProviderHealth> {
  const healthMap = await fetchAllHealthStatus();
  const key = `${relayPulseProvider.toLowerCase()}/${service}`;

  return (
    healthMap.get(key) || {
      isHealthy: false,
      status: "unknown",
      latency: 0,
      lastChecked: Date.now(),
    }
  );
}

/**
 * 批量检查多个供应商的健康状态
 * @param providers 供应商映射数组 [{ relayPulseProvider, service }]
 */
export async function checkProvidersHealth(
  providers: Array<{ relayPulseProvider: string; service: "cc" | "cx" }>,
): Promise<Map<string, ProviderHealth>> {
  const healthMap = await fetchAllHealthStatus();
  const result = new Map<string, ProviderHealth>();

  for (const { relayPulseProvider, service } of providers) {
    const key = `${relayPulseProvider.toLowerCase()}/${service}`;
    result.set(
      key,
      healthMap.get(key) || {
        isHealthy: false,
        status: "unknown",
        latency: 0,
        lastChecked: Date.now(),
      },
    );
  }

  return result;
}

/**
 * 强制刷新健康状态缓存
 */
export async function refreshHealthCache(): Promise<
  Map<string, ProviderHealth>
> {
  lastFetchTime = 0; // 清除缓存时间
  return fetchAllHealthStatus();
}

/**
 * 获取缓存中的健康状态（不触发网络请求）
 */
export function getCachedHealth(
  relayPulseProvider: string,
  service: "cc" | "cx" = "cc",
): ProviderHealth | undefined {
  const key = `${relayPulseProvider.toLowerCase()}/${service}`;
  return healthCache.get(key);
}

/**
 * 根据 AppId 获取对应的服务类型
 */
export function appIdToService(appId: AppId): "cc" | "cx" {
  switch (appId) {
    case "claude":
      return "cc";
    case "codex":
      return "cx";
    case "gemini":
      return "cc"; // Gemini 暂时映射到 cc（relay-pulse 未覆盖）
    default:
      return "cc";
  }
}

export const healthCheckApi = {
  fetchAll: fetchAllHealthStatus,
  check: checkProviderHealth,
  checkMultiple: checkProvidersHealth,
  refresh: refreshHealthCache,
  getCached: getCachedHealth,
  appIdToService,
};
