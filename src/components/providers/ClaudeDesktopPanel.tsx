import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  FileJson,
  GitBranch,
  Import,
  RefreshCw,
  Route,
} from "lucide-react";
import { toast } from "sonner";
import { providersApi, type ClaudeDesktopStatus } from "@/lib/api/providers";
import { Button } from "@/components/ui/button";

interface ClaudeDesktopPanelProps {
  onProvidersChanged?: () => void;
}

export function ClaudeDesktopPanel({
  onProvidersChanged,
}: ClaudeDesktopPanelProps) {
  const [status, setStatus] = useState<ClaudeDesktopStatus | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadStatus = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const next = await providersApi.getClaudeDesktopStatus();
      setStatus(next);
    } catch (loadError) {
      const message =
        loadError instanceof Error && loadError.message
          ? loadError.message
          : "Failed to load Claude Desktop status";
      setError(message);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const issues = useMemo(() => {
    if (!status) return [];
    const next: string[] = [];
    if (!status.supported) {
      next.push("3P profile management is currently supported on macOS and Windows.");
    }
    if (status.supported && !status.configured) {
      next.push("CC Switch profile has not been applied to Claude Desktop yet.");
    }
    if (
      status.expectedBaseUrl &&
      status.actualBaseUrl &&
      status.expectedBaseUrl !== status.actualBaseUrl
    ) {
      next.push("Claude Desktop profile base URL does not match the selected provider.");
    }
    if (status.mode === "proxy" && !status.proxyRunning) {
      next.push("Local proxy is not running, so proxy-mode Desktop routes will fail.");
    }
    if (status.staleRawModels) {
      next.push("Profile contains raw upstream model IDs; reapply the provider profile.");
    }
    if (status.missingRouteMappings) {
      next.push("Current provider is missing Claude Desktop model route mappings.");
    }
    if (status.mode === "proxy" && !status.gatewayTokenConfigured) {
      next.push("Gateway token is not configured for the local Claude Desktop route.");
    }
    return next;
  }, [status]);

  const handleImport = async () => {
    if (isImporting) return;
    setIsImporting(true);
    try {
      const imported =
        await providersApi.importClaudeDesktopProvidersFromClaude();
      toast.success(
        imported > 0
          ? `Imported ${imported} Claude Code provider(s)`
          : "No compatible Claude Code providers to import",
      );
      await loadStatus();
      onProvidersChanged?.();
    } catch (importError) {
      toast.error(
        importError instanceof Error && importError.message
          ? importError.message
          : "Failed to import Claude Code providers",
      );
    } finally {
      setIsImporting(false);
    }
  };

  return (
    <section className="mb-4 rounded-lg border border-border-default bg-card p-4 shadow-sm">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <FileJson className="h-4 w-4 text-muted-foreground" />
            <h2 className="text-base font-semibold">Claude Desktop Profile</h2>
          </div>
          <p className="text-sm text-muted-foreground">
            Applies a 3P profile for Claude Desktop. MCP and Prompt management
            stay disabled here because Desktop does not consume the same live
            config files as Claude Code.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            onClick={loadStatus}
            disabled={isLoading}
          >
            <RefreshCw className="h-4 w-4" />
            {isLoading ? "Refreshing..." : "Refresh"}
          </Button>
          <Button
            type="button"
            variant="outline"
            onClick={handleImport}
            disabled={isImporting}
          >
            <Import className="h-4 w-4" />
            {isImporting ? "Importing..." : "Import Claude Code"}
          </Button>
        </div>
      </div>

      {error ? (
        <div className="mt-3 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      ) : null}

      <div className="mt-4 grid gap-3 md:grid-cols-3">
        <StatusTile
          label="3P profile"
          value={status?.configured ? "Applied" : "Not applied"}
          ok={Boolean(status?.configured)}
          loading={isLoading && !status}
        />
        <StatusTile
          label="Mode"
          value={status?.mode === "proxy" ? "Local routing" : status?.mode ?? "Unknown"}
          ok={Boolean(status?.mode)}
          loading={isLoading && !status}
        />
        <StatusTile
          label="Local proxy"
          value={status?.proxyRunning ? "Running" : "Stopped"}
          ok={Boolean(status?.proxyRunning)}
          loading={isLoading && !status}
        />
      </div>

      <div className="mt-4 grid gap-3 text-xs md:grid-cols-2">
        <InfoLine label="Profile path" value={status?.profilePath} />
        <InfoLine label="Config library" value={status?.configLibraryPath} />
        <InfoLine label="Expected base URL" value={status?.expectedBaseUrl} />
        <InfoLine label="Actual base URL" value={status?.actualBaseUrl} />
      </div>

      <div className="mt-4 flex flex-wrap gap-2">
        <CapabilityBadge icon={Route} label="Provider cards show route mode" />
        <CapabilityBadge icon={GitBranch} label="Failover routes use Proxy settings" />
        <CapabilityBadge icon={AlertTriangle} label="MCP / Prompt unsupported" muted />
      </div>

      {issues.length > 0 ? (
        <div className="mt-4 rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-xs">
          <div className="mb-2 flex items-center gap-2 font-medium text-amber-700 dark:text-amber-300">
            <AlertTriangle className="h-4 w-4" />
            Attention needed
          </div>
          <ul className="space-y-1 text-muted-foreground">
            {issues.map((issue) => (
              <li key={issue}>{issue}</li>
            ))}
          </ul>
        </div>
      ) : status ? (
        <div className="mt-4 flex items-center gap-2 rounded-md border border-emerald-500/30 bg-emerald-500/10 p-3 text-xs text-emerald-700 dark:text-emerald-300">
          <CheckCircle2 className="h-4 w-4" />
          Claude Desktop profile status looks consistent.
        </div>
      ) : null}
    </section>
  );
}

function StatusTile({
  label,
  value,
  ok,
  loading,
}: {
  label: string;
  value: string;
  ok: boolean;
  loading: boolean;
}) {
  return (
    <div className="rounded-md border border-border-default bg-muted/30 px-3 py-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 flex items-center gap-2 text-sm font-medium">
        <span
          className={ok ? "h-2 w-2 rounded-full bg-emerald-500" : "h-2 w-2 rounded-full bg-amber-500"}
        />
        {loading ? "Loading..." : value}
      </div>
    </div>
  );
}

function InfoLine({ label, value }: { label: string; value?: string | null }) {
  return (
    <div className="min-w-0 rounded-md bg-muted/30 px-3 py-2">
      <div className="text-muted-foreground">{label}</div>
      <div className="mt-1 truncate font-mono" title={value || "Not detected"}>
        {value || "Not detected"}
      </div>
    </div>
  );
}

function CapabilityBadge({
  icon: Icon,
  label,
  muted = false,
}: {
  icon: typeof Route;
  label: string;
  muted?: boolean;
}) {
  return (
    <span
      className={
        muted
          ? "inline-flex items-center gap-1 rounded-md border border-amber-500/30 px-2 py-1 text-xs text-amber-700 dark:text-amber-300"
          : "inline-flex items-center gap-1 rounded-md border border-border-default px-2 py-1 text-xs text-muted-foreground"
      }
    >
      <Icon className="h-3.5 w-3.5" />
      {label}
    </span>
  );
}
