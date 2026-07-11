import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WebDavSettingsSection } from "@/components/settings/WebDavSettingsSection";
import type { WebDavSettings } from "@/types";

const settingsApiMocks = vi.hoisted(() => ({
  downloadWebDavSnapshot: vi.fn(),
  listWebDavBackups: vi.fn(),
  previewWebDavSnapshot: vi.fn(),
  restoreWebDavBackup: vi.fn(),
  syncWebDavSnapshot: vi.fn(),
  uploadWebDavSnapshot: vi.fn(),
}));

const toastMocks = vi.hoisted(() => ({
  error: vi.fn(),
  success: vi.fn(),
  warning: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  settingsApi: settingsApiMocks,
}));

vi.mock("sonner", () => ({
  toast: toastMocks,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock("@/components/ui/dialog", () => ({
  Dialog: ({ open, children }: { open: boolean; children: React.ReactNode }) =>
    open ? <div>{children}</div> : null,
  DialogContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DialogDescription: ({ children }: { children: React.ReactNode }) => (
    <p>{children}</p>
  ),
  DialogFooter: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DialogHeader: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DialogTitle: ({ children }: { children: React.ReactNode }) => (
    <h2>{children}</h2>
  ),
}));

const settings: WebDavSettings = {
  enabled: true,
  autoSync: false,
  baseUrl: "https://dav.example.com",
  username: "me",
  password: "secret",
  remoteDir: "cc-switch-web",
  profile: "default",
  lastSyncStatus: "idle",
};

describe("WebDavSettingsSection", () => {
  beforeEach(() => {
    for (const mock of Object.values(settingsApiMocks)) {
      mock.mockReset();
    }
    toastMocks.error.mockReset();
    toastMocks.success.mockReset();
  });

  it("confirms before downloading and displays the local backup id", async () => {
    settingsApiMocks.downloadWebDavSnapshot.mockResolvedValueOnce({
      success: true,
      message: "Snapshot downloaded",
      remotePath: "https://dav.example.com/cc-switch-web/default.json",
      backupId: "backup-123",
      preview: {
        exists: true,
        remotePath: "https://dav.example.com/cc-switch-web/default.json",
        sizeBytes: 512,
        modifiedAt: "2026-06-01T00:00:00Z",
        artifactList: ["providers", "settings"],
        configVersion: 1,
        schemaVersion: 4,
        compatible: true,
        checks: [],
      },
    });

    render(<WebDavSettingsSection value={settings} onChange={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Download" }));
    expect(settingsApiMocks.downloadWebDavSnapshot).not.toHaveBeenCalled();

    fireEvent.click(screen.getAllByRole("button", { name: "Download" })[1]);

    await waitFor(() =>
      expect(settingsApiMocks.downloadWebDavSnapshot).toHaveBeenCalledWith(
        settings,
      ),
    );
    expect(screen.getByText("Backup ID: backup-123")).toBeInTheDocument();
    expect(screen.getByText("Config version: 1")).toBeInTheDocument();
    expect(screen.getByText("Schema version: 4")).toBeInTheDocument();
  });

  it("shows friendly authentication errors", async () => {
    settingsApiMocks.previewWebDavSnapshot.mockRejectedValueOnce(
      new Error("WebDAV request failed: 401 Unauthorized"),
    );

    render(<WebDavSettingsSection value={settings} onChange={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Preview" }));

    await waitFor(() =>
      expect(toastMocks.error).toHaveBeenCalledWith(
        "WebDAV authentication failed. Check username, password, and server permissions.",
      ),
    );
  });
});
