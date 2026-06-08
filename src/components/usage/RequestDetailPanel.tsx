import { usageAppLabel, type RequestLog } from "@/types/usage";
import { formatDateTime, formatNumber, formatUsd, statusTone } from "./format";

interface RequestDetailPanelProps {
  log: RequestLog | null;
}

export function RequestDetailPanel({ log }: RequestDetailPanelProps) {
  if (!log) {
    return (
      <div className="rounded-lg border border-border-default bg-card p-4 text-sm text-muted-foreground">
        Select a request to inspect its cost and token breakdown.
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-border-default bg-card p-4">
      <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="text-xs text-muted-foreground">Request</div>
          <div className="max-w-[420px] truncate font-mono text-sm">
            {log.requestId}
          </div>
        </div>
        <span
          className={`rounded-md border px-2 py-1 text-xs ${statusTone(log.statusCode)}`}
        >
          {log.statusCode}
        </span>
      </div>

      <div className="grid gap-3 text-sm md:grid-cols-3">
        <Item label="Provider" value={log.providerName || log.providerId} />
        <Item label="App" value={usageAppLabel(log.appType)} />
        <Item label="Model" value={log.model} />
        <Item label="Request model" value={log.requestModel || "-"} />
        <Item label="Streaming" value={log.isStreaming ? "Yes" : "No"} />
        <Item label="Created" value={formatDateTime(log.createdAt)} />
        <Item label="Latency" value={`${log.latencyMs}ms`} />
        <Item
          label="First token"
          value={log.firstTokenMs ? `${log.firstTokenMs}ms` : "-"}
        />
        <Item
          label="Duration"
          value={log.durationMs ? `${log.durationMs}ms` : "-"}
        />
      </div>

      <div className="mt-4 grid gap-3 text-sm md:grid-cols-4">
        <Item label="Input" value={formatNumber(log.inputTokens)} />
        <Item label="Output" value={formatNumber(log.outputTokens)} />
        <Item label="Cache read" value={formatNumber(log.cacheReadTokens)} />
        <Item
          label="Cache create"
          value={formatNumber(log.cacheCreationTokens)}
        />
      </div>

      <div className="mt-4 grid gap-3 text-sm md:grid-cols-5">
        <Item label="Input cost" value={formatUsd(log.inputCostUsd)} />
        <Item label="Output cost" value={formatUsd(log.outputCostUsd)} />
        <Item label="Cache read cost" value={formatUsd(log.cacheReadCostUsd)} />
        <Item
          label="Cache create cost"
          value={formatUsd(log.cacheCreationCostUsd)}
        />
        <Item label="Total cost" value={formatUsd(log.totalCostUsd)} />
      </div>

      {log.errorMessage ? (
        <div className="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/40 dark:text-red-300">
          {log.errorMessage}
        </div>
      ) : null}
    </div>
  );
}

function Item({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md bg-muted/40 px-3 py-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="truncate font-medium">{value}</div>
    </div>
  );
}
