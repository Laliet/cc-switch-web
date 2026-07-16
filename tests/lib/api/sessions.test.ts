import { beforeEach, describe, expect, it, vi } from "vitest";

import { sessionsApi } from "@/lib/api/sessions";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api/adapter", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const session = {
  providerId: "codex",
  sessionId: "session-1",
  sourcePath: "/home/user/.codex/sessions/session-1.jsonl",
};

describe("sessionsApi", () => {
  beforeEach(() => invokeMock.mockReset());

  it("lists server-host sessions", async () => {
    invokeMock.mockResolvedValueOnce([session]);
    await expect(sessionsApi.list()).resolves.toEqual([session]);
    expect(invokeMock).toHaveBeenCalledWith("list_sessions", {
      refresh: false,
    });
  });

  it("lists a paged server-host session result", async () => {
    const page = {
      sessions: [session],
      nextCursor: "100",
      total: 101,
      scannedAt: 123,
    };
    invokeMock.mockResolvedValueOnce(page);
    await expect(
      sessionsApi.listPage({
        cursor: "0",
        limit: 100,
        providerId: "codex",
        query: "migration",
        refresh: true,
      }),
    ).resolves.toEqual(page);
    expect(invokeMock).toHaveBeenCalledWith("list_sessions_page", {
      cursor: "0",
      limit: 100,
      providerId: "codex",
      query: "migration",
      refresh: true,
    });
  });

  it("loads messages with provider and source path", async () => {
    invokeMock.mockResolvedValueOnce([{ role: "user", content: "hello" }]);
    await sessionsApi.getMessages(session.providerId, session.sourcePath);
    expect(invokeMock).toHaveBeenCalledWith("get_session_messages", {
      providerId: "codex",
      sourcePath: session.sourcePath,
    });
  });

  it("deletes one session", async () => {
    invokeMock.mockResolvedValueOnce(true);
    await expect(sessionsApi.delete(session)).resolves.toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("delete_session", { ...session });
  });

  it("deletes sessions in a batch", async () => {
    const result = [{ ...session, success: true }];
    invokeMock.mockResolvedValueOnce(result);
    await expect(sessionsApi.deleteMany([session])).resolves.toEqual(result);
    expect(invokeMock).toHaveBeenCalledWith("delete_sessions", {
      items: [session],
    });
  });
});
