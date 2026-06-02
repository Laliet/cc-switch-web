import { Database, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { usageApi } from "@/lib/api/usage";
import { useDataSources, usageKeys } from "@/lib/query/usage";
import { useQueryClient } from "@tanstack/react-query";
import { formatUsd } from "./format";

interface DataSourceBarProps {
  refreshIntervalMs: number;
}

export function DataSourceBar({ refreshIntervalMs }: DataSourceBarProps) {
  const query = useDataSources(refreshIntervalMs);
  const queryClient = useQueryClient();

  const sync = async () => {
    try {
      const result = await usageApi.syncSessionUsage();
      await queryClient.invalidateQueries({ queryKey: usageKeys.all });
      if (result.errors.length > 0) {
        toast.info(result.errors[0]);
      } else {
        toast.success(`Imported ${result.imported} session logs`);
      }
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Session sync failed",
      );
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-2 rounded-lg border border-border-default bg-card px-4 py-3 text-sm">
      <Database className="h-4 w-4 text-muted-foreground" />
      <span className="font-medium">All-time data sources</span>
      {(
        query.data ?? [
          { dataSource: "proxy", requestCount: 0, totalCostUsd: "0" },
        ]
      ).map((item) => (
        <span
          key={item.dataSource}
          className="rounded-md bg-muted/50 px-2 py-1 text-xs"
        >
          {item.dataSource}: {item.requestCount} /{" "}
          {formatUsd(item.totalCostUsd)}
        </span>
      ))}
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="ml-auto"
        onClick={() => void sync()}
      >
        <RefreshCw className="h-4 w-4" />
        Sync sessions
      </Button>
    </div>
  );
}
