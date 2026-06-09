import type { ReactNode } from "react";
import { act, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useSwitchProviderMutation } from "@/lib/query";

const toastMock = vi.hoisted(() => ({
  success: vi.fn(),
  error: vi.fn(),
}));

const providersApiMock = vi.hoisted(() => ({
  switch: vi.fn(),
  updateTrayMenu: vi.fn(),
}));

const settingsApiMock = vi.hoisted(() => ({
  getConfigDirInfo: vi.fn(),
}));

vi.mock("sonner", () => ({
  toast: toastMock,
}));

vi.mock("react-i18next", () => ({
  initReactI18next: {
    type: "3rdParty",
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (_key: string, options?: Record<string, unknown>) =>
      String(options?.defaultValue ?? _key),
  }),
}));

vi.mock("@/lib/api", () => ({
  providersApi: providersApiMock,
  settingsApi: settingsApiMock,
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { retry: false },
    },
  });

  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return { wrapper };
}

describe("provider mutations", () => {
  beforeEach(() => {
    providersApiMock.switch.mockReset();
    providersApiMock.updateTrayMenu.mockReset();
    settingsApiMock.getConfigDirInfo.mockReset();
    toastMock.success.mockReset();
    toastMock.error.mockReset();

    providersApiMock.switch.mockResolvedValue(undefined);
    providersApiMock.updateTrayMenu.mockResolvedValue(true);
    settingsApiMock.getConfigDirInfo.mockResolvedValue({
      dir: "/home/user/.claude",
    });
  });

  it("shows the Claude Desktop restart hint after switching a Desktop provider", async () => {
    const { wrapper } = createWrapper();
    const { result } = renderHook(
      () => useSwitchProviderMutation("claude-desktop"),
      { wrapper },
    );

    await act(async () => {
      await result.current.mutateAsync("desktop-provider");
    });

    expect(providersApiMock.switch).toHaveBeenCalledWith(
      "desktop-provider",
      "claude-desktop",
    );
    expect(settingsApiMock.getConfigDirInfo).not.toHaveBeenCalled();
    expect(toastMock.success).toHaveBeenCalledWith("切换供应商成功", {
      description:
        "已写入 Claude Desktop 3P profile。如未生效，请完全退出并重新打开 Claude Desktop。",
    });
  });
});
