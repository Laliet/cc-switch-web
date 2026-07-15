import { useQuery } from "@tanstack/react-query";
import {
  getLatestStreamCheckLogs,
  getStreamCheckLogs,
  type StreamCheckLog,
  type StreamCheckLogQuery,
} from "@/lib/api/model-test";
import type { AppId } from "@/lib/api";

export function useStreamCheckHistory(
  appId: AppId,
  query: Omit<StreamCheckLogQuery, "appType"> = {},
) {
  const supported = !["omo", "omo-slim", "openclaw"].includes(appId);
  return useQuery<StreamCheckLog[]>({
    queryKey: ["stream-check-logs", appId, query],
    queryFn: () => getStreamCheckLogs({ ...query, appType: appId }),
    enabled: supported,
    staleTime: 15_000,
  });
}

export function useLatestStreamCheckHistory(appId: AppId) {
  const supported = !["omo", "omo-slim", "openclaw"].includes(appId);
  return useQuery<StreamCheckLog[]>({
    queryKey: ["stream-check-logs", appId, "latest"],
    queryFn: () => getLatestStreamCheckLogs(appId),
    enabled: supported,
    staleTime: 15_000,
  });
}
