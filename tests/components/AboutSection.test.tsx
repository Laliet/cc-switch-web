import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AboutSection } from "@/components/settings/AboutSection";

const toastSuccessMock = vi.hoisted(() => vi.fn());
const toastErrorMock = vi.hoisted(() => vi.fn());
const getCurrentVersionMock = vi.hoisted(() => vi.fn());
const relaunchAppMock = vi.hoisted(() => vi.fn());
const openExternalMock = vi.hoisted(() => vi.fn());
const checkUpdatesMock = vi.hoisted(() => vi.fn());
const useUpdateMock = vi.hoisted(() => vi.fn());

const tMock = vi.fn((key: string, options?: Record<string, unknown>) => {
  if (options && typeof options === "object" && "version" in options) {
    return `${key}:${String(options.version ?? "")}`;
  }
  return key;
});

vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => toastSuccessMock(...args),
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

vi.mock("@/lib/query", () => ({
  useCapabilitiesQuery: () => ({
    data: { features: { appUpdate: true } },
  }),
}));

vi.mock("@/lib/updater", () => ({
  getCurrentVersion: (...args: unknown[]) => getCurrentVersionMock(...args),
  relaunchApp: (...args: unknown[]) => relaunchAppMock(...args),
}));

vi.mock("@/lib/api", () => ({
  settingsApi: {
    openExternal: (...args: unknown[]) => openExternalMock(...args),
    checkUpdates: (...args: unknown[]) => checkUpdatesMock(...args),
  },
}));

vi.mock("@/contexts/UpdateContext", () => ({
  useUpdate: () => useUpdateMock(),
}));

interface UpdateState {
  hasUpdate: boolean;
  updateInfo: {
    currentVersion: string;
    availableVersion: string;
    notes?: string;
  } | null;
  updateHandle: {
    downloadAndInstall: () => Promise<void>;
  } | null;
  checkUpdate: ReturnType<typeof vi.fn>;
  resetDismiss: ReturnType<typeof vi.fn>;
  isChecking: boolean;
}

const createUpdateState = (overrides: Partial<UpdateState> = {}) => ({
  hasUpdate: false,
  updateInfo: null,
  updateHandle: null,
  checkUpdate: vi.fn().mockResolvedValue("up-to-date"),
  resetDismiss: vi.fn(),
  isChecking: false,
  ...overrides,
});

describe("AboutSection", () => {
  beforeEach(() => {
    tMock.mockClear();
    toastSuccessMock.mockClear();
    toastErrorMock.mockClear();
    getCurrentVersionMock.mockReset();
    relaunchAppMock.mockReset();
    openExternalMock.mockReset();
    checkUpdatesMock.mockReset();
    useUpdateMock.mockReset();

    getCurrentVersionMock.mockResolvedValue("1.0.0");
    useUpdateMock.mockReturnValue(createUpdateState());
  });

  it("loads and displays version", async () => {
    getCurrentVersionMock.mockResolvedValueOnce("1.2.3");
    const { container } = render(<AboutSection isPortable={false} />);

    expect(container.querySelector("svg.animate-spin")).toBeInTheDocument();

    await waitFor(() =>
      expect(screen.getByText(/v1\.2\.3/)).toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(
        container.querySelector("svg.animate-spin"),
      ).not.toBeInTheDocument(),
    );
    expect(getCurrentVersionMock).toHaveBeenCalledTimes(1);
  });

  it("shows portable mode hint and uses portable update flow", async () => {
    const updateHandle = { downloadAndInstall: vi.fn() };
    useUpdateMock.mockReturnValue(
      createUpdateState({
        hasUpdate: true,
        updateInfo: {
          currentVersion: "1.0.0",
          availableVersion: "2.0.0",
        },
        updateHandle,
      }),
    );

    const user = userEvent.setup();
    render(<AboutSection isPortable />);

    expect(screen.getByText("settings.portableMode")).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "settings.updateTo:2.0.0" }),
    );

    await waitFor(() => expect(checkUpdatesMock).toHaveBeenCalledTimes(1));
    expect(updateHandle.downloadAndInstall).not.toHaveBeenCalled();
  });

  it("triggers checkUpdate when clicking update button", async () => {
    const checkUpdate = vi.fn().mockResolvedValue("up-to-date");
    useUpdateMock.mockReturnValue(createUpdateState({ checkUpdate }));

    const user = userEvent.setup();
    render(<AboutSection isPortable={false} />);

    await user.click(
      screen.getByRole("button", { name: "settings.checkForUpdates" }),
    );

    await waitFor(() => expect(checkUpdate).toHaveBeenCalledTimes(1));
    expect(toastSuccessMock).toHaveBeenCalledWith("settings.upToDate");
  });

  it("shows update button and update info when update available", async () => {
    useUpdateMock.mockReturnValue(
      createUpdateState({
        hasUpdate: true,
        updateInfo: {
          currentVersion: "1.0.0",
          availableVersion: "2.0.0",
          notes: "Fixes and improvements",
        },
      }),
    );

    render(<AboutSection isPortable={false} />);

    await waitFor(() =>
      expect(screen.getByText(/v1\.0\.0/)).toBeInTheDocument(),
    );

    expect(
      screen.getByRole("button", { name: "settings.updateTo:2.0.0" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("settings.updateAvailable:2.0.0"),
    ).toBeInTheDocument();
    expect(screen.getByText("Fixes and improvements")).toBeInTheDocument();
  });

  it("downloads update and relaunches app", async () => {
    const user = userEvent.setup();
    let resolveDownload: (() => void) | undefined;
    const downloadAndInstall = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveDownload = resolve;
        }),
    );
    const resetDismiss = vi.fn();
    const updateHandle = { downloadAndInstall };

    useUpdateMock.mockReturnValue(
      createUpdateState({
        hasUpdate: true,
        updateInfo: {
          currentVersion: "1.0.0",
          availableVersion: "2.0.0",
        },
        updateHandle,
        resetDismiss,
      }),
    );

    render(<AboutSection isPortable={false} />);

    await user.click(
      screen.getByRole("button", { name: "settings.updateTo:2.0.0" }),
    );

    await waitFor(() => expect(resetDismiss).toHaveBeenCalledTimes(1));
    expect(downloadAndInstall).toHaveBeenCalledTimes(1);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "settings.updating" }),
      ).toBeInTheDocument(),
    );

    resolveDownload?.();

    await waitFor(() => expect(relaunchAppMock).toHaveBeenCalledTimes(1));
  });

  it("opens release notes link", async () => {
    const user = userEvent.setup();
    useUpdateMock.mockReturnValue(
      createUpdateState({
        hasUpdate: true,
        updateInfo: {
          currentVersion: "1.0.0",
          availableVersion: "2.1.0",
        },
      }),
    );

    render(<AboutSection isPortable={false} />);

    await user.click(
      screen.getByRole("button", { name: "settings.releaseNotes" }),
    );

    await waitFor(() =>
      expect(openExternalMock).toHaveBeenCalledWith(
        "https://github.com/Laliet/cc-switch-web/releases/tag/v2.1.0",
      ),
    );
  });
});
