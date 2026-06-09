import type { ReactNode } from "react";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useProvidersQuery } from "@/lib/query/queries";

const getAllMock = vi.hoisted(() => vi.fn());
const getCurrentMock = vi.hoisted(() => vi.fn());
const getBackupMock = vi.hoisted(() => vi.fn());
const importDefaultMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", () => ({
  providersApi: {
    getAll: (...args: unknown[]) => getAllMock(...args),
    getCurrent: (...args: unknown[]) => getCurrentMock(...args),
    getBackup: (...args: unknown[]) => getBackupMock(...args),
    importDefault: (...args: unknown[]) => importDefaultMock(...args),
  },
  settingsApi: {},
  usageApi: {},
}));

interface WrapperProps {
  children: ReactNode;
}

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });

  const wrapper = ({ children }: WrapperProps) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return { wrapper, queryClient };
};

describe("useProvidersQuery", () => {
  beforeEach(() => {
    getAllMock.mockReset();
    getCurrentMock.mockReset();
    getBackupMock.mockReset();
    importDefaultMock.mockReset();
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  it("does not import defaults when loading providers fails", async () => {
    getAllMock.mockRejectedValueOnce(new Error("network failed"));
    getCurrentMock.mockResolvedValueOnce("claude-1");
    getBackupMock.mockResolvedValueOnce(null);

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useProvidersQuery("claude"), {
      wrapper,
    });

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(importDefaultMock).not.toHaveBeenCalled();
    expect(result.current.data).toEqual({
      providers: {},
      currentProviderId: "claude-1",
      backupProviderId: null,
    });
  });

  it("imports defaults only after an empty provider list is confirmed", async () => {
    getAllMock.mockResolvedValueOnce({}).mockResolvedValueOnce({
      "claude-1": {
        id: "claude-1",
        name: "Claude Default",
        settingsConfig: {},
        category: "official",
        createdAt: 1,
        sortIndex: 0,
      },
    });
    getCurrentMock.mockResolvedValueOnce("").mockResolvedValueOnce("claude-1");
    getBackupMock.mockResolvedValueOnce(null);
    importDefaultMock.mockResolvedValueOnce(true);

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useProvidersQuery("claude"), {
      wrapper,
    });

    await waitFor(() =>
      expect(result.current.data?.providers["claude-1"]?.name).toBe(
        "Claude Default",
      ),
    );

    expect(importDefaultMock).toHaveBeenCalledTimes(1);
    expect(result.current.data).toEqual({
      providers: {
        "claude-1": {
          id: "claude-1",
          name: "Claude Default",
          settingsConfig: {},
          category: "official",
          createdAt: 1,
          sortIndex: 0,
        },
      },
      currentProviderId: "claude-1",
      backupProviderId: null,
    });
  });
});
