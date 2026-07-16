import { useEffect, useMemo, useState } from "react";
import { RefreshCw, Save } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  useApplyOpenClawReconciliation,
  useOpenClawReconciliation,
} from "@/hooks/useOpenClaw";
import type {
  OpenClawReconciliationItem,
  OpenClawReconciliationStatus,
} from "@/lib/api";
import { cn } from "@/lib/utils";
import { extractErrorMessage } from "@/utils/errorUtils";

interface OpenClawReconciliationPanelProps {
  enabled: boolean;
}

export function OpenClawReconciliationPanel({
  enabled,
}: OpenClawReconciliationPanelProps) {
  const { t } = useTranslation();
  const query = useOpenClawReconciliation(enabled);
  const mutation = useApplyOpenClawReconciliation();
  const [selected, setSelected] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!query.data) return;
    setSelected(
      new Set(
        query.data.items
          .filter(
            (item) =>
              item.status === "new" ||
              (item.status === "changed" && item.liveConfigManaged),
          )
          .map((item) => item.providerId),
      ),
    );
  }, [query.data]);

  const selectable = useMemo(
    () => query.data?.items.filter((item) => item.status !== "invalid") ?? [],
    [query.data],
  );
  const allSelected =
    selectable.length > 0 &&
    selectable.every((item) => selected.has(item.providerId));

  const apply = async () => {
    if (!query.data || selected.size === 0) return;
    try {
      const result = await mutation.mutateAsync({
        providerIds: [...selected],
        expectedEtag: query.data.etag,
      });
      toast.success(t("openclaw.reconcile.completed"), {
        description: t("openclaw.reconcile.result", {
          imported: result.imported,
          updated: result.updated,
          unchanged: result.unchanged,
        }),
      });
    } catch (error) {
      toast.error(t("openclaw.reconcile.failed"), {
        description: extractErrorMessage(error),
      });
    }
  };

  if (query.isLoading) {
    return (
      <div className="grid min-h-64 place-items-center text-sm text-muted-foreground">
        {t("common.loading")}
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b px-5 py-3">
        <div className="flex items-center gap-4 text-sm">
          <span>
            {t("openclaw.reconcile.liveCount", {
              count: query.data?.liveCount ?? 0,
            })}
          </span>
          <span className="text-muted-foreground">
            {t("openclaw.reconcile.storedCount", {
              count: query.data?.storedCount ?? 0,
            })}
          </span>
        </div>
        <Button
          type="button"
          size="icon"
          variant="outline"
          title={t("common.refresh")}
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        >
          <RefreshCw
            className={cn("h-4 w-4", query.isFetching && "animate-spin")}
          />
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="grid min-h-11 grid-cols-[36px_minmax(0,1fr)_100px] items-center border-b bg-muted/40 px-4 text-xs font-medium text-muted-foreground sm:grid-cols-[36px_minmax(0,1fr)_90px_90px_100px]">
          <Checkbox
            checked={allSelected}
            aria-label={t("common.selectAll")}
            onCheckedChange={(checked) =>
              setSelected(
                checked
                  ? new Set(selectable.map((item) => item.providerId))
                  : new Set(),
              )
            }
          />
          <span>{t("openclaw.reconcile.provider")}</span>
          <span className="hidden sm:block">
            {t("openclaw.reconcile.models")}
          </span>
          <span className="hidden sm:block">API Key</span>
          <span>{t("openclaw.reconcile.status")}</span>
        </div>
        {query.data?.items.map((item) => (
          <ReconciliationRow
            key={item.providerId}
            item={item}
            checked={selected.has(item.providerId)}
            onCheckedChange={(checked) =>
              setSelected((current) => {
                const next = new Set(current);
                if (checked) next.add(item.providerId);
                else next.delete(item.providerId);
                return next;
              })
            }
          />
        ))}
        {query.data?.items.length === 0 ? (
          <div className="grid min-h-48 place-items-center text-sm text-muted-foreground">
            {t("openclaw.reconcile.empty")}
          </div>
        ) : null}
      </div>

      <div className="flex items-center justify-between gap-3 border-t px-5 py-3">
        <span className="text-xs text-muted-foreground">
          {t("openclaw.reconcile.selected", { count: selected.size })}
        </span>
        <Button
          onClick={() => void apply()}
          disabled={mutation.isPending || selected.size === 0}
        >
          <Save className="h-4 w-4" />
          {mutation.isPending
            ? t("common.saving")
            : t("openclaw.reconcile.apply")}
        </Button>
      </div>
    </div>
  );
}

function ReconciliationRow({
  item,
  checked,
  onCheckedChange,
}: {
  item: OpenClawReconciliationItem;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="grid min-h-16 grid-cols-[36px_minmax(0,1fr)_100px] items-center border-b px-4 py-2 text-sm sm:grid-cols-[36px_minmax(0,1fr)_90px_90px_100px]">
      <Checkbox
        checked={checked}
        disabled={item.status === "invalid"}
        aria-label={item.providerId}
        onCheckedChange={(value) => onCheckedChange(Boolean(value))}
      />
      <div className="min-w-0">
        <div className="truncate font-medium" title={item.displayName}>
          {item.displayName}
        </div>
        <div
          className="truncate font-mono text-xs text-muted-foreground"
          title={item.providerId}
        >
          {item.providerId}
        </div>
        {item.reason ? (
          <div
            className="truncate text-xs text-destructive"
            title={item.reason}
          >
            {item.reason}
          </div>
        ) : null}
      </div>
      <span className="hidden text-xs sm:block">{item.modelCount}</span>
      <span className="hidden text-xs sm:block">
        {item.hasApiKey ? t("common.yes") : t("common.no")}
      </span>
      <StatusBadge status={item.status} />
    </div>
  );
}

function StatusBadge({ status }: { status: OpenClawReconciliationStatus }) {
  const { t } = useTranslation();
  const styles: Record<OpenClawReconciliationStatus, string> = {
    new: "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
    changed:
      "border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300",
    unchanged: "border-border bg-muted text-muted-foreground",
    invalid: "border-destructive/30 bg-destructive/10 text-destructive",
  };
  return (
    <Badge variant="outline" className={cn("w-fit", styles[status])}>
      {t(`openclaw.reconcile.statuses.${status}`)}
    </Badge>
  );
}
