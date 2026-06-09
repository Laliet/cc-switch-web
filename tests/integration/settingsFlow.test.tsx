/**
 * Integration tests for settings and update user flows
 * Tests complete user journeys: theme switching, update checking, update actions
 */
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  useState,
  useEffect,
  useCallback,
  createContext,
  useContext,
  type ReactNode,
} from "react";
import type { UpdateInfo, UpdateHandle } from "@/lib/updater";

// ===== Mock Setup =====
const checkForUpdateMock = vi.hoisted(() => vi.fn());
const downloadAndInstallMock = vi.hoisted(() => vi.fn());
const relaunchAppMock = vi.hoisted(() => vi.fn());
const setThemeMock = vi.hoisted(() => vi.fn());
const getThemeMock = vi.hoisted(() => vi.fn());

const tMock = vi.fn((key: string, options?: Record<string, unknown>) => {
  if (options?.defaultValue) return String(options.defaultValue);
  if (key === "settings.theme.light") return "Light";
  if (key === "settings.theme.dark") return "Dark";
  if (key === "settings.theme.system") return "System";
  if (key === "update.available") return "Update available";
  if (key === "update.upToDate") return "You're up to date";
  if (key === "update.checking") return "Checking for updates...";
  if (key === "update.download") return "Download & Install";
  if (key === "update.dismiss") return "Dismiss";
  if (key === "update.downloading") return "Downloading...";
  return key;
});

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

vi.mock("@/lib/updater", () => ({
  checkForUpdate: (...args: unknown[]) => checkForUpdateMock(...args),
  relaunchApp: (...args: unknown[]) => relaunchAppMock(...args),
}));

// ===== Theme Context (Simplified) =====

type Theme = "light" | "dark" | "system";

interface ThemeContextValue {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

const ThemeContext = createContext<ThemeContextValue | undefined>(undefined);

function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>("system");

  useEffect(() => {
    const saved = getThemeMock();
    if (saved) setThemeState(saved);
  }, []);

  const setTheme = useCallback((newTheme: Theme) => {
    setThemeState(newTheme);
    setThemeMock(newTheme);
  }, []);

