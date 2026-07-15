import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
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
  refreshKey?: number;
}

const KNOWN_BACKEND_ISSUES = new Set([
  "Claude Desktop 3P profile management is only supported on macOS and Windows.",
  "CC Switch profile has not been applied to Claude Desktop yet.",
  "Claude Desktop profile base URL does not match the selected provider.",
  "Local proxy is not running, so proxy-mode Desktop routes will fail.",
  "Profile contains raw upstream model IDs; reapply the provider profile.",
  "Current provider is missing Claude Desktop model route mappings.",
  "Gateway token is not configured for the local Claude Desktop route.",
]);

export function ClaudeDesktopPanel({
  onProvidersChanged,
  refreshKey = 0,
}: ClaudeDesktopPanelProps) {
  const { t } = useTranslation();
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
          : t("claudeDesktopPanel.loadFailed", {
              defaultValue: "Failed to load Claude Desktop status",
            });
      setError(message);
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus, refreshKey]);

  const issues = useMemo(() => {
    if (!status) return [];
    if (!status.supported) {
      return [
        t("claudeDesktopPanel.unsupported", {
          defaultValue:
            "Claude Desktop 3P profile management is available only on macOS and Windows.",
        }),
      ];
    }
    const next: string[] = [];
    if (!status.configured) {
      next.push(
        t("claudeDesktopPanel.issueNotApplied", {
          defaultValue:
            "CC Switch profile has not been applied to Claude Desktop yet.",
        }),
      );
    }
    if (
      status.expectedBaseUrl &&
      status.actualBaseUrl &&
      status.expectedBaseUrl !== status.actualBaseUrl
    ) {
      next.push(
        t("claudeDesktopPanel.issueBaseUrlMismatch", {
          defaultValue:
            "Claude Desktop profile base URL does not match the selected provider.",
        }),
      );
    }
    if (status.mode === "proxy" && !status.proxyRunning) {
      next.push(
        t("claudeDesktopPanel.issueProxyStopped", {
          defaultValue:
            "Local proxy is not running, so proxy-mode Desktop routes will fail.",
        }),
      );
    }
    if (status.staleRawModels) {
      next.push(
        t("claudeDesktopPanel.issueStaleModels", {
          defaultValue:
            "Profile contains raw upstream model IDs; reapply the provider profile.",
        }),
      );
    }
    if (status.missingRouteMappings) {
      next.push(
        t("claudeDesktopPanel.issueMissingRoutes", {
          defaultValue:
            "Current provider is missing Claude Desktop model route mappings.",
        }),
      );
    }
    if (status.mode === "proxy" && !status.gatewayTokenConfigured) {
      next.push(
        t("claudeDesktopPanel.issueGatewayToken", {
          defaultValue:
            "Gateway token is not configured for the local Claude Desktop route.",
        }),
      );
    }
    for (const issue of status.issues ?? []) {
      if (!KNOWN_BACKEND_ISSUES.has(issue) && !next.includes(issue)) {
        next.push(issue);
      }
    }
    return next;
  }, [status, t]);

  const handleImport = async () => {
    if (isImporting) return;
    setIsImporting(true);
    try {
      const imported =
        await providersApi.importClaudeDesktopProvidersFromClaude();
      toast.success(
        imported > 0
          ? t("claudeDesktopPanel.importSuccess", {
              defaultValue: "Imported {{count}} Claude Code provider(s)",
              count: imported,
            })
          : t("claudeDesktopPanel.importEmpty", {
              defaultValue: "No compatible Claude Code providers to import",
            }),
      );
      await loadStatus();
      onProvidersChanged?.();
    } catch (importError) {
      toast.error(
        importError instanceof Error && importError.message
          ? importError.message
          : t("claudeDesktopPanel.importFailed", {
              defaultValue: "Failed to import Claude Code providers",
            }),
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
            <h2 className="text-base font-semibold">
              {t("claudeDesktopPanel.title", {
                defaultValue: "Claude Desktop Profile",
              })}
            </h2>
          </div>
          <p className="text-sm text-muted-foreground">
            {t("claudeDesktopPanel.description", {
              defaultValue:
                "Applies a 3P profile on the host running CC Switch. Claude Desktop does not consume the same MCP, Prompt, or Skills configuration as Claude Code.",
            })}
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
            {isLoading
              ? t("claudeDesktopPanel.refreshing", {
                  defaultValue: "Refreshing...",
                })
              : t("claudeDesktopPanel.refresh", { defaultValue: "Refresh" })}
          </Button>
          <Button
            type="button"
            variant="outline"
            onClick={handleImport}
            disabled={isImporting}
          >
            <Import className="h-4 w-4" />
            {isImporting
              ? t("claudeDesktopPanel.importing", {
                  defaultValue: "Importing...",
                })
              : t("claudeDesktopPanel.importClaude", {
                  defaultValue: "Import Claude Code",
                })}
          </Button>
        </div>
      </div>

      {error ? (
        <div className="mt-3 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      ) : null}

      <div className="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <StatusTile
          label={t("claudeDesktopPanel.profile", {
            defaultValue: "3P profile",
          })}
          value={
            status?.configured
              ? t("claudeDesktopPanel.applied", { defaultValue: "Applied" })
              : t("claudeDesktopPanel.notApplied", {
                  defaultValue: "Not applied",
                })
          }
          ok={Boolean(status?.configured)}
          loading={isLoading && !status}
          loadingText={t("common.loading", { defaultValue: "Loading..." })}
        />
        <StatusTile
          label={t("claudeDesktopPanel.mode", { defaultValue: "Mode" })}
          value={
            status?.mode === "proxy"
              ? t("claudeDesktopPanel.localRouting", {
                  defaultValue: "Local routing",
                })
              : status?.mode === "direct"
                ? t("claudeDesktopPanel.direct", { defaultValue: "Direct" })
                : t("common.unknown", { defaultValue: "Unknown" })
          }
          ok={Boolean(status?.mode)}
          loading={isLoading && !status}
          loadingText={t("common.loading", { defaultValue: "Loading..." })}
        />
        <StatusTile
          label={t("claudeDesktopPanel.localProxy", {
            defaultValue: "Local proxy",
          })}
          value={
            status?.proxyRunning
              ? t("claudeDesktopPanel.running", { defaultValue: "Running" })
              : t("claudeDesktopPanel.stopped", { defaultValue: "Stopped" })
          }
          ok={Boolean(status?.proxyRunning)}
          loading={isLoading && !status}
          loadingText={t("common.loading", { defaultValue: "Loading..." })}
        />
        <StatusTile
          label={t("claudeDesktopPanel.desktopRestart", {
            defaultValue: "Desktop restart",
          })}
          value={
            !status?.supported
              ? t("claudeDesktopPanel.unsupportedValue", {
                  defaultValue: "Unsupported",
                })
              : status.needsRestart
                ? t("claudeDesktopPanel.restartRequired", {
                    defaultValue: "Required",
                  })
                : status.desktopRunning
                  ? t("claudeDesktopPanel.restartLoaded", {
                      defaultValue: "Profile loaded",
                    })
                  : t("claudeDesktopPanel.restartOnNextLaunch", {
                      defaultValue: "Applies on next launch",
                    })
          }
          ok={Boolean(status?.supported && !status.needsRestart)}
          loading={isLoading && !status}
          loadingText={t("common.loading", { defaultValue: "Loading..." })}
        />
      </div>

      {status?.needsRestart ? (
        <div className="mt-4 rounded-md border border-sky-500/30 bg-sky-500/10 px-3 py-2 text-xs text-sky-700 dark:text-sky-300">
          {t("claudeDesktopPanel.restartHint", {
            defaultValue:
              "The profile changed while Claude Desktop was running. Fully quit and reopen Claude Desktop to load it.",
          })}
        </div>
      ) : null}

      <div className="mt-4 grid gap-3 text-xs md:grid-cols-2">
        <InfoLine
          label={t("claudeDesktopPanel.profilePath", {
            defaultValue: "Profile path",
          })}
          value={status?.profilePath}
          fallback={t("claudeDesktopPanel.notDetected", {
            defaultValue: "Not detected",
          })}
        />
        <InfoLine
          label={t("claudeDesktopPanel.configLibrary", {
            defaultValue: "Config library",
          })}
          value={status?.configLibraryPath}
          fallback={t("claudeDesktopPanel.notDetected", {
            defaultValue: "Not detected",
          })}
        />
        <InfoLine
          label={t("claudeDesktopPanel.expectedBaseUrl", {
            defaultValue: "Expected base URL",
          })}
          value={status?.expectedBaseUrl}
          fallback={t("claudeDesktopPanel.notDetected", {
            defaultValue: "Not detected",
          })}
        />
        <InfoLine
          label={t("claudeDesktopPanel.actualBaseUrl", {
            defaultValue: "Actual base URL",
          })}
          value={status?.actualBaseUrl}
          fallback={t("claudeDesktopPanel.notDetected", {
            defaultValue: "Not detected",
          })}
        />
      </div>

      <div className="mt-4 flex flex-wrap gap-2">
        <CapabilityBadge
          icon={Route}
          label={t("claudeDesktopPanel.routeMode", {
            defaultValue: "Provider cards show route mode",
          })}
        />
        <CapabilityBadge
          icon={GitBranch}
          label={t("claudeDesktopPanel.failover", {
            defaultValue: "Failover routes use Proxy settings",
          })}
        />
        <CapabilityBadge
          icon={AlertTriangle}
          label={t("claudeDesktopPanel.unsupportedFeatures", {
            defaultValue: "MCP / Prompt / Skills unsupported",
          })}
          muted
        />
      </div>

      {issues.length > 0 ? (
        <div className="mt-4 rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-xs">
          <div className="mb-2 flex items-center gap-2 font-medium text-amber-700 dark:text-amber-300">
            <AlertTriangle className="h-4 w-4" />
            {t("claudeDesktopPanel.attention", {
              defaultValue: "Attention needed",
            })}
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
          {t("claudeDesktopPanel.consistent", {
            defaultValue: "Claude Desktop profile status looks consistent.",
          })}
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
  loadingText,
}: {
  label: string;
  value: string;
  ok: boolean;
  loading: boolean;
  loadingText: string;
}) {
  return (
    <div className="rounded-md border border-border-default bg-muted/30 px-3 py-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 flex items-center gap-2 text-sm font-medium">
        <span
          className={
            ok
              ? "h-2 w-2 rounded-full bg-emerald-500"
              : "h-2 w-2 rounded-full bg-amber-500"
          }
        />
        {loading ? loadingText : value}
      </div>
    </div>
  );
}

function InfoLine({
  label,
  value,
  fallback,
}: {
  label: string;
  value?: string | null;
  fallback: string;
}) {
  return (
    <div className="min-w-0 rounded-md bg-muted/30 px-3 py-2">
      <div className="text-muted-foreground">{label}</div>
      <div className="mt-1 truncate font-mono" title={value || fallback}>
        {value || fallback}
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
