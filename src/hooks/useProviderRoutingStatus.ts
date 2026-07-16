import { useQuery } from "@tanstack/react-query";

import { settingsApi, type AppId } from "@/lib/api";
import type {
  FailoverQueueItem,
  ProxyAppId,
  ProxyRouteAppId,
  ProxySettings,
  ProxyStatus,
} from "@/types";

const PROXY_ROUTE_APPS = new Set<AppId>([
  "claude",
  "claude-desktop",
  "codex",
  "gemini",
  "opencode",
]);

export interface ProviderRoutingSnapshot {
  routeApp: ProxyRouteAppId;
  configApp: ProxyAppId;
  status: ProxyStatus;
  settings: ProxySettings;
  queue: FailoverQueueItem[];
}

const resolveProxyApps = (
  appId: AppId,
): { routeApp: ProxyRouteAppId; configApp: ProxyAppId } | null => {
  if (!PROXY_ROUTE_APPS.has(appId)) return null;
  const routeApp = appId as ProxyRouteAppId;
  return {
    routeApp,
    configApp: routeApp === "claude-desktop" ? "claude" : routeApp,
  };
};

export function useProviderRoutingStatus(appId: AppId) {
  const apps = resolveProxyApps(appId);
  return useQuery<ProviderRoutingSnapshot>({
    queryKey: ["provider-routing-status", apps?.routeApp],
    enabled: apps !== null,
    queryFn: async () => {
      if (!apps) throw new Error("Provider routing is unavailable for this app");
      const [status, settings, queue] = await Promise.all([
        settingsApi.getProxyStatus(),
        settingsApi.getProxyConfig(),
        settingsApi.getFailoverQueue(apps.routeApp),
      ]);
      return { ...apps, status, settings, queue };
    },
    staleTime: 2_000,
    refetchInterval: 5_000,
    retry: false,
  });
}
