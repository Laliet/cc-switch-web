import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RequestLogTable } from "@/components/usage/RequestLogTable";
import type { UsageRangeSelection } from "@/types/usage";

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
});
