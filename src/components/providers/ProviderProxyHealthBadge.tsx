import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import type { ProxyProviderHealth } from "@/types";

interface ProviderProxyHealthBadgeProps {
  health: ProxyProviderHealth;
}

export function ProviderProxyHealthBadge({
  health,
}: ProviderProxyHealthBadgeProps) {
  const { t } = useTranslation();
  const display = useMemo(() => {
    switch (health.state) {
      case "open":
        return {
          label: t("provider.circuit.open", { defaultValue: "熔断" }),
          classes:
            "border-red-500/30 bg-red-500/10 text-red-700 dark:text-red-300",
          dot: "bg-red-500",
        };
      case "half_open":
        return {
          label: t("provider.circuit.halfOpen", { defaultValue: "恢复探测" }),
          classes:
            "border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300",
          dot: "bg-amber-500",
        };
      case "healthy":
        return {
          label: t("provider.circuit.healthy", { defaultValue: "路由健康" }),
          classes:
            "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
          dot: "bg-emerald-500",
        };
      default:
        return {
          label: health.state,
          classes: "border-border-default bg-muted/40 text-muted-foreground",
          dot: "bg-muted-foreground",
        };
    }
  }, [health.state, t]);
  const details = [
    t("provider.circuit.failureCount", {
      count: health.failureCount,
      defaultValue: `连续失败 ${health.failureCount} 次`,
    }),
    t("provider.circuit.window", {
      failed: health.windowFailures,
      total: health.windowRequests,
      defaultValue: `窗口失败 ${health.windowFailures}/${health.windowRequests}`,
    }),
    typeof health.lastFailureSecondsAgo === "number"
      ? t("provider.circuit.lastFailure", {
          seconds: health.lastFailureSecondsAgo,
          defaultValue: `最近失败 ${health.lastFailureSecondsAgo} 秒前`,
        })
      : undefined,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded border px-1.5 py-0.5 text-xs font-medium",
        display.classes,
      )}
      title={`${display.label} · ${details}`}
      aria-label={`${display.label} · ${details}`}
    >
      <span className={cn("h-1.5 w-1.5 rounded-full", display.dot)} />
      <span>{display.label}</span>
      {health.failureCount > 0 ? <span>F{health.failureCount}</span> : null}
    </span>
  );
}
