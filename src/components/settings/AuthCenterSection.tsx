import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Gauge,
  KeyRound,
  Loader2,
  LogOut,
  PlugZap,
  RefreshCw,
  Star,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  authApi,
  settingsApi,
  type ManagedAuthAccount,
  type ManagedAuthDeviceSession,
  type ManagedAuthProvider,
  type ManagedAuthUsage,
} from "@/lib/api";
import { SubscriptionQuotaPanel } from "./SubscriptionQuotaPanel";

const PROVIDERS: {
  id: ManagedAuthProvider;
  label: string;
  hintKey: string;
  hintDefault: string;
}[] = [
  {
    id: "github_copilot",
    label: "GitHub Copilot",
    hintKey: "authCenter.githubCopilotHint",
    hintDefault: "Copilot token for hosted Claude Desktop / proxy providers.",
  },
  {
    id: "codex_oauth",
    label: "Codex OAuth",
    hintKey: "authCenter.codexOAuthHint",
    hintDefault: "ChatGPT/Codex OAuth token for live models and proxy routing.",
  },
];

const providerLabel = (provider: ManagedAuthProvider) =>
  PROVIDERS.find((item) => item.id === provider)?.label ?? provider;

const accountKey = (account: ManagedAuthAccount) =>
  `${account.provider}:${account.id}`;

const isLoggedOut = (account: ManagedAuthAccount) =>
  account.status?.trim().toLowerCase() === "logged_out";

const deviceVerificationUrl = (session: ManagedAuthDeviceSession) =>
  session.verificationUriComplete || session.verificationUri;

const supportsUsageQuery = (provider: ManagedAuthProvider) =>
  provider === "github_copilot" || provider === "codex_oauth";

const emptyDraft = () => ({
  provider: "github_copilot" as ManagedAuthProvider,
  id: "",
  label: "",
  username: "",
  accessToken: "",
  refreshToken: "",
  expiresAt: "",
  scope: "",
  makeDefault: true,
});

