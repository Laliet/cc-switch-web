import { useCallback, useMemo } from "react";
import {
  keepPreviousData,
  useQuery,
  type QueryObserverResult,
} from "@tanstack/react-query";
import { getRelayPulseProviderFromProvider } from "@/config/healthCheckMapping";
import { healthCheckApi, type AppId, type ProviderHealth } from "@/lib/api";
import { appIdToService } from "@/lib/api/healthCheck";
import type { Provider } from "@/types";

export interface UseHealthCheckOptions {
  enabled?: boolean;
  refetchInterval?: number;
}

type ProviderCollection = Provider[] | Record<string, Provider> | undefined;

interface MonitoredProvider {
  providerId: string;
  relayPulseProvider: string;
}

export const DEFAULT_HEALTH_REFETCH_INTERVAL = 60_000;

export function useHealthCheck(
  appId: AppId,
  providers: ProviderCollection,
  options?: UseHealthCheckOptions,
) {
  const { enabled = true, refetchInterval = DEFAULT_HEALTH_REFETCH_INTERVAL } =
    options || {};

  const service = useMemo(() => appIdToService(appId), [appId]);

  const providerList = useMemo<Provider[]>(
    () =>
      !providers
        ? []
        : Array.isArray(providers)
          ? providers
          : Object.values(providers),
    [providers],
  );

  const monitoredProviders = useMemo<MonitoredProvider[]>(() => {
    return providerList
      .map((provider) => {
        const relayPulseProvider = getRelayPulseProviderFromProvider(provider);
        if (!relayPulseProvider) return undefined;
        return {
          providerId: provider.id,
          relayPulseProvider: relayPulseProvider.toLowerCase(),
        };
      })
      .filter((item): item is MonitoredProvider => Boolean(item));
  }, [providerList]);

  const providersKey = useMemo(
    () =>
      monitoredProviders
        .map((item) => `${item.providerId}:${item.relayPulseProvider}`)
        .sort()
        .join("|"),
    [monitoredProviders],
  );

  const resolvedRefetchInterval =
    enabled && monitoredProviders.length > 0 && refetchInterval > 0
      ? refetchInterval
      : false;

  const query = useQuery<Record<string, ProviderHealth>>({
    queryKey: ["health-check", appId, providersKey],
    queryFn: async () => {
      if (monitoredProviders.length === 0) return {};

      const healthMap = await healthCheckApi.checkMultiple(
        monitoredProviders.map((item) => ({
          relayPulseProvider: item.relayPulseProvider,
          service,
        })),
      );

      const result: Record<string, ProviderHealth> = {};
      for (const { providerId, relayPulseProvider } of monitoredProviders) {
        const key = `${relayPulseProvider}/${service}`;
        result[providerId] = healthMap.get(key) || {
          isHealthy: false,
          status: "unknown",
          latency: 0,
          lastChecked: Date.now(),
        };
      }

      return result;
    },
    enabled: enabled && monitoredProviders.length > 0,
    refetchInterval: resolvedRefetchInterval,
    refetchIntervalInBackground: true,
    refetchOnWindowFocus: false,
    staleTime: 0,
    placeholderData: keepPreviousData,
  });

  const refetch = useCallback(async (): Promise<
    QueryObserverResult<Record<string, ProviderHealth>, Error>
  > => {
    await healthCheckApi.refresh();
    return query.refetch();
  }, [query]);

  return {
    healthMap: query.data ?? {},
    isLoading: query.isLoading,
    refetch,
    lastUpdated: query.dataUpdatedAt || null,
  };
}
