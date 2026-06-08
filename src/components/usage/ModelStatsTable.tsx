import { useModelStats } from "@/lib/query/usage";
import type {
  AppTypeFilter,
  UsageRangeSelection,
  UsageStatsFilters,
} from "@/types/usage";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatNumber, formatUsd } from "./format";

interface ModelStatsTableProps {
  range: UsageRangeSelection;
  appType: AppTypeFilter;
  filters?: UsageStatsFilters;
  refreshIntervalMs: number;
}

export function ModelStatsTable({
  range,
  appType,
  filters,
  refreshIntervalMs,
}: ModelStatsTableProps) {
  const query = useModelStats(range, appType, filters, refreshIntervalMs);

  return (
    <div className="rounded-lg border border-border-default bg-card">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Model</TableHead>
            <TableHead className="text-right">Requests</TableHead>
            <TableHead className="text-right">Tokens</TableHead>
            <TableHead className="text-right">Cost</TableHead>
            <TableHead className="text-right">Avg / request</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {(query.data ?? []).map((row) => (
            <TableRow key={row.model}>
              <TableCell className="max-w-[320px] truncate font-medium">
                {row.model}
              </TableCell>
              <TableCell className="text-right">
                {formatNumber(row.requestCount)}
              </TableCell>
              <TableCell className="text-right">
                {formatNumber(row.totalTokens)}
              </TableCell>
              <TableCell className="text-right">
                {formatUsd(row.totalCost)}
              </TableCell>
              <TableCell className="text-right">
                {formatUsd(row.avgCostPerRequest)}
              </TableCell>
            </TableRow>
          ))}
          {!query.isLoading && (query.data ?? []).length === 0 ? (
            <TableRow>
              <TableCell
                colSpan={5}
                className="text-center text-muted-foreground"
              >
                No model usage yet
              </TableCell>
            </TableRow>
          ) : null}
        </TableBody>
      </Table>
    </div>
  );
}
