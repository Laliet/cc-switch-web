import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { SessionManagerPage } from "@/components/sessions/SessionManagerPage";

const api = vi.hoisted(() => ({
  list: vi.fn(),
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
    api.list.mockResolvedValue(sessions);
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
  });

  it("searches across titles and project paths", async () => {
    render(<SessionManagerPage open onOpenChange={vi.fn()} />);
    await screen.findAllByText("Fix the login flow");

    fireEvent.change(screen.getByLabelText("sessionManager.search"), {
      target: { value: "deployment" },
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
});
