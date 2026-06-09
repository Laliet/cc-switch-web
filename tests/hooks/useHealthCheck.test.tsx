import type { ReactNode } from "react";
import { renderHook, act, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useHealthCheck } from "@/hooks/useHealthCheck";

const checkMultipleMock = vi.hoisted(() => vi.fn());
const refreshMock = vi.hoisted(() => vi.fn());
const mappingMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", () => ({
  healthCheckApi: {
    checkMultiple: (...args: unknown[]) => checkMultipleMock(...args),
    refresh: (...args: unknown[]) => refreshMock(...args),
  },
}));

vi.mock("@/config/healthCheckMapping", () => ({
  getRelayPulseProviderFromProvider: (...args: unknown[]) =>
    mappingMock(...args),
}));

interface WrapperProps {
  children: ReactNode;
}

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  const wrapper = ({ children }: WrapperProps) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return { wrapper, queryClient };
};

describe("useHealthCheck", () => {
  beforeEach(() => {
    checkMultipleMock.mockReset();
    refreshMock.mockReset();
    mappingMock.mockReset();
  });

  it("maps providers and fills fallback health", async () => {
    mappingMock.mockImplementation((provider: { name: string }) =>
      provider.name === "Mapped" ? "relay" : undefined,
    );
    checkMultipleMock.mockResolvedValueOnce(new Map());

    const providers = [
      { id: "p1", name: "Mapped", settingsConfig: {} },
      { id: "p2", name: "Skipped", settingsConfig: {} },
    ];

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useHealthCheck("claude", providers), {
      wrapper,
    });

    await waitFor(() => expect(result.current.healthMap.p1).toBeDefined());
    expect(result.current.healthMap.p1.status).toBe("unknown");
    expect(result.current.healthMap.p2).toBeUndefined();

    expect(checkMultipleMock).toHaveBeenCalledWith([
      { relayPulseProvider: "relay", service: "cc" },
    ]);
  });

  it("uses returned health data for mapped providers", async () => {
    mappingMock.mockImplementation(() => "relay");
    checkMultipleMock.mockResolvedValueOnce(
      new Map([
        [
          "relay/cx",
          {
            isHealthy: true,
            status: "available",
            latency: 10,
            lastChecked: 123,
          },
        ],
      ]),
    );

    const providers = [{ id: "p1", name: "Mapped", settingsConfig: {} }];
    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useHealthCheck("codex", providers), {
      wrapper,
    });

    await waitFor(() => expect(result.current.healthMap.p1).toBeDefined());
    expect(result.current.healthMap.p1.status).toBe("available");
    expect(checkMultipleMock).toHaveBeenCalledWith([
      { relayPulseProvider: "relay", service: "cx" },
    ]);
  });

  it("skips fetching when disabled or empty", async () => {
    const { wrapper } = createWrapper();
    const { result } = renderHook(
      () => useHealthCheck("claude", [], { enabled: false }),
      { wrapper },
    );

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(checkMultipleMock).not.toHaveBeenCalled();
  });

  it("refetch triggers refresh before query refetch", async () => {
    mappingMock.mockImplementation(() => "relay");
    checkMultipleMock.mockResolvedValue(new Map());

    const providers = [{ id: "p1", name: "Mapped", settingsConfig: {} }];
    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useHealthCheck("claude", providers), {
      wrapper,
    });

    await waitFor(() => expect(result.current.healthMap.p1).toBeDefined());

    await act(async () => {
      await result.current.refetch();
    });

    expect(refreshMock).toHaveBeenCalledTimes(1);
    expect(checkMultipleMock).toHaveBeenCalled();
  });
});
