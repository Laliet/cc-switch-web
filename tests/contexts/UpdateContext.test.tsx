import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UpdateProvider, useUpdate } from "@/contexts/UpdateContext";

const checkForUpdateMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/updater", () => ({
  checkForUpdate: (...args: unknown[]) => checkForUpdateMock(...args),
}));

vi.mock("@/lib/query", () => ({
  useCapabilitiesQuery: () => ({
    data: { features: { appUpdate: true } },
  }),
}));

const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: vi.fn<(key: string) => string | null>(
      (key: string) => store[key] ?? null,
    ),
    setItem: vi.fn((key: string, value: string) => {
      store[key] = value;
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key];
    }),
    clear: () => {
      store = {};
    },
  };
})();

Object.defineProperty(window, "localStorage", {
  value: localStorageMock,
});

function TestConsumer() {
  const {
    hasUpdate,
    updateInfo,
    isChecking,
    error,
    isDismissed,
    dismissUpdate,
    checkUpdate,
    resetDismiss,
  } = useUpdate();

  return (
    <div>
      <div data-testid="hasUpdate">{String(hasUpdate)}</div>
      <div data-testid="isChecking">{String(isChecking)}</div>
      <div data-testid="isDismissed">{String(isDismissed)}</div>
      <div data-testid="error">{error ?? "null"}</div>
      <div data-testid="version">{updateInfo?.availableVersion ?? "null"}</div>
      <button onClick={() => checkUpdate()}>Check Update</button>
      <button onClick={dismissUpdate}>Dismiss</button>
      <button onClick={resetDismiss}>Reset Dismiss</button>
    </div>
  );
}

