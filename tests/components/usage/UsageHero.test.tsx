import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UsageHero } from "@/components/usage/UsageHero";

const usageQueryMocks = vi.hoisted(() => ({
  useUsageSummary: vi.fn(),
  useUsageSummaryByApp: vi.fn(),
}));

vi.mock("@/lib/query/usage", () => usageQueryMocks);

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

describe("UsageHero", () => {
  it("labels Claude Desktop separately in the app breakdown", () => {
    usageQueryMocks.useUsageSummary.mockReturnValue({ data: summary });
    usageQueryMocks.useUsageSummaryByApp.mockReturnValue({
      data: [{ appType: "claude-desktop", summary }],
    });

    render(
      <UsageHero
        range={{ preset: "today" }}
        appType="all"
        refreshIntervalMs={0}
      />,
    );

    expect(screen.getByText(/Claude Desktop/)).toHaveTextContent(
      "Claude Desktop: $0",
    );
  });
});
