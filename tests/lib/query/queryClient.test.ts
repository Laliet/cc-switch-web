import { describe, expect, it, vi } from "vitest";
import { createAppQueryClient } from "@/lib/query/queryClient";

const toastMock = vi.hoisted(() => ({
  error: vi.fn(),
}));

vi.mock("sonner", () => ({
  toast: toastMock,
}));

describe("app query client", () => {
  it("shows a toast when a background query fails", async () => {
    const queryClient = createAppQueryClient();

    await expect(
      queryClient.fetchQuery({
        queryKey: ["api", "failure"],
        queryFn: async () => {
          throw new Error("server disconnected");
        },
        retry: false,
      }),
    ).rejects.toThrow("server disconnected");

    expect(toastMock.error).toHaveBeenCalledWith("API request failed", {
      description: "server disconnected",
    });
  });

  it("allows queries to suppress global error toasts", async () => {
    const queryClient = createAppQueryClient();
    toastMock.error.mockClear();

    await expect(
      queryClient.fetchQuery({
        queryKey: ["api", "quiet-failure"],
        queryFn: async () => {
          throw new Error("handled locally");
        },
        meta: { suppressErrorToast: true },
        retry: false,
      }),
    ).rejects.toThrow("handled locally");

    expect(toastMock.error).not.toHaveBeenCalled();
  });
});
