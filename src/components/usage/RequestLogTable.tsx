import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, Database, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useRequestLogs } from "@/lib/query/usage";
import type {
  AppTypeFilter,
  LogFilters,
  RequestLog,
  UsageRangeSelection,
  UsageStatsFilters,
} from "@/types/usage";
import { formatDateTime, formatNumber, formatUsd, statusTone } from "./format";
import { RequestDetailPanel } from "./RequestDetailPanel";

interface RequestLogTableProps {
  range: UsageRangeSelection;
  appType: AppTypeFilter;
  filters?: UsageStatsFilters;
  refreshIntervalMs: number;
}

export function RequestLogTable({
  range,
  appType,
  filters: dashboardFilters,
  refreshIntervalMs,
}: RequestLogTableProps) {
  const [page, setPage] = useState(0);
  const [model, setModel] = useState("");
  const [providerName, setProviderName] = useState("");
  const [status, setStatus] = useState("all");
  const [selected, setSelected] = useState<RequestLog | null>(null);
  const [pageInput, setPageInput] = useState("1");
  const pageSize = 20;
  useEffect(() => {
    setPage(0);
  }, [appType, dashboardFilters?.model, dashboardFilters?.providerId, range]);

  useEffect(() => {
    setPageInput(String(page + 1));
  }, [page]);

  const filters = useMemo<LogFilters>(
    () => ({
      appType: appType === "all" ? undefined : appType,
      providerId: dashboardFilters?.providerId,
      providerName: providerName.trim() || undefined,
      model: dashboardFilters?.model ?? (model.trim() || undefined),
      statusCode: status === "all" ? undefined : Number(status),
    }),
    [
      appType,
      dashboardFilters?.model,
      dashboardFilters?.providerId,
      model,
      providerName,
      status,
    ],
  );
  const query = useRequestLogs(
    range,
    filters,
    page,
    pageSize,
    refreshIntervalMs,
  );
  const total = query.data?.total ?? 0;
  const pages = Math.max(1, Math.ceil(total / pageSize));
  const jumpToPage = () => {
    const parsed = Number.parseInt(pageInput, 10);
    if (!Number.isFinite(parsed)) {
      setPageInput(String(page + 1));
      return;
    }
    const nextPage = Math.min(Math.max(parsed, 1), pages) - 1;
    setPage(nextPage);
  };

  const sourceLabel = (value?: string | null) => {
    const source = value?.trim() || "proxy";
    if (source === "session") return "Session";
    if (source === "proxy") return "Proxy";
    if (source === "rollup") return "Rollup";
    return source;
  };

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-border-default bg-card p-4">
        <div className="mb-3 flex flex-wrap items-center gap-2">
          <div className="relative min-w-[180px] flex-1">
            <Search className="pointer-events-none absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              value={providerName}
              onChange={(event) => {
                setProviderName(event.target.value);
                setPage(0);
              }}
              className="pl-8"
              placeholder="Provider"
            />
          </div>
          <Input
            value={model}
            onChange={(event) => {
              setModel(event.target.value);
              setPage(0);
            }}
            className="min-w-[180px] flex-1"
            placeholder="Model"
            disabled={Boolean(dashboardFilters?.model)}
          />
          <Select
            value={status}
            onValueChange={(value) => {
              setStatus(value);
              setPage(0);
            }}
          >
            <SelectTrigger className="w-[150px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All status</SelectItem>
              <SelectItem value="200">200</SelectItem>
              <SelectItem value="400">400</SelectItem>
              <SelectItem value="401">401</SelectItem>
              <SelectItem value="429">429</SelectItem>
              <SelectItem value="500">500</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Time</TableHead>
              <TableHead>Provider</TableHead>
              <TableHead>Source</TableHead>
              <TableHead>Model</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Tokens</TableHead>
              <TableHead className="text-right">Cost</TableHead>
              <TableHead className="text-right">Latency</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {(query.data?.data ?? []).map((log) => (
              <TableRow
                key={log.requestId}
                className="cursor-pointer"
                onClick={() => setSelected(log)}
              >
                <TableCell className="whitespace-nowrap">
                  {formatDateTime(log.createdAt)}
                </TableCell>
                <TableCell className="max-w-[180px] truncate">
                  {log.providerName || log.providerId}
                </TableCell>
                <TableCell>
                  <span
                    className="inline-flex items-center gap-1 rounded-md border px-2 py-1 text-xs text-muted-foreground"
                    title={`Data source: ${sourceLabel(log.dataSource)}`}
                  >
                    <Database className="h-3.5 w-3.5" />
                    {sourceLabel(log.dataSource)}
                  </span>
                </TableCell>
                <TableCell className="max-w-[260px] truncate">
                  <div className="flex items-center gap-1">
                    {log.isUnpriced ? (
                      <AlertTriangle className="h-3.5 w-3.5 text-amber-500" />
                    ) : null}
                    <span className="truncate">{log.model}</span>
                  </div>
                </TableCell>
                <TableCell>
                  <span
                    className={`rounded-md border px-2 py-1 text-xs ${statusTone(log.statusCode)}`}
                  >
                    {log.statusCode}
                  </span>
                </TableCell>
                <TableCell className="text-right">
                  {formatNumber(
                    log.inputTokens +
                      log.outputTokens +
                      log.cacheReadTokens +
                      log.cacheCreationTokens,
                  )}
                </TableCell>
                <TableCell className="text-right">
                  {formatUsd(log.totalCostUsd)}
                </TableCell>
                <TableCell className="text-right">{log.latencyMs}ms</TableCell>
              </TableRow>
            ))}
            {!query.isLoading && (query.data?.data ?? []).length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={8}
                  className="text-center text-muted-foreground"
                >
                  No request logs in this range
                </TableCell>
              </TableRow>
            ) : null}
          </TableBody>
        </Table>
        <div className="mt-3 flex items-center justify-between text-sm text-muted-foreground">
          <span>
            {total} logs, page {page + 1} / {pages}
          </span>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={page <= 0}
              onClick={() => setPage((value) => Math.max(0, value - 1))}
            >
              Prev
            </Button>
            <Input
              value={pageInput}
              onChange={(event) => setPageInput(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  jumpToPage();
                }
              }}
              className="h-8 w-16 text-center"
              aria-label="Jump to page"
            />
            <Button variant="outline" size="sm" onClick={jumpToPage}>
              Go
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={page + 1 >= pages}
              onClick={() => setPage((value) => value + 1)}
            >
              Next
            </Button>
          </div>
        </div>
      </div>
      <RequestDetailPanel log={selected} />
    </div>
  );
}