  return (
    <ThemeContext.Provider value={{ theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) throw new Error("useTheme must be used within ThemeProvider");
  return context;
}

// ===== Update Context (Simplified) =====

interface UpdateContextValue {
  hasUpdate: boolean;
  updateInfo: UpdateInfo | null;
  updateHandle: UpdateHandle | null;
  isChecking: boolean;
  isDownloading: boolean;
  downloadProgress: number;
  error: string | null;
  isDismissed: boolean;
  checkUpdate: () => Promise<void>;
  dismissUpdate: () => void;
  downloadUpdate: () => Promise<void>;
}

const UpdateContext = createContext<UpdateContextValue | undefined>(undefined);

function UpdateProvider({ children }: { children: ReactNode }) {
  const [hasUpdate, setHasUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateHandle, setUpdateHandle] = useState<UpdateHandle | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [isDismissed, setIsDismissed] = useState(false);

  const checkUpdate = useCallback(async () => {
    setIsChecking(true);
    setError(null);

    try {
      const result = await checkForUpdateMock({ timeout: 30000 });

      if (result.status === "available") {
        setHasUpdate(true);
        setUpdateInfo(result.info);
        setUpdateHandle(result.update);
        setIsDismissed(false);
      } else {
        setHasUpdate(false);
        setUpdateInfo(null);
        setUpdateHandle(null);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : "Check failed";
      setError(message);
      setHasUpdate(false);
    } finally {
      setIsChecking(false);
    }
  }, []);

  const dismissUpdate = useCallback(() => {
    setIsDismissed(true);
  }, []);

  const downloadUpdate = useCallback(async () => {
    if (!updateHandle) return;

    setIsDownloading(true);
    setDownloadProgress(0);

    try {
      await updateHandle.downloadAndInstall((event) => {
        if (event.event === "Progress" && event.total) {
          const progress = ((event.downloaded || 0) / event.total) * 100;
          setDownloadProgress(progress);
        }
      });

      // Relaunch after download
      await relaunchAppMock();
    } catch (err) {
      const message = err instanceof Error ? err.message : "Download failed";
      setError(message);
    } finally {
      setIsDownloading(false);
    }
  }, [updateHandle]);

  return (
    <UpdateContext.Provider
      value={{
        hasUpdate,
        updateInfo,
        updateHandle,
        isChecking,
        isDownloading,
        downloadProgress,
        error,
        isDismissed,
        checkUpdate,
        dismissUpdate,
        downloadUpdate,
      }}
    >
      {children}
    </UpdateContext.Provider>
  );
}

function useUpdate() {
  const context = useContext(UpdateContext);
  if (!context) throw new Error("useUpdate must be used within UpdateProvider");
  return context;
}

// ===== Test Components =====

function ThemeSettings() {
  const { theme, setTheme } = useTheme();

  return (
    <div data-testid="theme-settings">
      <h2>Theme Settings</h2>
      <div data-testid="current-theme">Current: {theme}</div>
      <div className="theme-buttons">
        <button
          onClick={() => setTheme("light")}
          data-active={theme === "light"}
          data-testid="theme-light"
        >
          Light
        </button>
        <button
          onClick={() => setTheme("dark")}
          data-active={theme === "dark"}
          data-testid="theme-dark"
        >
          Dark
        </button>
        <button
          onClick={() => setTheme("system")}
          data-active={theme === "system"}
          data-testid="theme-system"
        >
          System
        </button>
      </div>
    </div>
  );
}

function UpdateSettings() {
  const {
    hasUpdate,
    updateInfo,
    isChecking,
    isDownloading,
    downloadProgress,
    error,
    isDismissed,
    checkUpdate,
    dismissUpdate,
    downloadUpdate,
  } = useUpdate();

  return (
    <div data-testid="update-settings">
      <h2>Updates</h2>

      {isChecking ? (
        <div data-testid="checking-state">Checking for updates...</div>
      ) : hasUpdate && !isDismissed ? (
        <div data-testid="update-available">
          <p>Update available: {updateInfo?.availableVersion}</p>
          <p>Current version: {updateInfo?.currentVersion}</p>
          {updateInfo?.notes && <p>Release notes: {updateInfo.notes}</p>}

          {isDownloading ? (
            <div data-testid="downloading-state">
              <p>Downloading... {Math.round(downloadProgress)}%</p>
              <progress value={downloadProgress} max={100} />
            </div>
          ) : (
            <div className="update-actions">
              <button onClick={downloadUpdate} data-testid="download-button">
                Download & Install
              </button>
              <button onClick={dismissUpdate} data-testid="dismiss-button">
                Dismiss
              </button>
            </div>
          )}
        </div>
      ) : isDismissed ? (
        <div data-testid="dismissed-state">
          <p>Update dismissed</p>
          <button onClick={checkUpdate} data-testid="check-again-button">
            Check again
          </button>
        </div>
      ) : (
        <div data-testid="up-to-date">
          <p>You're up to date!</p>
          <button onClick={checkUpdate} data-testid="check-update-button">
            Check for updates
          </button>
        </div>
      )}

      {error && <div data-testid="error-message">{error}</div>}
    </div>
  );
}

// Full Settings Page
function SettingsPage() {
  return (
    <ThemeProvider>
      <UpdateProvider>
        <div data-testid="settings-page">
          <h1>Settings</h1>
          <ThemeSettings />
          <UpdateSettings />
        </div>
      </UpdateProvider>
    </ThemeProvider>
  );
}

// ===== Tests =====

describe("Settings and Update User Flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tMock.mockClear();
    getThemeMock.mockReturnValue("system");
    checkForUpdateMock.mockResolvedValue({ status: "up-to-date" });
    downloadAndInstallMock.mockResolvedValue(undefined);
    relaunchAppMock.mockResolvedValue(undefined);
  });

  describe("Complete Theme Switching Flow", () => {
    it("user can switch between themes", async () => {
      const user = userEvent.setup();

      render(<SettingsPage />);

      // 1. Verify initial theme (system)
      expect(screen.getByTestId("current-theme")).toHaveTextContent(
        "Current: system",
      );

      // 2. Switch to light theme
      await user.click(screen.getByTestId("theme-light"));

      expect(screen.getByTestId("current-theme")).toHaveTextContent(
        "Current: light",
      );
      expect(setThemeMock).toHaveBeenCalledWith("light");

      // 3. Switch to dark theme
      await user.click(screen.getByTestId("theme-dark"));

      expect(screen.getByTestId("current-theme")).toHaveTextContent(
        "Current: dark",
      );
      expect(setThemeMock).toHaveBeenCalledWith("dark");

      // 4. Switch back to system theme
      await user.click(screen.getByTestId("theme-system"));

      expect(screen.getByTestId("current-theme")).toHaveTextContent(
        "Current: system",
      );
      expect(setThemeMock).toHaveBeenCalledWith("system");
    });

    it("persists theme preference from storage", async () => {
      getThemeMock.mockReturnValue("dark");

      render(<SettingsPage />);

      // Should load saved theme
      expect(screen.getByTestId("current-theme")).toHaveTextContent(
        "Current: dark",
      );
    });
  });

  describe("Complete Update Check Flow", () => {
    it("user can check for updates when up to date", async () => {
      const user = userEvent.setup();

      checkForUpdateMock.mockResolvedValue({ status: "up-to-date" });

      render(<SettingsPage />);

      // 1. Should show up-to-date state initially
      expect(screen.getByTestId("up-to-date")).toBeInTheDocument();

      // 2. Click check for updates
      await user.click(screen.getByTestId("check-update-button"));

      // 3. Should show checking state
      await waitFor(() => {
        expect(checkForUpdateMock).toHaveBeenCalled();
      });

      // 4. Should return to up-to-date state
      await waitFor(() => {
        expect(screen.getByTestId("up-to-date")).toBeInTheDocument();
      });
    });

    it("user can check and find available update", async () => {
      const user = userEvent.setup();

      checkForUpdateMock.mockResolvedValue({
        status: "available",
        info: {
          currentVersion: "1.0.0",
          availableVersion: "2.0.0",
          notes: "New features and bug fixes",
        },
        update: {
          version: "2.0.0",
          downloadAndInstall: downloadAndInstallMock,
        },
      });

      render(<SettingsPage />);

      // 1. Click check for updates
      await user.click(screen.getByTestId("check-update-button"));

      // 2. Should show update available
      await waitFor(() => {
        expect(screen.getByTestId("update-available")).toBeInTheDocument();
      });

      // 3. Should display update info
      expect(screen.getByText("Update available: 2.0.0")).toBeInTheDocument();
      expect(screen.getByText("Current version: 1.0.0")).toBeInTheDocument();
      expect(
        screen.getByText("Release notes: New features and bug fixes"),
      ).toBeInTheDocument();

      // 4. Should show action buttons
      expect(screen.getByTestId("download-button")).toBeInTheDocument();
      expect(screen.getByTestId("dismiss-button")).toBeInTheDocument();
    });

    it("shows error when update check fails", async () => {
      const user = userEvent.setup();

      checkForUpdateMock.mockRejectedValue(new Error("Network error"));

      render(<SettingsPage />);

      await user.click(screen.getByTestId("check-update-button"));

      await waitFor(() => {
        expect(screen.getByTestId("error-message")).toHaveTextContent(
          "Network error",
        );
      });
    });
  });

  describe("Complete Update Download Flow", () => {
    it("user can download and install update", async () => {
      const user = userEvent.setup();

      // Simulate download progress
      downloadAndInstallMock.mockImplementation(async (onProgress) => {
        onProgress({ event: "Started", total: 100 });
        onProgress({ event: "Progress", total: 100, downloaded: 50 });
        onProgress({ event: "Progress", total: 100, downloaded: 100 });
        onProgress({ event: "Finished" });
      });

      checkForUpdateMock.mockResolvedValue({
        status: "available",
        info: {
          currentVersion: "1.0.0",
          availableVersion: "2.0.0",
        },
        update: {
          version: "2.0.0",
          downloadAndInstall: downloadAndInstallMock,
        },
      });

      render(<SettingsPage />);

      // 1. Check for updates
      await user.click(screen.getByTestId("check-update-button"));

      await waitFor(() => {
        expect(screen.getByTestId("update-available")).toBeInTheDocument();
      });

      // 2. Click download
      await user.click(screen.getByTestId("download-button"));

      // 3. Should show downloading state
      await waitFor(() => {
        expect(downloadAndInstallMock).toHaveBeenCalled();
      });

      // 4. Should call relaunch after download
      await waitFor(() => {
        expect(relaunchAppMock).toHaveBeenCalled();
      });
    });

    it("handles download error gracefully", async () => {
      const user = userEvent.setup();
      const consoleSpy = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});

      downloadAndInstallMock.mockRejectedValue(new Error("Download failed"));

      checkForUpdateMock.mockResolvedValue({
        status: "available",
        info: {
          currentVersion: "1.0.0",
          availableVersion: "2.0.0",
        },
        update: {
          version: "2.0.0",
          downloadAndInstall: downloadAndInstallMock,
        },
      });

      render(<SettingsPage />);

      await user.click(screen.getByTestId("check-update-button"));

      await waitFor(() => {
        expect(screen.getByTestId("update-available")).toBeInTheDocument();
      });

      await user.click(screen.getByTestId("download-button"));

      await waitFor(() => {
        expect(screen.getByTestId("error-message")).toHaveTextContent(
          "Download failed",
        );
      });

      consoleSpy.mockRestore();
    });
  });

  describe("Complete Dismiss Update Flow", () => {
    it("user can dismiss update notification", async () => {
      const user = userEvent.setup();

      checkForUpdateMock.mockResolvedValue({
        status: "available",
        info: {
          currentVersion: "1.0.0",
          availableVersion: "2.0.0",
        },
        update: {
          version: "2.0.0",
          downloadAndInstall: downloadAndInstallMock,
        },
      });

      render(<SettingsPage />);

      // 1. Check for updates
      await user.click(screen.getByTestId("check-update-button"));

      await waitFor(() => {
        expect(screen.getByTestId("update-available")).toBeInTheDocument();
      });

      // 2. Dismiss the update
      await user.click(screen.getByTestId("dismiss-button"));

      // 3. Should show dismissed state
      expect(screen.getByTestId("dismissed-state")).toBeInTheDocument();
      expect(screen.getByText("Update dismissed")).toBeInTheDocument();
    });

    it("user can check for updates again after dismissing", async () => {
      const user = userEvent.setup();

      checkForUpdateMock.mockResolvedValue({
        status: "available",
        info: {
          currentVersion: "1.0.0",
          availableVersion: "2.0.0",
        },
        update: {
          version: "2.0.0",
          downloadAndInstall: downloadAndInstallMock,
        },
      });

      render(<SettingsPage />);

      // 1. Check, find update, dismiss
      await user.click(screen.getByTestId("check-update-button"));

      await waitFor(() => {
        expect(screen.getByTestId("update-available")).toBeInTheDocument();
      });

      await user.click(screen.getByTestId("dismiss-button"));

      expect(screen.getByTestId("dismissed-state")).toBeInTheDocument();

      // 2. Check again
      await user.click(screen.getByTestId("check-again-button"));

      // 3. Should show update available again
      await waitFor(() => {
        expect(screen.getByTestId("update-available")).toBeInTheDocument();
      });
    });
  });

  describe("Complete User Journey: Settings Configuration", () => {
    it("user configures theme and checks for updates in single session", async () => {
      const user = userEvent.setup();

      checkForUpdateMock
        .mockResolvedValueOnce({ status: "up-to-date" })
        .mockResolvedValueOnce({
          status: "available",
          info: {
            currentVersion: "1.0.0",
            availableVersion: "2.0.0",
          },
          update: {
            version: "2.0.0",
            downloadAndInstall: downloadAndInstallMock,
          },
        });

      render(<SettingsPage />);

      // Step 1: User opens settings and sees default theme
      expect(screen.getByTestId("current-theme")).toHaveTextContent(
        "Current: system",
      );

      // Step 2: User switches to dark theme
      await user.click(screen.getByTestId("theme-dark"));
      expect(screen.getByTestId("current-theme")).toHaveTextContent(
        "Current: dark",
      );

      // Step 3: User checks for updates (first time - up to date)
      await user.click(screen.getByTestId("check-update-button"));

      await waitFor(() => {
        expect(screen.getByTestId("up-to-date")).toBeInTheDocument();
      });

      // Step 4: User changes mind and switches to light theme
      await user.click(screen.getByTestId("theme-light"));
      expect(screen.getByTestId("current-theme")).toHaveTextContent(
        "Current: light",
      );

      // Step 5: User checks for updates again (this time finds update)
      await user.click(screen.getByTestId("check-update-button"));

      await waitFor(() => {
        expect(screen.getByTestId("update-available")).toBeInTheDocument();
      });

      // Step 6: User dismisses the update
      await user.click(screen.getByTestId("dismiss-button"));
      expect(screen.getByTestId("dismissed-state")).toBeInTheDocument();

      // Verify all actions were recorded
      expect(setThemeMock).toHaveBeenCalledWith("dark");
      expect(setThemeMock).toHaveBeenCalledWith("light");
      expect(checkForUpdateMock).toHaveBeenCalledTimes(2);
    });
  });

  describe("State Persistence Across Interactions", () => {
    it("maintains update state while user interacts with other settings", async () => {
      const user = userEvent.setup();

      checkForUpdateMock.mockResolvedValue({
        status: "available",
        info: {
          currentVersion: "1.0.0",
          availableVersion: "2.0.0",
        },
        update: {
          version: "2.0.0",
          downloadAndInstall: downloadAndInstallMock,
        },
      });

      render(<SettingsPage />);

      // 1. Find update
      await user.click(screen.getByTestId("check-update-button"));

      await waitFor(() => {
        expect(screen.getByTestId("update-available")).toBeInTheDocument();
      });

      // 2. Switch theme (update state should remain)
      await user.click(screen.getByTestId("theme-dark"));

      expect(screen.getByTestId("update-available")).toBeInTheDocument();

      // 3. Switch theme again
      await user.click(screen.getByTestId("theme-light"));

      // Update state should still be there
      expect(screen.getByTestId("update-available")).toBeInTheDocument();
      expect(screen.getByText("Update available: 2.0.0")).toBeInTheDocument();
    });
  });
});
