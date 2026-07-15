import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api/adapter", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { subscriptionApi } from "@/lib/api/subscription";

describe("subscriptionApi", () => {
  beforeEach(() => invokeMock.mockReset());

  it("uses the shared Tauri command contract", async () => {
    invokeMock.mockResolvedValueOnce({
      provider: "codex",
      source: "managed_account",
      status: "available",
      windows: [],
      fetchedAt: 1,
    });

    await subscriptionApi.query("codex", {
      accountId: "account-1",
      force: true,
    });

    expect(invokeMock).toHaveBeenCalledWith("query_subscription_quota", {
      provider: "codex",
      accountId: "account-1",
      force: true,
    });
  });

  it("sends explicit defaults so desktop and Web receive the same request", async () => {
    invokeMock.mockResolvedValueOnce({
      provider: "claude",
      source: "cli_credentials",
      status: "unavailable",
      windows: [],
      fetchedAt: 1,
    });

    await subscriptionApi.query("claude");

    expect(invokeMock).toHaveBeenCalledWith("query_subscription_quota", {
      provider: "claude",
      accountId: null,
      force: false,
    });
  });
});
