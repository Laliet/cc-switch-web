import { useEffect, useState } from "react";
import {
  Download,
  Eye,
  History,
  RefreshCw,
  RotateCcw,
  Upload,
} from "lucide-react";
import { toast } from "sonner";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { settingsApi } from "@/lib/api";
import type {
  WebDavSettings,
  WebDavAutoSyncResult,
  WebDavBackupEntry,
  WebDavSnapshotPreview,
  WebDavSyncResult,
} from "@/types";

interface WebDavSettingsSectionProps {
  value?: WebDavSettings;
  onChange: (value: WebDavSettings) => void;
}

const DEFAULT_WEBDAV_SETTINGS: WebDavSettings = {
  enabled: false,
  autoSync: false,
  baseUrl: "",
  username: "",
  password: "",
  remoteDir: "cc-switch-web",
  profile: "default",
  lastSyncStatus: "idle",
};

export function WebDavSettingsSection({
  value,
  onChange,
}: WebDavSettingsSectionProps) {
  const [busyAction, setBusyAction] = useState<
    "upload" | "preview" | "download" | "sync" | "backups" | "restore" | null
  >(null);
  const [preview, setPreview] = useState<WebDavSnapshotPreview | null>(null);
  const [syncResult, setSyncResult] = useState<WebDavAutoSyncResult | null>(
    null,
  );
  const [backups, setBackups] = useState<WebDavBackupEntry[]>([]);
  const [lastResult, setLastResult] = useState<WebDavSyncResult | null>(null);
  const [confirmDownload, setConfirmDownload] = useState(false);
  const [restoreBackup, setRestoreBackup] = useState<WebDavBackupEntry | null>(
    null,
  );
  const settings = { ...DEFAULT_WEBDAV_SETTINGS, ...(value ?? {}) };
  const [runtimeStatus, setRuntimeStatus] = useState({
    status: settings.lastSyncStatus ?? "idle",
    error: settings.lastSyncError,
    syncedAt: settings.lastSyncAt,
  });

  const update = (patch: Partial<WebDavSettings>) => {
    onChange({ ...settings, ...patch });
  };

  const markSynced = (nextPreview?: WebDavSnapshotPreview | null) => {
    const configHash = nextPreview?.configHash;
    if (!configHash) return;
    update({
      lastSyncConfigHash: configHash,
      lastSyncAt: new Date().toISOString(),
      lastSyncRemoteSnapshotId: nextPreview.snapshotId,
    });
    setRuntimeStatus({
      status: "success",
      error: undefined,
      syncedAt: new Date().toISOString(),
    });
  };

  const runAction = async (
    action: "upload" | "preview" | "download" | "sync" | "backups" | "restore",
    task: () => Promise<void>,
  ) => {
    if (busyAction) return;
    setBusyAction(action);
    try {
      await task();
    } catch (error) {
      const message = friendlyWebDavError(error);
      setRuntimeStatus((current) => ({
        ...current,
        status: "error",
        error: message,
      }));
      toast.error(message);
    } finally {
      setBusyAction(null);
    }
  };

  const handleUpload = () =>
    runAction("upload", async () => {
      const result = await settingsApi.uploadWebDavSnapshot(settings);
      setPreview(result.preview ?? null);
      setLastResult(result);
      setSyncResult(null);
      markSynced(result.preview);
      setBackups(await settingsApi.listWebDavBackups(settings));
      toast.success(result.message || "Snapshot uploaded");
    });

  const handlePreview = () =>
    runAction("preview", async () => {
      const result = await settingsApi.previewWebDavSnapshot(settings);
      setPreview(result);
      setLastResult(null);
      setSyncResult(null);
      setBackups(await settingsApi.listWebDavBackups(settings));
      toast.success(
        result.exists ? "Remote snapshot loaded" : "Remote snapshot not found",
      );
    });

  const handleDownload = () =>
    runAction("download", async () => {
      const result = await settingsApi.downloadWebDavSnapshot(settings);
      setPreview(result.preview ?? null);
      setLastResult(result);
      setSyncResult(null);
      markSynced(result.preview);
      toast.success(result.message || "Snapshot downloaded");
    });

  const handleSync = (silent = false) =>
    runAction("sync", async () => {
      const result = await settingsApi.syncWebDavSnapshot(settings);
      setSyncResult(result);
      const nextPreview =
        result.result?.preview ?? result.remotePreview ?? null;
      setPreview(nextPreview);
      if (result.action === "conflict") {
        toast.warning(result.message || "WebDAV sync needs review");
        return;
      }
      if (result.action === "uploaded") {
        setBackups(await settingsApi.listWebDavBackups(settings));
      }
      if (result.action === "uploaded" || result.action === "downloaded") {
        setLastResult(result.result ?? null);
      }
      markSynced(nextPreview);
      if (!silent) {
        toast.success(result.message || "WebDAV sync complete");
      }
    });

  const handleListBackups = () =>
    runAction("backups", async () => {
      const result = await settingsApi.listWebDavBackups(settings);
      setBackups(result);
      toast.success(result.length ? "Remote backups loaded" : "No backups yet");
    });

  const handleRestore = (backup: WebDavBackupEntry) =>
    runAction("restore", async () => {
      const result = await settingsApi.restoreWebDavBackup(backup.id, settings);
      setPreview(result.preview ?? null);
      setLastResult(result);
      setSyncResult(null);
      toast.success(result.message || "Backup restored");
    });

  useEffect(() => {
    if (!settings.enabled || !settings.autoSync) return;
    const runIfIdle = () => {
      if (busyAction === null && !document.hidden) {
        void handleSync(true);
      }
    };
    const timer = window.setTimeout(runIfIdle, 2500);
    const interval = window.setInterval(runIfIdle, 5 * 60 * 1000);
    return () => {
      window.clearTimeout(timer);
      window.clearInterval(interval);
    };
  }, [
    settings.enabled,
    settings.autoSync,
    settings.baseUrl,
    settings.username,
    settings.password,
    settings.remoteDir,
    settings.profile,
    settings.lastSyncConfigHash,
    busyAction,
  ]);

  useEffect(() => {
    if (!settings.enabled) return;
    const refreshStatus = async () => {
      try {
        const latest = await settingsApi.get();
        const webDav = latest.webDav;
        if (!webDav) return;
        setRuntimeStatus({
          status: webDav.lastSyncStatus ?? "idle",
          error: webDav.lastSyncError,
          syncedAt: webDav.lastSyncAt,
        });
      } catch {
        // The regular action error path remains responsible for user feedback.
      }
    };
    void refreshStatus();
    const timer = window.setInterval(refreshStatus, 5000);
    return () => window.clearInterval(timer);
  }, [settings.enabled]);

  return (
    <section className="space-y-4">
      <header className="space-y-1">
        <div className="flex items-center justify-between gap-4">
          <h3 className="text-sm font-medium">WebDAV Cloud Sync</h3>
          <Switch
            checked={settings.enabled}
            onCheckedChange={(enabled) => update({ enabled })}
            aria-label="Enable WebDAV sync"
          />
        </div>
        <p className="text-xs text-muted-foreground">
          Snapshot sync with versioned backups and conflict review.
        </p>
      </header>

      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Base URL" htmlFor="webdav-base-url">
          <Input
            id="webdav-base-url"
            value={settings.baseUrl}
            placeholder="https://dav.example.com/remote.php/dav/files/me"
            onChange={(event) => update({ baseUrl: event.target.value })}
          />
        </Field>
        <Field label="Remote Directory" htmlFor="webdav-remote-dir">
          <Input
            id="webdav-remote-dir"
            value={settings.remoteDir}
            placeholder="cc-switch-web"
            onChange={(event) => update({ remoteDir: event.target.value })}
          />
        </Field>
        <Field label="Username" htmlFor="webdav-username">
          <Input
            id="webdav-username"
            value={settings.username}
            autoComplete="username"
            onChange={(event) => update({ username: event.target.value })}
          />
        </Field>
        <Field label="Password" htmlFor="webdav-password">
          <Input
            id="webdav-password"
            type="password"
            value={settings.password}
            autoComplete="current-password"
            onChange={(event) => update({ password: event.target.value })}
          />
        </Field>
        <Field label="Profile" htmlFor="webdav-profile">
          <Input
            id="webdav-profile"
            value={settings.profile}
            placeholder="default"
            onChange={(event) => update({ profile: event.target.value })}
          />
        </Field>
        <div className="flex items-center justify-between gap-3 rounded-md border border-border-default p-3">
          <div>
            <Label htmlFor="webdav-auto-sync">Auto Sync</Label>
            {runtimeStatus.syncedAt ? (
              <p className="mt-1 text-xs text-muted-foreground">
                Last sync: {runtimeStatus.syncedAt}
              </p>
            ) : null}
            <p className="mt-1 text-xs text-muted-foreground">
              Status: {runtimeStatus.status}
            </p>
            {runtimeStatus.error ? (
              <p className="mt-1 max-w-72 break-words text-xs text-destructive">
                {runtimeStatus.error}
              </p>
            ) : null}
          </div>
          <Switch
            id="webdav-auto-sync"
            checked={settings.autoSync}
            onCheckedChange={(autoSync) => update({ autoSync })}
          />
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          onClick={() => handleSync(false)}
          disabled={!settings.enabled || busyAction !== null}
        >
          <RefreshCw className="mr-2 h-4 w-4" />
          {busyAction === "sync" ? "Syncing..." : "Sync"}
        </Button>
        <Button
          type="button"
          variant="outline"
          onClick={handleUpload}
          disabled={!settings.enabled || busyAction !== null}
        >
          <Upload className="mr-2 h-4 w-4" />
          {busyAction === "upload" ? "Uploading..." : "Upload"}
        </Button>
        <Button
          type="button"
          variant="outline"
          onClick={handlePreview}
          disabled={!settings.enabled || busyAction !== null}
        >
          <Eye className="mr-2 h-4 w-4" />
          {busyAction === "preview" ? "Checking..." : "Preview"}
        </Button>
        <Button
          type="button"
          variant="outline"
          onClick={() => setConfirmDownload(true)}
          disabled={!settings.enabled || busyAction !== null}
        >
          <Download className="mr-2 h-4 w-4" />
          {busyAction === "download" ? "Downloading..." : "Download"}
        </Button>
        <Button
          type="button"
          variant="outline"
          onClick={handleListBackups}
          disabled={!settings.enabled || busyAction !== null}
        >
          <History className="mr-2 h-4 w-4" />
          {busyAction === "backups" ? "Loading..." : "Backups"}
        </Button>
      </div>

      {lastResult?.backupId ? (
        <div className="rounded-md border border-emerald-500/30 bg-emerald-500/10 p-3 text-xs">
          <div className="font-medium text-emerald-700 dark:text-emerald-300">
            Local backup created before applying remote snapshot
          </div>
          <p className="mt-1 break-all text-muted-foreground">
            Backup ID: {lastResult.backupId}
          </p>
        </div>
      ) : null}

      {preview ? (
        <div className="rounded-md border border-border-default p-3 text-xs">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="font-medium">
              {preview.exists ? "Remote Snapshot" : "No Remote Snapshot"}
            </span>
            <span
              className={
                preview.compatible ? "text-emerald-600" : "text-amber-600"
              }
            >
              {preview.compatible ? "Compatible" : "Needs attention"}
            </span>
          </div>
          <div className="mt-2 space-y-1 text-muted-foreground">
            <p className="break-all">{preview.remotePath}</p>
            {preview.modifiedAt ? <p>Modified: {preview.modifiedAt}</p> : null}
            {preview.sizeBytes ? <p>Size: {preview.sizeBytes} bytes</p> : null}
            {preview.createdAt ? <p>Created: {preview.createdAt}</p> : null}
            {preview.snapshotId ? (
              <p className="break-all">Snapshot ID: {preview.snapshotId}</p>
            ) : null}
            {preview.configVersion ? (
              <p>Config version: {preview.configVersion}</p>
            ) : null}
            {preview.schemaVersion ? (
              <p>Schema version: {preview.schemaVersion}</p>
            ) : null}
            {preview.artifactList.length ? (
              <p>Artifacts: {preview.artifactList.join(", ")}</p>
            ) : null}
            {preview.checks.map((check) => (
              <p key={check.name}>
                {check.ok ? "OK" : "WARN"} {check.name}: {check.message}
              </p>
            ))}
          </div>
        </div>
      ) : null}

      {preview?.exists ? (
        <div className="rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-700 dark:text-amber-300">
          Remote snapshot exists. Upload replaces the latest snapshot and keeps
          a versioned backup for restore.
        </div>
      ) : null}

      {syncResult?.action === "conflict" ? (
        <div className="rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-xs">
          <div className="font-medium text-amber-700 dark:text-amber-300">
            WebDAV sync conflict
          </div>
          <p className="mt-1 text-muted-foreground">
            Local and remote snapshots differ from the last synced version.
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={handleUpload}
              disabled={!settings.enabled || busyAction !== null}
            >
              <Upload className="mr-2 h-4 w-4" />
              Use Local
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setConfirmDownload(true)}
              disabled={!settings.enabled || busyAction !== null}
            >
              <Download className="mr-2 h-4 w-4" />
              Use Remote
            </Button>
          </div>
        </div>
      ) : null}

      {backups.length ? (
        <div className="rounded-md border border-border-default p-3 text-xs">
          <div className="mb-2 flex items-center justify-between gap-2">
            <span className="font-medium">Remote Backups</span>
            <span className="text-muted-foreground">{backups.length} kept</span>
          </div>
          <div className="space-y-2">
            {backups.map((backup) => (
              <div
                key={backup.id}
                className="flex flex-col gap-2 rounded border border-border-default p-2 sm:flex-row sm:items-center sm:justify-between"
              >
                <div className="min-w-0 space-y-1">
                  <p className="break-all font-medium">{backup.id}</p>
                  <p className="text-muted-foreground">
                    {backup.createdAt ?? backup.modifiedAt ?? "Unknown time"}
                    {backup.artifactList.length
                      ? ` · ${backup.artifactList.join(", ")}`
                      : ""}
                  </p>
                  <p
                    className={
                      backup.compatible ? "text-emerald-600" : "text-amber-600"
                    }
                  >
                    {backup.compatible ? "Compatible" : "Needs attention"}
                  </p>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => setRestoreBackup(backup)}
                  disabled={
                    !settings.enabled ||
                    busyAction !== null ||
                    !backup.compatible
                  }
                >
                  <RotateCcw className="mr-2 h-4 w-4" />
                  Restore
                </Button>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      <ConfirmDialog
        isOpen={confirmDownload}
        title="Download WebDAV snapshot?"
        message={
          preview?.exists
            ? `The remote snapshot will replace local provider configuration.\n\n${preview.remotePath}\n\nA local backup will be created before import.`
            : "The remote snapshot will replace local provider configuration. A local backup will be created before import."
        }
        confirmText="Download"
        onConfirm={() => {
          setConfirmDownload(false);
          handleDownload();
        }}
        onCancel={() => setConfirmDownload(false)}
      />
      <ConfirmDialog
        isOpen={restoreBackup !== null}
        title="Restore WebDAV backup?"
        message={
          restoreBackup
            ? `This backup will replace local provider configuration.\n\n${restoreBackup.id}\n\nA local backup will be created before import.`
            : ""
        }
        confirmText="Restore"
        onConfirm={() => {
          const backup = restoreBackup;
          setRestoreBackup(null);
          if (backup) handleRestore(backup);
        }}
        onCancel={() => setRestoreBackup(null)}
      />
    </section>
  );
}

function friendlyWebDavError(error: unknown): string {
  const raw =
    error instanceof Error && error.message
      ? error.message
      : typeof error === "string"
        ? error
        : "";
  if (!raw) return "WebDAV sync failed";
  if (raw.includes("401") || raw.includes("403")) {
    return "WebDAV authentication failed. Check username, password, and server permissions.";
  }
  if (raw.includes("404") || raw.includes("not found")) {
    return "Remote WebDAV snapshot was not found. Preview the remote path or upload a snapshot first.";
  }
  if (raw.includes("timed out") || raw.includes("timeout")) {
    return "WebDAV request timed out. Check the server address and network connection.";
  }
  if (raw.includes("compatible")) {
    return "Remote WebDAV snapshot is not compatible with this version.";
  }
  return raw;
}

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-2">
      <Label htmlFor={htmlFor}>{label}</Label>
      {children}
    </div>
  );
}
