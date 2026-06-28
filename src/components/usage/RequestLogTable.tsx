import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
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
import { getFreshInputTokens, isUnpricedUsage } from "@/types/usage";
import type {
  AppTypeFilter,
  LogFilters,
  RequestLog,
  UsageRangeSelection,
  UsageStatsFilters,
} from "@/types/usage";
import {
  formatDateTime,
  formatNumber,
  formatUsd,
  parseFiniteNumber,
  statusTone,
} from "./format";
import { RequestDetailPanel } from "./RequestDetailPanel";
import { UsageDateRangePicker } from "./UsageDateRangePicker";

interface RequestLogTableProps {
  range: UsageRangeSelection;
  rangeLabel?: string;
  appType: AppTypeFilter;
  filters?: UsageStatsFilters;
  refreshIntervalMs: number;
  onRangeChange?: (range: UsageRangeSelection) => void;
}

export function RequestLogTable({
  range,
  rangeLabel,
  appType,
  filters: dashboardFilters,
  refreshIntervalMs,
  onRangeChange,
}: RequestLogTableProps) {
  const { t } = useTranslation();
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
          {onRangeChange && rangeLabel ? (
            <UsageDateRangePicker
              selection={range}
              triggerLabel={rangeLabel}
              onApply={(nextRange) => {
                onRangeChange(nextRange);
                setPage(0);
              }}
            />
          ) : null}
        </div>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("usage.time", { defaultValue: "Time" })}</TableHead>
              <TableHead>
                {t("usage.provider", { defaultValue: "Provider" })}
              </TableHead>
              <TableHead>
                {t("usage.source", { defaultValue: "Source" })}
              </TableHead>
              <TableHead>
                {t("usage.billingModel", { defaultValue: "Billing model" })}
              </TableHead>
              <TableHead>
                {t("usage.status", { defaultValue: "Status" })}
              </TableHead>
              <TableHead className="text-right">
                {t("usage.inputTokens", { defaultValue: "Input" })}
              </TableHead>
              <TableHead className="text-right">
                {t("usage.outputTokens", { defaultValue: "Output" })}
              </TableHead>
              <TableHead className="text-right">
                {t("usage.cost", { defaultValue: "Cost" })}
              </TableHead>
              <TableHead className="text-right">
                {t("usage.timingInfo", { defaultValue: "Timing" })}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {(query.data?.data ?? []).map((log) => {
              const freshInput = getFreshInputTokens(log);
              const cacheInclusive = freshInput !== log.inputTokens;
              const unpriced = log.isUnpriced || isUnpricedUsage(log);
              const multiplier = parseFiniteNumber(log.costMultiplier);
              return (
                <TableRow
                  key={log.requestId}
                  className="cursor-pointer"
                  onClick={() => setSelected(log)}
                >
                  <TableCell className="whitespace-nowrap text-xs">
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
                      {unpriced ? (
                        <AlertTriangle className="h-3.5 w-3.5 text-amber-500" />
                      ) : null}
                      <span
                        className="truncate font-mono text-xs"
                        title={
                          log.requestModel && log.requestModel !== log.model
                            ? `${log.requestModel} -> ${log.model}`
                            : log.model
                        }
                      >
                        {log.requestModel && log.requestModel !== log.model ? (
                          <>
                            {log.requestModel}
                            <span className="text-muted-foreground">
                              {" -> "}
                              {log.model}
                            </span>
                          </>
                        ) : (
                          log.model
                        )}
                      </span>
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
                    <div
                      title={
                        cacheInclusive
                          ? `Raw: ${formatNumber(log.inputTokens)}`
                          : undefined
                      }
                    >
                      {formatNumber(freshInput)}
                    </div>
                    {log.cacheReadTokens > 0 || log.cacheCreationTokens > 0 ? (
                      <div className="text-[10px] text-muted-foreground">
                        {[
                          log.cacheReadTokens > 0
                            ? `R${formatNumber(log.cacheReadTokens)}`
                            : null,
                          log.cacheCreationTokens > 0
                            ? `W${formatNumber(log.cacheCreationTokens)}`
                            : null,
                        ]
                          .filter(Boolean)
                          .join(" · ")}
                      </div>
                    ) : null}
                  </TableCell>
                  <TableCell className="text-right">
                    {formatNumber(log.outputTokens)}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className={unpriced ? "text-muted-foreground" : ""}>
                      {unpriced
                        ? t("usage.unpriced", { defaultValue: "Unpriced" })
                        : formatUsd(log.totalCostUsd)}
                    </div>
                    {multiplier !== null && multiplier !== 1 ? (
                      <div className="text-[11px] text-muted-foreground">
                        x{multiplier.toFixed(2)}
                      </div>
                    ) : null}
                  </TableCell>
                  <TableCell className="text-right text-xs tabular-nums">
                    {(log.latencyMs / 1000).toFixed(1)}s
                    {log.firstTokenMs != null ? (
                      <span className="text-muted-foreground">
                        /{(log.firstTokenMs / 1000).toFixed(1)}s
                      </span>
                    ) : null}
                  </TableCell>
                </TableRow>
              );
            })}
            {!query.isLoading && (query.data?.data ?? []).length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={9}
                  className="text-center text-muted-foreground"
                >
                  {t("usage.noRequestLogs", {
                    defaultValue: "No request logs in this range",
                  })}
                </TableCell>
              </TableRow>
            ) : null}
          </TableBody>
        </Table>
        <div className="mt-3 flex items-center justify-between text-sm text-muted-foreground">
          <span>
            {t("usage.logPageSummary", {
              defaultValue: "{{total}} logs, page {{page}} / {{pages}}",
              total,
              page: page + 1,
              pages,
            })}
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
