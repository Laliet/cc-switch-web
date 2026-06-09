import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthCenterSection } from "@/components/settings/AuthCenterSection";

const authApiMock = vi.hoisted(() => ({
  listAccounts: vi.fn(),
  importAccount: vi.fn(),
  setDefault: vi.fn(),
  deleteAccount: vi.fn(),
  logout: vi.fn(),
  startDeviceLogin: vi.fn(),
  pollDeviceLogin: vi.fn(),
  queryUsage: vi.fn(),
}));

const settingsApiMock = vi.hoisted(() => ({
  openExternal: vi.fn(),
}));

const toastMock = vi.hoisted(() => ({
  error: vi.fn(),
  info: vi.fn(),
  success: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  authApi: authApiMock,
  settingsApi: settingsApiMock,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      let value = String(options?.defaultValue ?? key);
      for (const [optionKey, optionValue] of Object.entries(options ?? {})) {
        value = value.replace(
          new RegExp(`{{${optionKey}}}`, "g"),
          String(optionValue),
        );
      }
      return value;
    },
  }),
}));

vi.mock("sonner", () => ({
  toast: toastMock,
}));

describe("AuthCenterSection", () => {
  beforeEach(() => {
    authApiMock.listAccounts.mockReset();
    authApiMock.importAccount.mockReset();
    authApiMock.setDefault.mockReset();
    authApiMock.deleteAccount.mockReset();
    authApiMock.logout.mockReset();
    authApiMock.startDeviceLogin.mockReset();
    authApiMock.pollDeviceLogin.mockReset();
    authApiMock.queryUsage.mockReset();
    settingsApiMock.openExternal.mockReset();
    toastMock.error.mockReset();
    toastMock.info.mockReset();
    toastMock.success.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("starts device login, opens verification URL, polls, and refreshes accounts", async () => {
    authApiMock.listAccounts.mockResolvedValueOnce([]).mockResolvedValueOnce([
      {
        id: "gh-1",
        provider: "github_copilot",
        label: "GitHub One",
        username: "octo",
        isDefault: true,
        createdAt: "2026-06-08T00:00:00Z",
        updatedAt: "2026-06-08T00:00:00Z",
      },
    ]);
    authApiMock.startDeviceLogin.mockResolvedValueOnce({
      provider: "github_copilot",
      sessionId: "session-1",
      userCode: "ABCD-1234",
      verificationUri: "https://github.com/login/device",
      verificationUriComplete:
        "https://github.com/login/device?user_code=ABCD-1234",
      intervalSeconds: 1,
      expiresAt: new Date(Date.now() + 60_000).toISOString(),
    });
    authApiMock.pollDeviceLogin.mockResolvedValueOnce({
      status: "authorized",
    });
    settingsApiMock.openExternal.mockResolvedValue(undefined);

    render(<AuthCenterSection />);

    await screen.findAllByText("No accounts yet.");
    const deviceButton = screen.getAllByRole("button", { name: /Device/i })[0];
    vi.useFakeTimers();
    fireEvent.click(deviceButton);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(settingsApiMock.openExternal).toHaveBeenCalledWith(
      "https://github.com/login/device?user_code=ABCD-1234",
    );
    expect(screen.getByText("Device code: ABCD-1234")).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(authApiMock.pollDeviceLogin).toHaveBeenCalledWith({
      provider: "github_copilot",
      sessionId: "session-1",
    });
    expect(screen.getByText("GitHub One")).toBeInTheDocument();
    expect(toastMock.success).toHaveBeenCalledWith(
      "GitHub Copilot account connected.",
    );
    vi.useRealTimers();
  });

  it("stops device login polling after unmount", async () => {
    authApiMock.listAccounts.mockResolvedValue([]);
    authApiMock.startDeviceLogin.mockResolvedValueOnce({
      provider: "github_copilot",
      sessionId: "session-1",
      userCode: "ABCD-1234",
      verificationUri: "https://github.com/login/device",
      verificationUriComplete: null,
      intervalSeconds: 1,
      expiresAt: new Date(Date.now() + 60_000).toISOString(),
    });
    settingsApiMock.openExternal.mockResolvedValue(undefined);

    const { unmount } = render(<AuthCenterSection />);

    await screen.findAllByText("No accounts yet.");
    vi.useFakeTimers();
    fireEvent.click(screen.getAllByRole("button", { name: /Device/i })[0]);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    unmount();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000);
      await Promise.resolve();
    });

    expect(authApiMock.pollDeviceLogin).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("does not open or poll when device login resolves after unmount", async () => {
    authApiMock.listAccounts.mockResolvedValue([]);
    type DeviceSession = Awaited<
      ReturnType<typeof authApiMock.startDeviceLogin>
    >;
    let resolveStart: ((session: DeviceSession) => void) | undefined;
    authApiMock.startDeviceLogin.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveStart = resolve;
        }),
    );

    const { unmount } = render(<AuthCenterSection />);

    await screen.findAllByText("No accounts yet.");
    fireEvent.click(screen.getAllByRole("button", { name: /Device/i })[0]);
    unmount();
    await act(async () => {
      resolveStart?.({
        provider: "github_copilot",
        sessionId: "session-1",
        userCode: "ABCD-1234",
        verificationUri: "https://github.com/login/device",
        verificationUriComplete: null,
        intervalSeconds: 1,
        expiresAt: new Date(Date.now() + 60_000).toISOString(),
      });
      await Promise.resolve();
    });

    expect(settingsApiMock.openExternal).not.toHaveBeenCalled();
    expect(authApiMock.pollDeviceLogin).not.toHaveBeenCalled();
    expect(toastMock.info).not.toHaveBeenCalled();
  });

  it("disables competing account actions while device login is active", async () => {
    authApiMock.listAccounts.mockResolvedValue([
      {
        id: "gh-1",
        provider: "github_copilot",
        label: "GitHub One",
        username: "octo",
        isDefault: false,
        createdAt: "2026-06-08T00:00:00Z",
        updatedAt: "2026-06-08T00:00:00Z",
      },
    ]);
    authApiMock.startDeviceLogin.mockResolvedValueOnce({
      provider: "github_copilot",
      sessionId: "session-1",
      userCode: "ABCD-1234",
      verificationUri: "https://github.com/login/device",
      verificationUriComplete: null,
      intervalSeconds: 30,
      expiresAt: new Date(Date.now() + 60_000).toISOString(),
    });
    settingsApiMock.openExternal.mockResolvedValue(undefined);

    const { unmount } = render(<AuthCenterSection />);

    expect(await screen.findByText("GitHub One")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getAllByRole("button", { name: /Device/i })[0]);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(screen.getByTitle("Query usage")).toBeDisabled();
    expect(screen.getByTitle("Set default")).toBeDisabled();
    expect(screen.getByTitle("Logout")).toBeDisabled();
    expect(screen.getByTitle("Delete")).toBeDisabled();
    expect(
      screen.getByRole("button", { name: /Import Token/i }),
    ).toBeDisabled();
    unmount();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(30_000);
      await Promise.resolve();
    });
    vi.useRealTimers();
  });

  it("imports an existing token and refreshes the account list", async () => {
    authApiMock.listAccounts.mockResolvedValueOnce([]).mockResolvedValueOnce([
      {
        id: "manual-1",
        provider: "github_copilot",
        label: "Manual",
        username: null,
        isDefault: true,
        createdAt: "2026-06-08T00:00:00Z",
        updatedAt: "2026-06-08T00:00:00Z",
      },
    ]);
    authApiMock.importAccount.mockResolvedValueOnce({
      id: "manual-1",
      provider: "github_copilot",
      label: "Manual",
      isDefault: true,
      createdAt: "2026-06-08T00:00:00Z",
      updatedAt: "2026-06-08T00:00:00Z",
    });

    render(<AuthCenterSection />);

    fireEvent.change(await screen.findByLabelText("Label"), {
      target: { value: "Manual" },
    });
    fireEvent.change(screen.getByLabelText("Account ID"), {
      target: { value: "manual-1" },
    });
    fireEvent.change(screen.getByLabelText("Access Token"), {
      target: { value: "token-1" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Import Token/i }));

    await waitFor(() => {
      expect(authApiMock.importAccount).toHaveBeenCalledWith(
        expect.objectContaining({
          provider: "github_copilot",
          id: "manual-1",
          label: "Manual",
          makeDefault: true,
          tokens: expect.objectContaining({
            accessToken: "token-1",
            tokenType: "Bearer",
          }),
        }),
      );
    });
    expect(await screen.findByText("Manual")).toBeInTheDocument();
  });

  it("sets default, queries usage, logs out, and deletes accounts", async () => {
    const primaryAccount = {
      id: "gh-1",
      provider: "github_copilot" as const,
      label: "Primary",
      username: "octo",
      isDefault: false,
      createdAt: "2026-06-08T00:00:00Z",
      updatedAt: "2026-06-08T00:00:00Z",
    };
    const defaultAccount = {
      id: "gh-2",
      provider: "github_copilot" as const,
      label: "Default",
      username: "hub",
      isDefault: true,
      createdAt: "2026-06-08T00:00:00Z",
      updatedAt: "2026-06-08T00:00:00Z",
    };
    authApiMock.listAccounts.mockResolvedValue([
      primaryAccount,
      defaultAccount,
    ]);
    authApiMock.setDefault.mockResolvedValue(true);
    authApiMock.queryUsage.mockResolvedValue({
      provider: "github_copilot",
      accountId: "gh-1",
      remaining: 0,
      total: 20,
    });
    authApiMock.logout.mockResolvedValue(true);
    authApiMock.deleteAccount.mockResolvedValue(true);

    render(<AuthCenterSection />);

    expect(await screen.findByText("Primary")).toBeInTheDocument();

    fireEvent.click(screen.getAllByTitle("Set default")[0]);
    await waitFor(() => {
      expect(authApiMock.setDefault).toHaveBeenCalledWith(
        "github_copilot",
        "gh-1",
      );
    });

    fireEvent.click(screen.getAllByTitle("Query usage")[0]);
    await waitFor(() => {
      expect(authApiMock.queryUsage).toHaveBeenCalledWith(
        "github_copilot",
        "gh-1",
      );
    });
    expect(await screen.findByText("0 / 20 remaining")).toBeInTheDocument();

    fireEvent.click(screen.getAllByTitle("Logout")[0]);
    await waitFor(() => {
      expect(authApiMock.logout).toHaveBeenCalledWith("github_copilot", "gh-1");
    });

    fireEvent.click(screen.getAllByTitle("Delete")[0]);
    await waitFor(() => {
      expect(authApiMock.deleteAccount).toHaveBeenCalledWith(
        "github_copilot",
        "gh-1",
      );
    });
  });

  it("exposes usage query for Codex OAuth accounts", async () => {
    authApiMock.listAccounts.mockResolvedValue([
      {
        id: "codex-1",
        provider: "codex_oauth",
        label: "Codex",
        username: "user@example.com",
        isDefault: true,
        createdAt: "2026-06-08T00:00:00Z",
        updatedAt: "2026-06-08T00:00:00Z",
      },
    ]);

    render(<AuthCenterSection />);

    expect(await screen.findByText("Codex")).toBeInTheDocument();
    expect(screen.getByTitle("Query usage")).toBeInTheDocument();
  });

  it("marks logged out accounts and disables unavailable actions", async () => {
    authApiMock.listAccounts.mockResolvedValue([
      {
        id: "gh-logged-out",
        provider: "github_copilot",
        label: "Logged Out Account",
        username: "octo",
        isDefault: true,
        status: "logged_out",
        createdAt: "2026-06-08T00:00:00Z",
        updatedAt: "2026-06-08T00:00:00Z",
      },
    ]);
    authApiMock.deleteAccount.mockResolvedValue(true);

    render(<AuthCenterSection />);

    expect(await screen.findByText("Logged Out Account")).toBeInTheDocument();
    expect(screen.getByText("Logged out")).toBeInTheDocument();
    expect(screen.queryByText("Default")).not.toBeInTheDocument();
    expect(screen.getByTitle("Query usage")).toBeDisabled();
    expect(screen.getByTitle("Set default")).toBeDisabled();
    expect(screen.getByTitle("Logout")).toBeDisabled();
    expect(screen.getByTitle("Delete")).not.toBeDisabled();

    fireEvent.click(screen.getByTitle("Query usage"));
    fireEvent.click(screen.getByTitle("Set default"));
    fireEvent.click(screen.getByTitle("Logout"));
    expect(authApiMock.queryUsage).not.toHaveBeenCalled();
    expect(authApiMock.setDefault).not.toHaveBeenCalled();
    expect(authApiMock.logout).not.toHaveBeenCalled();
  });
});