describe("UpdateContext", () => {
  beforeEach(() => {
    vi.clearAllTimers();
    checkForUpdateMock.mockReset();
    localStorageMock.clear();
    localStorageMock.getItem.mockClear();
    localStorageMock.setItem.mockClear();
    localStorageMock.removeItem.mockClear();
    // Default mock to prevent auto-check from hanging
    checkForUpdateMock.mockResolvedValue({ status: "up-to-date" });
  });

  it("throws error when useUpdate is used outside provider", () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    expect(() => render(<TestConsumer />)).toThrow(
      "useUpdate must be used within UpdateProvider",
    );

    consoleSpy.mockRestore();
  });

  it("provides initial state", () => {
    render(
      <UpdateProvider>
        <TestConsumer />
      </UpdateProvider>,
    );

    expect(screen.getByTestId("hasUpdate")).toHaveTextContent("false");
    expect(screen.getByTestId("isDismissed")).toHaveTextContent("false");
    expect(screen.getByTestId("error")).toHaveTextContent("null");
  });

  it("sets hasUpdate when manual check finds update", async () => {
    const user = userEvent.setup();
    checkForUpdateMock.mockResolvedValue({
      status: "available",
      info: {
        currentVersion: "1.0.0",
        availableVersion: "2.0.0",
        notes: "New features",
      },
      update: { downloadAndInstall: vi.fn() },
    });

    render(
      <UpdateProvider>
        <TestConsumer />
      </UpdateProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Check Update" }));

    await waitFor(() => {
      expect(screen.getByTestId("hasUpdate")).toHaveTextContent("true");
      expect(screen.getByTestId("version")).toHaveTextContent("2.0.0");
    });
  });

  it("handles check update error", async () => {
    const user = userEvent.setup();
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    checkForUpdateMock.mockRejectedValue(new Error("Network error"));

    render(
      <UpdateProvider>
        <TestConsumer />
      </UpdateProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Check Update" }));

    await waitFor(() => {
      expect(screen.getByTestId("error")).toHaveTextContent("Network error");
      expect(screen.getByTestId("hasUpdate")).toHaveTextContent("false");
    });

    consoleSpy.mockRestore();
  });

  it("dismisses update and persists to localStorage", async () => {
    const user = userEvent.setup();
    checkForUpdateMock.mockResolvedValue({
      status: "available",
      info: {
        currentVersion: "1.0.0",
        availableVersion: "2.0.0",
      },
      update: { downloadAndInstall: vi.fn() },
    });

    render(
      <UpdateProvider>
        <TestConsumer />
      </UpdateProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Check Update" }));

    await waitFor(() => {
      expect(screen.getByTestId("hasUpdate")).toHaveTextContent("true");
    });

    await user.click(screen.getByRole("button", { name: "Dismiss" }));

    expect(screen.getByTestId("isDismissed")).toHaveTextContent("true");
    expect(localStorageMock.setItem).toHaveBeenCalledWith(
      "ccswitch:update:dismissedVersion",
      "2.0.0",
    );
  });

  it("resets dismiss state and clears localStorage", async () => {
    const user = userEvent.setup();
    checkForUpdateMock.mockResolvedValue({
      status: "available",
      info: {
        currentVersion: "1.0.0",
        availableVersion: "2.0.0",
      },
      update: { downloadAndInstall: vi.fn() },
    });

    render(
      <UpdateProvider>
        <TestConsumer />
      </UpdateProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Check Update" }));

    await waitFor(() => {
      expect(screen.getByTestId("hasUpdate")).toHaveTextContent("true");
    });

    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.getByTestId("isDismissed")).toHaveTextContent("true");

    await user.click(screen.getByRole("button", { name: "Reset Dismiss" }));
    expect(screen.getByTestId("isDismissed")).toHaveTextContent("false");
    expect(localStorageMock.removeItem).toHaveBeenCalledWith(
      "ccswitch:update:dismissedVersion",
    );
  });

  it("reads dismissed version from localStorage on update check", async () => {
    const user = userEvent.setup();
    localStorageMock.getItem.mockImplementation((key: string) => {
      if (key === "ccswitch:update:dismissedVersion") return "2.0.0";
      return "";
    });

    checkForUpdateMock.mockResolvedValue({
      status: "available",
      info: {
        currentVersion: "1.0.0",
        availableVersion: "2.0.0",
      },
      update: { downloadAndInstall: vi.fn() },
    });

    render(
      <UpdateProvider>
        <TestConsumer />
      </UpdateProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Check Update" }));

    await waitFor(() => {
      expect(screen.getByTestId("hasUpdate")).toHaveTextContent("true");
      expect(screen.getByTestId("isDismissed")).toHaveTextContent("true");
    });
  });

  it("migrates legacy dismissed version key", async () => {
    const user = userEvent.setup();
    localStorageMock.getItem.mockImplementation((key: string) => {
      if (key === "dismissedUpdateVersion") return "1.5.0";
      return "";
    });

    checkForUpdateMock.mockResolvedValue({
      status: "available",
      info: {
        currentVersion: "1.0.0",
        availableVersion: "1.5.0",
      },
      update: { downloadAndInstall: vi.fn() },
    });

    render(
      <UpdateProvider>
        <TestConsumer />
      </UpdateProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Check Update" }));

    await waitFor(() => {
      expect(screen.getByTestId("isDismissed")).toHaveTextContent("true");
    });

    expect(localStorageMock.setItem).toHaveBeenCalledWith(
      "ccswitch:update:dismissedVersion",
      "1.5.0",
    );
    expect(localStorageMock.removeItem).toHaveBeenCalledWith(
      "dismissedUpdateVersion",
    );
  });

  it("clears update state when up-to-date", async () => {
    const user = userEvent.setup();

    // First check finds update
    checkForUpdateMock.mockResolvedValueOnce({
      status: "available",
      info: {
        currentVersion: "1.0.0",
        availableVersion: "2.0.0",
      },
      update: { downloadAndInstall: vi.fn() },
    });

    render(
      <UpdateProvider>
        <TestConsumer />
      </UpdateProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Check Update" }));

    await waitFor(() => {
      expect(screen.getByTestId("hasUpdate")).toHaveTextContent("true");
    });

    // Second check is up-to-date
    checkForUpdateMock.mockResolvedValueOnce({ status: "up-to-date" });

    await user.click(screen.getByRole("button", { name: "Check Update" }));

    await waitFor(() => {
      expect(screen.getByTestId("hasUpdate")).toHaveTextContent("false");
      expect(screen.getByTestId("version")).toHaveTextContent("null");
    });
  });

  it("handles non-Error exceptions", async () => {
    const user = userEvent.setup();
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    checkForUpdateMock.mockRejectedValue("String error");

    render(
      <UpdateProvider>
        <TestConsumer />
      </UpdateProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Check Update" }));

    await waitFor(() => {
      expect(screen.getByTestId("error")).toHaveTextContent("检查更新失败");
    });

    consoleSpy.mockRestore();
  });
});
