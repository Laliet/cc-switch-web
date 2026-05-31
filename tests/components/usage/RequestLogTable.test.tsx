import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RequestLogTable } from "@/components/usage/RequestLogTable";
import type { RequestLog, UsageRangeSelection } from "@/types/usage";

const queryMocks = vi.hoisted(() => ({
  useRequestLogs: vi.fn(),
}));

vi.mock("@/lib/query/usage", () => queryMocks);

describe("RequestLogTable", () => {
  beforeEach(() => {
    queryMocks.useRequestLogs.mockReset();
    queryMocks.useRequestLogs.mockReturnValue({
      data: {
        data: [],
        total: 41,
        page: 0,
        pageSize: 20,
      },
      isLoading: false,
    });
  });

  it("resets pagination when parent usage filters change", async () => {
    const user = userEvent.setup();
    const range: UsageRangeSelection = { preset: "today" };
    const { rerender } = render(
      <RequestLogTable range={range} appType="claude" refreshIntervalMs={0} />,
    );

    await user.click(screen.getByRole("button", { name: "Next" }));
    await waitFor(() =>
      expect(queryMocks.useRequestLogs.mock.calls.at(-1)?.[2]).toBe(1),
    );

    rerender(
      <RequestLogTable range={range} appType="codex" refreshIntervalMs={0} />,
    );

    await waitFor(() => {
      const lastCall = queryMocks.useRequestLogs.mock.calls.at(-1);
      expect(lastCall?.[1]).toMatchObject({ appType: "codex" });
      expect(lastCall?.[2]).toBe(0);
    });
  });

  it("shows data source and supports direct page jumps", async () => {
    const user = userEvent.setup();
    const log: RequestLog = {
      requestId: "request-1",
      providerId: "provider-1",
      providerName: "Provider One",
      appType: "claude",
      model: "claude-sonnet-4-6",
      costMultiplier: "1",
      inputTokens: 10,
      outputTokens: 5,
      cacheReadTokens: 2,
      cacheCreationTokens: 3,
      inputCostUsd: "0",
      outputCostUsd: "0",
      cacheReadCostUsd: "0",
      cacheCreationCostUsd: "0",
      totalCostUsd: "0",
      isStreaming: true,
      latencyMs: 120,
      statusCode: 200,
      createdAt: 1_775_000_000_000,
      dataSource: "session",
      isUnpriced: false,
    };
    queryMocks.useRequestLogs.mockReturnValue({
      data: {
        data: [log],
        total: 41,
        page: 0,
        pageSize: 20,
      },
      isLoading: false,
    });

    render(
      <RequestLogTable
        range={{ preset: "today" }}
        appType="claude"
        refreshIntervalMs={0}
      />,
    );

    expect(screen.getByText("Session")).toBeInTheDocument();

    await user.clear(screen.getByLabelText("Jump to page"));
    await user.type(screen.getByLabelText("Jump to page"), "3");
    await user.click(screen.getByRole("button", { name: "Go" }));

    await waitFor(() =>
      expect(queryMocks.useRequestLogs.mock.calls.at(-1)?.[2]).toBe(2),
    );
  });
});
