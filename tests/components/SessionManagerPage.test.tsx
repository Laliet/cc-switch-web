import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { SessionManagerPage } from "@/components/sessions/SessionManagerPage";

const api = vi.hoisted(() => ({
  list: vi.fn(),
  listPage: vi.fn(),
  getMessages: vi.fn(),
  delete: vi.fn(),
  deleteMany: vi.fn(),
}));

vi.mock("@/lib/api/sessions", () => ({ sessionsApi: api }));

const sessions = [
  {
    providerId: "codex",
    sessionId: "codex-1",
    title: "Fix the login flow",
    summary: "Investigate authentication errors",
    projectDir: "/srv/projects/web",
    sourcePath: "/home/server/.codex/sessions/codex-1.jsonl",
    resumeCommand: "codex resume codex-1",
    lastActiveAt: 1_700_000_000_000,
  },
  {
    providerId: "claude",
    sessionId: "claude-1",
    title: "Document deployment",
    sourcePath: "/home/server/.claude/projects/claude-1.jsonl",
    resumeCommand: "claude --resume claude-1",
  },
];

describe("SessionManagerPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.list.mockResolvedValue(sessions);
    api.listPage.mockResolvedValue({
      sessions,
      nextCursor: undefined,
      total: sessions.length,
      scannedAt: 1_700_000_000_000,
    });
    api.getMessages.mockResolvedValue([
      { role: "user", content: "Please fix authentication" },
      { role: "assistant", content: "I will inspect the login flow" },
    ]);
    api.delete.mockResolvedValue(true);
    api.deleteMany.mockResolvedValue([]);
    Object.assign(navigator, {
      clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
  });

  it("loads server sessions and displays messages and server paths", async () => {
    render(<SessionManagerPage open onOpenChange={vi.fn()} />);

    expect(
      (await screen.findAllByText("Fix the login flow")).length,
    ).toBeGreaterThan(0);
    expect(screen.getByText("/srv/projects/web")).toBeInTheDocument();
    expect(
      screen.getByText(/\/home\/server\/\.codex\/sessions\/codex-1\.jsonl/),
    ).toBeInTheDocument();
    expect(
      (await screen.findAllByText("Please fix authentication")).length,
    ).toBeGreaterThan(0);
    expect(api.getMessages).toHaveBeenCalledWith(
      "codex",
      "/home/server/.codex/sessions/codex-1.jsonl",
    );
    expect(api.listPage).toHaveBeenCalledWith({
      limit: 100,
      providerId: undefined,
      query: undefined,
      refresh: false,
    });
  });

  it("searches the complete server-side session snapshot", async () => {
    api.listPage.mockImplementation(({ query }: { query?: string } = {}) =>
      Promise.resolve({
        sessions: query === "deployment" ? [sessions[1]] : sessions,
        nextCursor: undefined,
        total: query === "deployment" ? 1 : sessions.length,
        scannedAt: 1_700_000_000_000,
      }),
    );
    render(<SessionManagerPage open onOpenChange={vi.fn()} />);
    await screen.findAllByText("Fix the login flow");

    fireEvent.change(screen.getByLabelText("sessionManager.search"), {
      target: { value: "deployment" },
    });

    await waitFor(() =>
      expect(api.listPage).toHaveBeenLastCalledWith({
        limit: 100,
        providerId: undefined,
        query: "deployment",
        refresh: false,
      }),
    );
    expect(
      (await screen.findAllByText("Document deployment")).length,
    ).toBeGreaterThan(0);
    expect(screen.queryByText("Fix the login flow")).not.toBeInTheDocument();
  });

  it("ignores a slower response from an obsolete search", async () => {
    type Page = {
      sessions: typeof sessions;
      nextCursor: undefined;
      total: number;
      scannedAt: number;
    };
    let resolveOld!: (page: Page) => void;
    let resolveCurrent!: (page: Page) => void;
    api.listPage.mockImplementation(({ query }: { query?: string } = {}) => {
      if (query === "old") {
        return new Promise<Page>((resolve) => {
          resolveOld = resolve;
        });
      }
      if (query === "current") {
        return new Promise<Page>((resolve) => {
          resolveCurrent = resolve;
        });
      }
      return Promise.resolve({
        sessions,
        nextCursor: undefined,
        total: sessions.length,
        scannedAt: 1_700_000_000_000,
      });
    });
    render(<SessionManagerPage open onOpenChange={vi.fn()} />);
    await screen.findAllByText("Fix the login flow");

    const input = screen.getByLabelText("sessionManager.search");
    fireEvent.change(input, { target: { value: "old" } });
    await waitFor(() =>
      expect(api.listPage).toHaveBeenCalledWith(
        expect.objectContaining({ query: "old" }),
      ),
    );
    fireEvent.change(input, { target: { value: "current" } });
    await waitFor(() =>
      expect(api.listPage).toHaveBeenCalledWith(
        expect.objectContaining({ query: "current" }),
      ),
    );

    await act(async () => {
      resolveCurrent({
        sessions: [sessions[1]],
        nextCursor: undefined,
        total: 1,
        scannedAt: 2,
      });
    });
    expect(
      (await screen.findAllByText("Document deployment")).length,
    ).toBeGreaterThan(0);

    await act(async () => {
      resolveOld({
        sessions: [sessions[0]],
        nextCursor: undefined,
        total: 1,
        scannedAt: 1,
      });
    });
    expect(screen.getAllByText("Document deployment").length).toBeGreaterThan(
      0,
    );
    expect(screen.queryByText("Fix the login flow")).not.toBeInTheDocument();
  });

  it("copies the resume command instead of launching a terminal", async () => {
    render(<SessionManagerPage open onOpenChange={vi.fn()} />);
    await screen.findAllByText("Fix the login flow");

    fireEvent.click(
      screen.getByRole("button", { name: "sessionManager.copyResume" }),
    );

    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        "codex resume codex-1",
      ),
    );
  });

  it("loads additional session pages on demand", async () => {
    api.listPage
      .mockResolvedValueOnce({
        sessions: [sessions[0]],
        nextCursor: "1",
        total: 2,
        scannedAt: 1_700_000_000_000,
      })
      .mockResolvedValueOnce({
        sessions: [sessions[1]],
        nextCursor: undefined,
        total: 2,
        scannedAt: 1_700_000_000_000,
      });

    render(<SessionManagerPage open onOpenChange={vi.fn()} />);
    await screen.findAllByText("Fix the login flow");

    fireEvent.click(
      screen.getByRole("button", { name: /sessionManager\.loadMore/ }),
    );

    expect(await screen.findByText("Document deployment")).toBeInTheDocument();
    expect(api.listPage).toHaveBeenLastCalledWith({
      cursor: "1",
      limit: 100,
      providerId: undefined,
      query: undefined,
    });
  });

  it("renders only the visible window for very long conversations", async () => {
    api.getMessages.mockResolvedValue(
      Array.from({ length: 2_000 }, (_, index) => ({
        role: index % 2 === 0 ? "user" : "assistant",
        content: `virtual message ${index}`,
        ts: index,
      })),
    );

    render(<SessionManagerPage open onOpenChange={vi.fn()} />);
    const messageScroll = await screen.findByTestId("session-message-scroll");
    const messageToc = await screen.findByTestId("session-message-toc");
    await within(messageScroll).findByText("virtual message 0");

    const renderedMessages = within(messageScroll).queryAllByText(
      /^virtual message \d+$/,
    );
    expect(renderedMessages.length).toBeGreaterThan(0);
    expect(renderedMessages.length).toBeLessThan(80);
    expect(
      within(messageScroll).queryByText("virtual message 1999"),
    ).not.toBeInTheDocument();
    expect(within(messageToc).getAllByRole("button").length).toBeLessThan(80);
  });
});
