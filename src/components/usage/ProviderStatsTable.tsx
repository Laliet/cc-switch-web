import { useProviderStats } from "@/lib/query/usage";
import type { AppTypeFilter, UsageRangeSelection } from "@/types/usage";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatNumber, formatPercent, formatUsd } from "./format";

interface ProviderStatsTableProps {
  range: UsageRangeSelection;
  appType: AppTypeFilter;
  refreshIntervalMs: number;
}

export function ProviderStatsTable({
  range,
  appType,
  refreshIntervalMs,
}: ProviderStatsTableProps) {
  const query = useProviderStats(range, appType, refreshIntervalMs);

  return (
    <div className="rounded-lg border border-border-default bg-card">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Provider</TableHead>
            <TableHead>App</TableHead>
            <TableHead className="text-right">Requests</TableHead>
            <TableHead className="text-right">Tokens</TableHead>
            <TableHead className="text-right">Cost</TableHead>
            <TableHead className="text-right">Success</TableHead>
            <TableHead className="text-right">Avg latency</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {(query.data ?? []).map((row) => (
            <TableRow key={`${row.appType}:${row.providerId}`}>
              <TableCell className="font-medium">{row.providerName}</TableCell>
              <TableCell>{row.appType}</TableCell>
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
                {formatPercent(row.successRate)}
              </TableCell>
              <TableCell className="text-right">{row.avgLatencyMs}ms</TableCell>
            </TableRow>
          ))}
          {!query.isLoading && (query.data ?? []).length === 0 ? (
            <TableRow>
              <TableCell colSpan={7} className="text-center text-muted-foreground">
                No provider usage yet
              </TableCell>
            </TableRow>
          ) : null}
        </TableBody>
      </Table>
    </div>
  );
}
