import { useQuery } from "@tanstack/react-query";
import { Gauge } from "lucide-react";
import type { AppId } from "@/lib/api";
import {
  subscriptionApi,
  type QuotaWindow,
  type SubscriptionProvider,
} from "@/lib/api/subscription";

interface SubscriptionQuotaSummaryProps {
  appId: AppId;
}

const SUBSCRIPTION_APPS = new Set<AppId>(["claude", "codex", "gemini"]);

function remainingPercent(window: QuotaWindow): number | undefined {
  if (
    typeof window.remaining === "number" &&
    typeof window.total === "number" &&
    window.total > 0
  ) {
    return Math.max(0, Math.min(100, (window.remaining / window.total) * 100));
  }
  if (typeof window.used === "number") {
    return Math.max(0, Math.min(100, 100 - window.used));
  }
  return undefined;
}

export function SubscriptionQuotaSummary({
  appId,
}: SubscriptionQuotaSummaryProps) {
  const enabled = SUBSCRIPTION_APPS.has(appId);
  const query = useQuery({
    queryKey: ["subscription-quota", appId],
    queryFn: () => subscriptionApi.query(appId as SubscriptionProvider),
    enabled,
    staleTime: 60_000,
    retry: false,
  });
  const quota = query.data;
  const candidates =
    quota?.windows
      .map((window) => ({ window, percent: remainingPercent(window) }))
      .filter(
        (item): item is { window: QuotaWindow; percent: number } =>
          item.percent !== undefined,
      ) ?? [];
  candidates.sort((left, right) => left.percent - right.percent);
  const mostRestricted = candidates[0];

  if (
    !enabled ||
    !quota ||
    !quota.status.startsWith("available") ||
    !mostRestricted
  ) {
    return null;
  }

  const percent = Math.round(mostRestricted.percent);
  const tooltip = [
    quota.plan,
    `${mostRestricted.window.name}: ${percent}% remaining`,
    mostRestricted.window.resetAt
      ? `resets ${new Date(mostRestricted.window.resetAt).toLocaleString()}`
      : undefined,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <span
      className="inline-flex items-center gap-1 text-xs text-muted-foreground"
      title={tooltip}
      aria-label={tooltip}
    >
      <Gauge className="h-3.5 w-3.5" aria-hidden="true" />
      <span>{percent}%</span>
    </span>
  );
}
