import { useState } from "react";
import { Download, Eye, Upload } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { settingsApi } from "@/lib/api";
import type { WebDavSettings, WebDavSnapshotPreview } from "@/types";

interface WebDavSettingsSectionProps {
  value?: WebDavSettings;
  onChange: (value: WebDavSettings) => void;
}

const DEFAULT_WEBDAV_SETTINGS: WebDavSettings = {
  enabled: false,
  baseUrl: "",
  username: "",
  password: "",
  remoteDir: "cc-switch-web",
  profile: "default",
};

export function WebDavSettingsSection({
  value,
  onChange,
}: WebDavSettingsSectionProps) {
  const [busyAction, setBusyAction] = useState<
    "upload" | "preview" | "download" | null
  >(null);
  const [preview, setPreview] = useState<WebDavSnapshotPreview | null>(null);
  const settings = { ...DEFAULT_WEBDAV_SETTINGS, ...(value ?? {}) };

  const update = (patch: Partial<WebDavSettings>) => {
    onChange({ ...settings, ...patch });
  };

  const runAction = async (
    action: "upload" | "preview" | "download",
    task: () => Promise<void>,
  ) => {
    if (busyAction) return;
    setBusyAction(action);
    try {
      await task();
    } catch (error) {
      const message =
        error instanceof Error && error.message
          ? error.message
          : "WebDAV sync failed";
      toast.error(message);
    } finally {
      setBusyAction(null);
    }
  };

  const handleUpload = () =>
    runAction("upload", async () => {
      const result = await settingsApi.uploadWebDavSnapshot(settings);
      setPreview(result.preview ?? null);
      toast.success(result.message || "Snapshot uploaded");
    });

  const handlePreview = () =>
    runAction("preview", async () => {
      const result = await settingsApi.previewWebDavSnapshot(settings);
      setPreview(result);
      toast.success(
        result.exists ? "Remote snapshot loaded" : "Remote snapshot not found",
      );
    });

  const handleDownload = () =>
    runAction("download", async () => {
      const result = await settingsApi.downloadWebDavSnapshot(settings);
      setPreview(result.preview ?? null);
      toast.success(result.message || "Snapshot downloaded");
    });

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
          Manual snapshot upload, preview, and download. Automatic conflict
          merging is not enabled in this release.
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
      </div>

      <div className="flex flex-wrap gap-2">
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
          onClick={handleDownload}
          disabled={!settings.enabled || busyAction !== null}
        >
          <Download className="mr-2 h-4 w-4" />
          {busyAction === "download" ? "Downloading..." : "Download"}
        </Button>
      </div>

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
    </section>
  );
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
