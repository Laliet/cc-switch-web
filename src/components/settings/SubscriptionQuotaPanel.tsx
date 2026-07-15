import { useCallback, useEffect, useState } from "react";
import { Gauge, Loader2, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  subscriptionApi,
  type SubscriptionProvider,
  type SubscriptionQuota,
} from "@/lib/api/subscription";

const PROVIDERS: Array<{ id: SubscriptionProvider; label: string }> = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex / ChatGPT" },
  { id: "gemini", label: "Gemini" },
];

export function SubscriptionQuotaPanel() {
  const { t } = useTranslation();
  const [quotas, setQuotas] = useState<
    Partial<Record<SubscriptionProvider, SubscriptionQuota>>
  >({});
  const [loading, setLoading] = useState(false);

  const load = useCallback(async (force = false) => {
    setLoading(true);
    const results = await Promise.all(
      PROVIDERS.map(async ({ id }) => {
        try {
          return [id, await subscriptionApi.query(id, { force })] as const;
        } catch (error) {
          return [
            id,
            {
              provider: id,
              source: "server_credentials",
              status: "unavailable",
              windows: [],
              fetchedAt: Date.now(),
              error: error instanceof Error ? error.message : String(error),
            } satisfies SubscriptionQuota,
          ] as const;
        }
      }),
    );
    setQuotas(
      Object.fromEntries(results) as Partial<
        Record<SubscriptionProvider, SubscriptionQuota>
      >,
    );
    setLoading(false);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <section className="space-y-4 rounded-md border border-border-default p-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h4 className="flex items-center gap-2 text-sm font-medium">
            <Gauge className="h-4 w-4" />
            {t("subscription.title", { defaultValue: "官方订阅额度" })}
          </h4>
          <p className="text-xs text-muted-foreground">
            {t("subscription.subtitle", {
              defaultValue: "仅显示服务器凭据返回的脱敏额度状态。",
            })}
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          size="icon"
          onClick={() => void load(true)}
          disabled={loading}
          title={t("common.refresh", { defaultValue: "刷新" })}
        >
          {loading ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <RefreshCw className="h-4 w-4" />
          )}
        </Button>
      </div>
      <div className="grid gap-3 md:grid-cols-3">
        {PROVIDERS.map(({ id, label }) => {
          const quota = quotas[id];
          const status = quota?.status ?? "loading";
          return (
            <div
              key={id}
              className="space-y-2 rounded-md border border-border-default p-3"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="text-sm font-medium">{label}</span>
                <Badge
                  variant={status === "available" ? "secondary" : "outline"}
                >
                  {status}
                </Badge>
              </div>
              {quota?.plan ? (
                <div className="text-xs text-muted-foreground">
                  {quota.plan}
                </div>
              ) : null}
              {quota?.windows.length ? (
                <div className="space-y-2">
                  {quota.windows.slice(0, 3).map((window) => {
                    const total =
                      typeof window.total === "number" && window.total > 0
                        ? window.total
                        : undefined;
                    const remaining =
                      typeof window.remaining === "number"
                        ? window.remaining
                        : undefined;
                    const percent =
                      total && remaining !== undefined
                        ? Math.max(0, Math.min(100, (remaining / total) * 100))
                        : undefined;
                    return (
                      <div key={window.name} className="space-y-1">
                        <div className="flex justify-between text-xs text-muted-foreground">
                          <span>{window.name}</span>
                          <span>
                            {remaining !== undefined
                              ? remaining.toLocaleString()
                              : "-"}
                          </span>
                        </div>
                        <div className="h-1.5 overflow-hidden rounded-full bg-muted">
                          <div
                            className="h-full bg-primary"
                            style={{ width: `${percent ?? 0}%` }}
                          />
                        </div>
                      </div>
                    );
                  })}
                </div>
              ) : (
                <div className="text-xs text-muted-foreground">
                  {quota?.error ??
                    t("subscription.noQuota", {
                      defaultValue: "暂无可用额度窗口",
                    })}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
