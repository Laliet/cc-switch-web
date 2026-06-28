import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { UsageDashboard } from "@/components/usage/UsageDashboard";
import type {
  AppTypeFilter,
  UsageRangeSelection,
  UsageStatsFilters,
} from "@/types/usage";

const usageQueryMocks = vi.hoisted(() => ({
  useDataSources: vi.fn(),
  useUsageDataExtent: vi.fn(),
  useUsageSummary: vi.fn(),
  useUsageSummaryByApp: vi.fn(),
  useUsageTrends: vi.fn(),
  useProviderStats: vi.fn(),
  useModelStats: vi.fn(),
  useRequestLogs: vi.fn(),
  useModelPricing: vi.fn(),
  useUpdateModelPricing: vi.fn(),
  useDeleteModelPricing: vi.fn(),
}));

vi.mock("@/lib/query/usage", () => usageQueryMocks);
vi.mock("@/components/usage/UsageHero", () => ({
  UsageHero: ({
    range,
    appType,
    filters,
  }: {
    range: UsageRangeSelection;
    appType: AppTypeFilter;
    filters?: UsageStatsFilters;
  }) => (
    <div data-testid="usage-hero-range">
      {range.preset}:{appType}:{range.customStartDate ?? ""}:
      {range.customEndDate ?? ""}:{filters?.providerId ?? ""}:
      {filters?.model ?? ""}
    </div>
  ),
}));
vi.mock("@/components/usage/UsageTrendChart", () => ({
  UsageTrendChart: ({
    range,
    filters,
  }: {
    range: UsageRangeSelection;
    filters?: UsageStatsFilters;
  }) => (
    <div data-testid="usage-trend-range">
      {range.preset}:{filters?.providerId ?? ""}:{filters?.model ?? ""}
    </div>
  ),
}));
vi.mock("@/components/usage/RequestLogTable", () => ({
  RequestLogTable: ({
    range,
    filters,
  }: {
    range: UsageRangeSelection;
    filters?: UsageStatsFilters;
  }) => (
    <div data-testid="request-log-range">
      {range.preset}:{filters?.providerId ?? ""}:{filters?.model ?? ""}
    </div>
  ),
}));
vi.mock("@/components/usage/ProviderStatsTable", () => ({
  ProviderStatsTable: ({
    range,
    filters,
  }: {
    range: UsageRangeSelection;
    filters?: UsageStatsFilters;
  }) => (
    <div data-testid="provider-stats-range">
      {range.preset}:{filters?.providerId ?? ""}:{filters?.model ?? ""}
    </div>
  ),
}));
vi.mock("@/components/usage/ModelStatsTable", () => ({
  ModelStatsTable: ({
    range,
    filters,
  }: {
    range: UsageRangeSelection;
    filters?: UsageStatsFilters;
  }) => (
    <div data-testid="model-stats-range">
      {range.preset}:{filters?.providerId ?? ""}:{filters?.model ?? ""}
    </div>
  ),
}));
vi.mock("@/components/usage/PricingConfigPanel", () => ({
  PricingConfigPanel: () => <div data-testid="pricing-panel" />,
}));

const summary = {
  totalRequests: 0,
  totalCost: "0",
  totalInputTokens: 0,
  totalOutputTokens: 0,
  totalCacheCreationTokens: 0,
  totalCacheReadTokens: 0,
  successRate: 0,
  realTotalTokens: 0,
  cacheHitRate: 0,
};

const renderDashboard = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <UsageDashboard />
    </QueryClientProvider>,
  );
};

describe("UsageDashboard", () => {
  beforeEach(() => {
    for (const mock of Object.values(usageQueryMocks)) {
      mock.mockReset();
    }
    vi.spyOn(Date, "now").mockReturnValue(
      new Date("2026-06-02T08:00:00+08:00").getTime(),
    );
    usageQueryMocks.useDataSources.mockReturnValue({
      data: [
        {
          dataSource: "codex_session",
          requestCount: 27243,
          totalCostUsd: "1003.763089",
        },
      ],
    });
    usageQueryMocks.useUsageDataExtent.mockReturnValue({
      data: {
        firstSeenAt: new Date("2025-10-04T00:00:00+08:00").getTime(),
        lastSeenAt: new Date("2025-12-27T10:30:00+08:00").getTime(),
        requestCount: 27243,
      },
    });
    usageQueryMocks.useUsageSummary.mockReturnValue({ data: summary });
    usageQueryMocks.useUsageSummaryByApp.mockReturnValue({ data: [] });
    usageQueryMocks.useUsageTrends.mockReturnValue({ data: [] });
    usageQueryMocks.useProviderStats.mockReturnValue({ data: [] });
    usageQueryMocks.useModelStats.mockReturnValue({ data: [] });
    usageQueryMocks.useRequestLogs.mockReturnValue({
      data: { data: [], total: 0, page: 0, pageSize: 20 },
      isLoading: false,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("auto-selects a recent data range when today is empty but historical usage exists", async () => {
    renderDashboard();

    await waitFor(() =>
      expect(screen.getByTestId("usage-hero-range")).toHaveTextContent(
        "custom:all",
      ),
    );
    expect(screen.getByTestId("usage-hero-range")).toHaveTextContent(
      String(new Date(2025, 11, 21).getTime()),
    );
    expect(screen.getByText("All-time data sources")).toBeInTheDocument();
  });

  it("shows an inline error when usage summary loading fails", () => {
    usageQueryMocks.useUsageSummary.mockReturnValue({
      data: undefined,
      error: new Error("API connection failed"),
    });

    renderDashboard();

    expect(
      screen.getByText("Usage data failed to load: API connection failed"),
    ).toBeInTheDocument();
  });
});
