import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PricingConfigPanel } from "@/components/usage/PricingConfigPanel";

const usageQueryMocks = vi.hoisted(() => ({
  useModelPricing: vi.fn(),
  useUpdateModelPricing: vi.fn(),
  useDeleteModelPricing: vi.fn(),
}));

vi.mock("@/lib/query/usage", () => usageQueryMocks);

const renderPanel = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <PricingConfigPanel />
    </QueryClientProvider>,
  );
};

describe("PricingConfigPanel", () => {
  const mutateAsync = vi.fn();
  const deleteAsync = vi.fn();

  beforeEach(() => {
    mutateAsync.mockReset();
    deleteAsync.mockReset();
    usageQueryMocks.useModelPricing.mockReturnValue({
      data: [
        {
          modelId: "claude-sonnet-4-6",
          displayName: "Claude Sonnet 4.6",
          inputCostPerMillion: "3",
          outputCostPerMillion: "15",
          cacheReadCostPerMillion: "0.3",
          cacheCreationCostPerMillion: "3.75",
        },
      ],
    });
    usageQueryMocks.useUpdateModelPricing.mockReturnValue({
      mutateAsync,
      isPending: false,
    });
    usageQueryMocks.useDeleteModelPricing.mockReturnValue({
      mutateAsync: deleteAsync,
    });
  });

  it("edits pricing in a dialog instead of the main table surface", async () => {
    const user = userEvent.setup();
    mutateAsync.mockResolvedValue(2);
    renderPanel();

    expect(screen.queryByLabelText("Model ID")).not.toBeInTheDocument();

    await user.click(
      screen.getByRole("button", {
        name: "Edit pricing for claude-sonnet-4-6",
      }),
    );

    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByLabelText("Model ID")).toHaveValue("claude-sonnet-4-6");

    await user.clear(screen.getByLabelText("Output cost / 1M tokens"));
    await user.type(screen.getByLabelText("Output cost / 1M tokens"), "18");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(mutateAsync).toHaveBeenCalledWith(
      expect.objectContaining({
        modelId: "claude-sonnet-4-6",
        outputCostPerMillion: "18",
      }),
    );
  });
});
