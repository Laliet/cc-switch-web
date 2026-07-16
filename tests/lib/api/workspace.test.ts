import { beforeEach, describe, expect, it, vi } from "vitest";

import { workspaceApi } from "@/lib/api/workspace";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api/adapter", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("workspaceApi", () => {
  beforeEach(() => invokeMock.mockReset());

  it("searches all Daily Memory files on the host", async () => {
    const results = [
      {
        date: "2026-07-16",
        sizeBytes: 12,
        modifiedAt: 1,
        etag: "etag-1",
        snippet: "release notes",
        matchCount: 1,
      },
    ];
    invokeMock.mockResolvedValueOnce(results);

    await expect(workspaceApi.searchDailyMemory("release")).resolves.toEqual(
      results,
    );
    expect(invokeMock).toHaveBeenCalledWith("search_daily_memory_files", {
      query: "release",
    });
  });

  it("requires the loaded ETag when deleting Daily Memory", async () => {
    invokeMock.mockResolvedValueOnce({
      date: "2026-07-16",
      deleted: true,
      backupId: "backup-1",
    });

    await workspaceApi.deleteDailyMemory("2026-07-16", "etag-1");
    expect(invokeMock).toHaveBeenCalledWith("delete_daily_memory_file", {
      date: "2026-07-16",
      expectedEtag: "etag-1",
    });
  });
});
