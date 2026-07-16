import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";

interface FailoverPriorityBadgeProps {
  priority: number;
  active: boolean;
}

export function FailoverPriorityBadge({
  priority,
  active,
}: FailoverPriorityBadgeProps) {
  const { t } = useTranslation();
  return (
    <span
      className={cn(
        "inline-flex items-center rounded border px-1.5 py-0.5 text-xs font-semibold",
        active
          ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
          : "border-border-default bg-muted/40 text-muted-foreground",
      )}
      title={t("provider.failoverPriorityTooltip", {
        priority,
        state: active
          ? t("provider.failoverActive", { defaultValue: "自动切换已启用" })
          : t("provider.failoverInactive", { defaultValue: "自动切换未运行" }),
        defaultValue: `故障转移优先级 ${priority} · {{state}}`,
      })}
    >
      P{priority}
    </span>
  );
}
