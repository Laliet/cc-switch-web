import type { ReactNode } from "react";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useRequestLogs, useUsageSummary } from "@/lib/query/usage";
import type { LogFilters, UsageRangeSelection } from "@/types/usage";

const usageApiMocks = vi.hoisted(() => ({
  getUsageSummary: vi.fn(),
  getRequestLogs: vi.fn(),
}));

vi.mock("@/lib/api/usage", () => ({
  usageApi: usageApiMocks,
}));

interface WrapperProps {
  children: ReactNode;
}

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        staleTime: 0,
      },
    },
  });

  const wrapper = ({ children }: WrapperProps) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return { wrapper, queryClient };
};

describe("usage query hooks", () => {
  let nowMs: number;

  beforeEach(() => {
    usageApiMocks.getUsageSummary.mockReset();
    usageApiMocks.getRequestLogs.mockReset();
    nowMs = new Date("2026-05-31T02:00:00Z").getTime();
    vi.spyOn(Date, "now").mockImplementation(() => nowMs);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("recomputes relative summary ranges when a query refetches", async () => {
    usageApiMocks.getUsageSummary.mockResolvedValue({
      totalRequests: 0,
      totalCost: "0",
      totalInputTokens: 0,
      totalOutputTokens: 0,
      totalCacheCreationTokens: 0,
      totalCacheReadTokens: 0,
      successRate: 0,
      realTotalTokens: 0,
      cacheHitRate: 0,
    });
    const range: UsageRangeSelection = { preset: "1d" };
    const { wrapper, queryClient } = createWrapper();

    renderHook(
      () =>
        useUsageSummary(range, "claude", {
          providerId: "provider-1",
          model: "sonnet",
        }),
      { wrapper },
    );
    await waitFor(() =>
      expect(usageApiMocks.getUsageSummary).toHaveBeenCalledTimes(1),
    );
    const firstEnd = usageApiMocks.getUsageSummary.mock.calls[0][1];

    nowMs = new Date("2026-05-31T02:05:00Z").getTime();
    await queryClient.refetchQueries();

    expect(usageApiMocks.getUsageSummary).toHaveBeenCalledTimes(2);
    expect(usageApiMocks.getUsageSummary.mock.calls[1][1]).toBe(
      firstEnd + 300_000,
    );
    expect(usageApiMocks.getUsageSummary.mock.calls[1][3]).toEqual({
      providerId: "provider-1",
      model: "sonnet",
    });
  });

  it("recomputes request log ranges when a query refetches", async () => {
    usageApiMocks.getRequestLogs.mockResolvedValue({
      data: [],
      total: 0,
      page: 0,
      pageSize: 20,
    });
    const range: UsageRangeSelection = { preset: "1d" };
    const filters: LogFilters = { appType: "claude", model: "sonnet" };
    const { wrapper, queryClient } = createWrapper();

    renderHook(() => useRequestLogs(range, filters, 0, 20), { wrapper });
    await waitFor(() =>
      expect(usageApiMocks.getRequestLogs).toHaveBeenCalledTimes(1),
    );
    const firstFilters = usageApiMocks.getRequestLogs.mock.calls[0][0];

    nowMs = new Date("2026-05-31T02:05:00Z").getTime();
    await queryClient.refetchQueries();

    const secondFilters = usageApiMocks.getRequestLogs.mock.calls[1][0];
    expect(secondFilters).toMatchObject({ appType: "claude", model: "sonnet" });
    expect(secondFilters.startDate).toBe(firstFilters.startDate + 300_000);
    expect(secondFilters.endDate).toBe(firstFilters.endDate + 300_000);
  });
});