export function AuthCenterSection() {
  const { t } = useTranslation();
  const mountedRef = useRef(true);
  const [accounts, setAccounts] = useState<ManagedAuthAccount[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [deviceSession, setDeviceSession] =
    useState<ManagedAuthDeviceSession | null>(null);
  const [usageByAccount, setUsageByAccount] = useState<
    Record<string, ManagedAuthUsage>
  >({});
  const [draft, setDraft] = useState(() => emptyDraft());

  const formatUsage = useCallback(
    (usage: ManagedAuthUsage) => {
      const remaining =
        typeof usage.remaining === "number" ? usage.remaining : undefined;
      const total = typeof usage.total === "number" ? usage.total : undefined;
      if (remaining !== undefined && total !== undefined) {
        return t("authCenter.usageRemainingTotal", {
          defaultValue: "{{remaining}} / {{total}} remaining",
          remaining: remaining.toLocaleString(),
          total: total.toLocaleString(),
        });
      }
      if (remaining !== undefined) {
        return t("authCenter.usageRemaining", {
          defaultValue: "{{remaining}} remaining",
          remaining: remaining.toLocaleString(),
        });
      }
      if (typeof usage.used === "number") {
        return t("authCenter.usageUsed", {
          defaultValue: "{{used}} used",
          used: usage.used.toLocaleString(),
        });
      }
      return t("authCenter.usageUnknown", {
        defaultValue: "Usage returned without normalized quota fields",
      });
    },
    [t],
  );

  const grouped = useMemo(() => {
    return PROVIDERS.map((provider) => ({
      ...provider,
      accounts: accounts.filter((account) => account.provider === provider.id),
    }));
  }, [accounts]);

  const loadAccounts = useCallback(async () => {
    setLoading(true);
    try {
      const nextAccounts = await authApi.listAccounts();
      if (mountedRef.current) {
        setAccounts(nextAccounts);
      }
    } catch (error) {
      if (mountedRef.current) {
        toast.error(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (mountedRef.current) {
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    void loadAccounts();
    return () => {
      mountedRef.current = false;
    };
  }, [loadAccounts]);

  const updateDraft = (updates: Partial<ReturnType<typeof emptyDraft>>) => {
    setDraft((current) => ({ ...current, ...updates }));
  };

  const handleImport = async () => {
    if (!draft.label.trim() || !draft.accessToken.trim()) {
      toast.error(
        t("authCenter.importRequired", {
          defaultValue: "Account label and access token are required.",
        }),
      );
      return;
    }
    setBusyKey("import");
    try {
      const account = await authApi.importAccount({
        provider: draft.provider,
        id: draft.id.trim() || undefined,
        label: draft.label.trim(),
        username: draft.username.trim() || undefined,
        makeDefault: draft.makeDefault,
        tokens: {
          accessToken: draft.accessToken.trim(),
          refreshToken: draft.refreshToken.trim() || undefined,
          expiresAt: draft.expiresAt.trim() || undefined,
          scope: draft.scope.trim() || undefined,
          tokenType: "Bearer",
        },
      });
      if (!mountedRef.current) return;
      setDraft({
        ...emptyDraft(),
        provider: draft.provider,
      });
      await loadAccounts();
      if (!mountedRef.current) return;
      toast.success(
        t("authCenter.importSuccess", {
          defaultValue: "{{label}} imported.",
          label: account.label,
        }),
      );
    } catch (error) {
      if (mountedRef.current) {
        toast.error(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (mountedRef.current) {
        setBusyKey(null);
      }
    }
  };

  const handleDeviceLogin = async (provider: ManagedAuthProvider) => {
    setBusyKey(`device:${provider}`);
    try {
      const session = await authApi.startDeviceLogin({ provider });
      if (!mountedRef.current) return;
      setDeviceSession(session);
      const verificationUrl = deviceVerificationUrl(session);
      try {
        await settingsApi.openExternal(verificationUrl);
      } catch (openError) {
        console.warn("[AuthCenter] Failed to open verification URL", openError);
      }
      if (!mountedRef.current) return;
      toast.info(
        t("authCenter.deviceToast", {
          defaultValue: "Open {{url}} and enter {{code}}",
          url: session.verificationUri,
          code: session.userCode,
        }),
      );
      let intervalMs = Math.max(session.intervalSeconds, 1) * 1000;
      const deadline = new Date(session.expiresAt).getTime();
      while (Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, intervalMs));
        if (!mountedRef.current) return;
        const result = await authApi.pollDeviceLogin({
          provider,
          sessionId: session.sessionId,
        });
        if (!mountedRef.current) return;
        if (result.status === "authorized") {
          await loadAccounts();
          if (!mountedRef.current) return;
          toast.success(
            t("authCenter.deviceConnected", {
              defaultValue: "{{provider}} account connected.",
              provider: providerLabel(provider),
            }),
          );
          return;
        }
        if (result.status === "slow_down") {
          intervalMs += 5000;
          continue;
        }
        if (result.status !== "pending") {
          throw new Error(result.message || `Device login ${result.status}`);
        }
      }
      throw new Error(
        t("authCenter.deviceExpired", {
          defaultValue: "Device login expired.",
        }),
      );
    } catch (error) {
      if (mountedRef.current) {
        toast.error(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (mountedRef.current) {
        setDeviceSession(null);
        setBusyKey(null);
      }
    }
  };

  const handleSetDefault = async (account: ManagedAuthAccount) => {
    setBusyKey(`default:${account.provider}:${account.id}`);
    try {
      await authApi.setDefault(account.provider, account.id);
      if (mountedRef.current) {
        await loadAccounts();
      }
    } catch (error) {
      if (mountedRef.current) {
        toast.error(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (mountedRef.current) {
        setBusyKey(null);
      }
    }
  };

  const handleDelete = async (account: ManagedAuthAccount) => {
    setBusyKey(`delete:${account.provider}:${account.id}`);
    try {
      await authApi.deleteAccount(account.provider, account.id);
      if (!mountedRef.current) return;
      await loadAccounts();
      if (!mountedRef.current) return;
      toast.success(
        t("authCenter.deleteSuccess", {
          defaultValue: "{{label}} deleted.",
          label: account.label,
        }),
      );
    } catch (error) {
      if (mountedRef.current) {
        toast.error(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (mountedRef.current) {
        setBusyKey(null);
      }
    }
  };

  const handleLogout = async (account: ManagedAuthAccount) => {
    setBusyKey(`logout:${account.provider}:${account.id}`);
    try {
      await authApi.logout(account.provider, account.id);
      if (!mountedRef.current) return;
      await loadAccounts();
      if (!mountedRef.current) return;
      toast.success(
        t("authCenter.logoutSuccess", {
          defaultValue: "{{label}} logged out.",
          label: account.label,
        }),
      );
    } catch (error) {
      if (mountedRef.current) {
        toast.error(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (mountedRef.current) {
        setBusyKey(null);
      }
    }
  };

  const handleQueryUsage = async (account: ManagedAuthAccount) => {
    const key = `usage:${account.provider}:${account.id}`;
    setBusyKey(key);
    try {
      const usage = await authApi.queryUsage(account.provider, account.id);
      if (mountedRef.current) {
        setUsageByAccount((current) => ({
          ...current,
          [accountKey(account)]: usage,
        }));
        toast.success(formatUsage(usage));
      }
    } catch (error) {
      if (mountedRef.current) {
        toast.error(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (mountedRef.current) {
        setBusyKey(null);
      }
    }
  };

  const deviceLoginBusy = busyKey?.startsWith("device:") ?? false;
  const authCenterBlocked = deviceLoginBusy || busyKey === "import";

  return (
    <div className="space-y-6">
      <SubscriptionQuotaPanel />
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="text-sm font-medium">
            {t("authCenter.title", { defaultValue: "Auth Center" })}
          </h3>
          <p className="text-xs text-muted-foreground">
            {t("authCenter.subtitle", {
              defaultValue:
                "Manage hosted GitHub Copilot and Codex OAuth accounts.",
            })}
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => void loadAccounts()}
          disabled={loading || authCenterBlocked}
        >
          {loading ? (
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          ) : (
            <RefreshCw className="mr-2 h-4 w-4" />
          )}
          {t("authCenter.refresh", { defaultValue: "Refresh" })}
        </Button>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        {grouped.map((group) => (
          <div
            key={group.id}
            className="space-y-3 rounded-md border border-border-default p-4"
          >
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="text-sm font-medium">{group.label}</div>
                <div className="text-xs text-muted-foreground">
                  {t(group.hintKey, {
                    defaultValue: group.hintDefault,
                  })}
                </div>
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => void handleDeviceLogin(group.id)}
                disabled={authCenterBlocked}
              >
                {busyKey === `device:${group.id}` ? (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                ) : (
                  <PlugZap className="mr-2 h-4 w-4" />
                )}
                {t("authCenter.device", { defaultValue: "Device" })}
              </Button>
            </div>

            {deviceSession?.provider === group.id ? (
              <div className="rounded-md border border-dashed border-border-default px-3 py-2 text-xs">
                <div className="font-medium">
                  {t("authCenter.deviceCode", {
                    defaultValue: "Device code: {{code}}",
                    code: deviceSession.userCode,
                  })}
                </div>
                <div className="mt-2 flex flex-wrap items-center justify-between gap-2 text-muted-foreground">
                  <span>
                    {t("authCenter.openVerification", {
                      defaultValue: "Open {{url}}",
                      url: deviceSession.verificationUri,
                    })}
                  </span>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() =>
                      void settingsApi.openExternal(
                        deviceVerificationUrl(deviceSession),
                      )
                    }
                  >
                    {t("authCenter.open", { defaultValue: "Open" })}
                  </Button>
                </div>
              </div>
            ) : null}

            {group.accounts.length === 0 ? (
              <div className="rounded-md bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
                {t("authCenter.noAccounts", {
                  defaultValue: "No accounts yet.",
                })}
              </div>
            ) : (
              <div className="space-y-2">
                {group.accounts.map((account) => {
                  const loggedOut = isLoggedOut(account);
                  return (
                    <div
                      key={accountKey(account)}
                      className="rounded-md border border-border-default px-3 py-2"
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="truncate text-sm font-medium">
                              {account.label}
                            </span>
                            {account.isDefault && !loggedOut ? (
                              <Badge variant="secondary">
                                {t("authCenter.default", {
                                  defaultValue: "Default",
                                })}
                              </Badge>
                            ) : null}
                            {loggedOut ? (
                              <Badge variant="outline">
                                {t("authCenter.loggedOut", {
                                  defaultValue: "Logged out",
                                })}
                              </Badge>
                            ) : null}
                          </div>
                          <div className="mt-1 text-xs text-muted-foreground">
                            {account.username || account.id}
                          </div>
                          {account.expiresAt ? (
                            <div className="mt-1 text-xs text-muted-foreground">
                              {t("authCenter.expires", {
                                defaultValue: "Expires {{time}}",
                                time: new Date(
                                  account.expiresAt,
                                ).toLocaleString(),
                              })}
                            </div>
                          ) : null}
                          {usageByAccount[accountKey(account)] ? (
                            <div className="mt-1 text-xs text-muted-foreground">
                              {formatUsage(usageByAccount[accountKey(account)])}
                              {usageByAccount[accountKey(account)].resetAt
                                ? t("authCenter.usageReset", {
                                    defaultValue: ", resets {{time}}",
                                    time: new Date(
                                      usageByAccount[
                                        accountKey(account)
                                      ].resetAt!,
                                    ).toLocaleString(),
                                  })
                                : ""}
                            </div>
                          ) : null}
                        </div>
                        <div className="flex shrink-0 items-center gap-1">
                          {supportsUsageQuery(account.provider) ? (
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon"
                              disabled={loggedOut || authCenterBlocked}
                              onClick={() => void handleQueryUsage(account)}
                              title={t("authCenter.queryUsage", {
                                defaultValue: "Query usage",
                              })}
                            >
                              {busyKey ===
                              `usage:${account.provider}:${account.id}` ? (
                                <Loader2 className="h-4 w-4 animate-spin" />
                              ) : (
                                <Gauge className="h-4 w-4" />
                              )}
                            </Button>
                          ) : null}
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            disabled={
                              account.isDefault ||
                              loggedOut ||
                              authCenterBlocked
                            }
                            onClick={() => void handleSetDefault(account)}
                            title={t("authCenter.setDefault", {
                              defaultValue: "Set default",
                            })}
                          >
                            <Star className="h-4 w-4" />
                          </Button>
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            disabled={loggedOut || authCenterBlocked}
                            onClick={() => void handleLogout(account)}
                            title={t("authCenter.logout", {
                              defaultValue: "Logout",
                            })}
                          >
                            <LogOut className="h-4 w-4" />
                          </Button>
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            disabled={authCenterBlocked}
                            onClick={() => void handleDelete(account)}
                            title={t("authCenter.delete", {
                              defaultValue: "Delete",
                            })}
                          >
                            <Trash2 className="h-4 w-4" />
                          </Button>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        ))}
      </div>

      <div className="space-y-4 rounded-md border border-border-default p-4">
        <div>
          <h4 className="text-sm font-medium">
            {t("authCenter.importTitle", {
              defaultValue: "Import Existing Token",
            })}
          </h4>
          <p className="text-xs text-muted-foreground">
            {t("authCenter.importHint", {
              defaultValue:
                "Import a token manually when device login is unavailable or you need a specific account.",
            })}
          </p>
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <Label>
              {t("authCenter.provider", { defaultValue: "Provider" })}
            </Label>
            <Select
              value={draft.provider}
              onValueChange={(value) =>
                updateDraft({ provider: value as ManagedAuthProvider })
              }
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {PROVIDERS.map((provider) => (
                  <SelectItem key={provider.id} value={provider.id}>
                    {provider.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-2">
            <Label htmlFor="auth-center-label">
              {t("authCenter.label", { defaultValue: "Label" })}
            </Label>
            <Input
              id="auth-center-label"
              value={draft.label}
              onChange={(event) => updateDraft({ label: event.target.value })}
              placeholder={providerLabel(draft.provider)}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="auth-center-id">
              {t("authCenter.accountId", { defaultValue: "Account ID" })}
            </Label>
            <Input
              id="auth-center-id"
              value={draft.id}
              onChange={(event) => updateDraft({ id: event.target.value })}
              placeholder={t("authCenter.autoGenerated", {
                defaultValue: "auto-generated",
              })}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="auth-center-username">
              {t("authCenter.username", { defaultValue: "Username" })}
            </Label>
            <Input
              id="auth-center-username"
              value={draft.username}
              onChange={(event) =>
                updateDraft({ username: event.target.value })
              }
              placeholder={t("authCenter.optional", {
                defaultValue: "optional",
              })}
            />
          </div>
          <div className="space-y-2 md:col-span-2">
            <Label htmlFor="auth-center-access-token">
              {t("authCenter.accessToken", { defaultValue: "Access Token" })}
            </Label>
            <Input
              id="auth-center-access-token"
              value={draft.accessToken}
              onChange={(event) =>
                updateDraft({ accessToken: event.target.value })
              }
              type="password"
              autoComplete="off"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="auth-center-refresh-token">
              {t("authCenter.refreshToken", { defaultValue: "Refresh Token" })}
            </Label>
            <Input
              id="auth-center-refresh-token"
              value={draft.refreshToken}
              onChange={(event) =>
                updateDraft({ refreshToken: event.target.value })
              }
              type="password"
              autoComplete="off"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="auth-center-expires">
              {t("authCenter.expiresAt", { defaultValue: "Expires At" })}
            </Label>
            <Input
              id="auth-center-expires"
              value={draft.expiresAt}
              onChange={(event) =>
                updateDraft({ expiresAt: event.target.value })
              }
              placeholder="2026-06-08T12:00:00Z"
            />
          </div>
          <div className="space-y-2 md:col-span-2">
            <Label htmlFor="auth-center-scope">
              {t("authCenter.scope", { defaultValue: "Scope" })}
            </Label>
            <Input
              id="auth-center-scope"
              value={draft.scope}
              onChange={(event) => updateDraft({ scope: event.target.value })}
              placeholder={t("authCenter.optional", {
                defaultValue: "optional",
              })}
            />
          </div>
        </div>
        <div className="flex items-center justify-between gap-3">
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={draft.makeDefault}
              onChange={(event) =>
                updateDraft({ makeDefault: event.target.checked })
              }
            />
            {t("authCenter.makeDefault", {
              defaultValue: "Set as default account",
            })}
          </label>
          <Button
            type="button"
            onClick={() => void handleImport()}
            disabled={authCenterBlocked}
          >
            {busyKey === "import" ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <KeyRound className="mr-2 h-4 w-4" />
            )}
            {t("authCenter.importToken", { defaultValue: "Import Token" })}
          </Button>
        </div>
      </div>
    </div>
  );
}
