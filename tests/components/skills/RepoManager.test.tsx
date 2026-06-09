import type { ComponentProps } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RepoManager } from "@/components/skills/RepoManager";
import type { Skill, SkillRepo } from "@/lib/api/skills";

const openExternalMock = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));

vi.mock("@/lib/api", () => ({
  settingsApi: {
    openExternal: (...args: unknown[]) => openExternalMock(...args),
  },
}));

const tMock = vi.fn((key: string, params?: Record<string, unknown>) => {
  if (key === "skills.repo.skillCount") {
    return `skills.repo.skillCount:${params?.count ?? 0}`;
  }
  return key;
});

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

vi.mock("@/components/ui/dialog", () => ({
  Dialog: ({ open, onOpenChange, children }: any) =>
    open ? (
      <div data-testid="dialog-root">
        <button
          type="button"
          data-testid="dialog-close"
          onClick={() => onOpenChange?.(false)}
        >
          close
        </button>
        {children}
      </div>
    ) : null,
  DialogContent: ({ children }: any) => <div>{children}</div>,
  DialogHeader: ({ children }: any) => <div>{children}</div>,
  DialogTitle: ({ children }: any) => <h2>{children}</h2>,
  DialogDescription: ({ children }: any) => <p>{children}</p>,
}));

vi.mock("@/components/ui/button", () => ({
  Button: ({ children, type = "button", ...rest }: any) => (
    <button type={type} {...rest}>
      {children}
    </button>
  ),
}));

vi.mock("@/components/ui/input", () => ({
  Input: ({ value, onChange, ...rest }: any) => (
    <input
      value={value}
      onChange={(event) =>
        onChange?.({ target: { value: event.target.value } })
      }
      {...rest}
    />
  ),
}));

vi.mock("@/components/ui/label", () => ({
  Label: ({ children, htmlFor }: any) => (
    <label htmlFor={htmlFor}>{children}</label>
  ),
}));

const createRepo = (overrides: Partial<SkillRepo> = {}): SkillRepo => ({
  owner: "acme",
  name: "skills",
  branch: "",
  enabled: true,
  skillsPath: undefined,
  ...overrides,
});

const createSkill = (overrides: Partial<Skill> = {}): Skill => ({
  key: "skill-1",
  name: "Skill",
  description: "Skill description",
  directory: "skills/skill",
  installed: false,
  repoOwner: "acme",
  repoName: "skills",
  repoBranch: "",
  ...overrides,
});

const renderRepoManager = (
  props: Partial<ComponentProps<typeof RepoManager>> = {},
) => {
  const onAdd = props.onAdd ?? vi.fn().mockResolvedValue(undefined);
  const onRemove = props.onRemove ?? vi.fn().mockResolvedValue(undefined);
  const onOpenChange = props.onOpenChange ?? vi.fn();

  render(
    <RepoManager
      open={props.open ?? true}
      onOpenChange={onOpenChange}
      repos={props.repos ?? []}
      skills={props.skills ?? []}
      onAdd={onAdd}
      onRemove={onRemove}
    />,
  );

  return { onAdd, onRemove, onOpenChange };
};

beforeEach(() => {
  tMock.mockClear();
  openExternalMock.mockClear();
});

describe("RepoManager", () => {
  it("renders when open and calls onOpenChange on close", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();

    const { rerender } = render(
      <RepoManager
        open={false}
        onOpenChange={onOpenChange}
        repos={[]}
        skills={[]}
        onAdd={vi.fn().mockResolvedValue(undefined)}
        onRemove={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(screen.queryByText("skills.repo.title")).not.toBeInTheDocument();

    rerender(
      <RepoManager
        open={true}
        onOpenChange={onOpenChange}
        repos={[]}
        skills={[]}
        onAdd={vi.fn().mockResolvedValue(undefined)}
        onRemove={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(screen.getByText("skills.repo.title")).toBeInTheDocument();

    await user.click(screen.getByTestId("dialog-close"));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("updates add form inputs", async () => {
    const user = userEvent.setup();
    renderRepoManager();

    const repoInput = screen.getByLabelText(
      "skills.repo.url",
    ) as HTMLInputElement;
    const branchInput = screen.getByPlaceholderText(
      "skills.repo.branchPlaceholder",
    ) as HTMLInputElement;
    const pathInput = screen.getByPlaceholderText(
      "skills.repo.pathPlaceholder",
    ) as HTMLInputElement;

    await user.type(repoInput, "https://github.com/acme/skills");
    await user.type(branchInput, "main");
    await user.type(pathInput, "skills");

    expect(repoInput.value).toBe("https://github.com/acme/skills");
    expect(branchInput.value).toBe("main");
    expect(pathInput.value).toBe("skills");
  });

  it.each([
    ["https://github.com/owner/name", { owner: "owner", name: "name" }],
    ["https://github.com/owner/name/", { owner: "owner", name: "name" }],
    ["https://github.com/owner/name.git", { owner: "owner", name: "name" }],
    ["git@github.com:owner/name.git", { owner: "owner", name: "name" }],
    ["owner/name", { owner: "owner", name: "name" }],
  ])("parses repo url format %s", async (url, expected) => {
    const user = userEvent.setup();
    const onAdd = vi.fn().mockResolvedValue(undefined);
    renderRepoManager({ onAdd });

    await user.type(screen.getByLabelText("skills.repo.url"), url);
    await user.click(screen.getByRole("button", { name: "skills.repo.add" }));

    await waitFor(() => expect(onAdd).toHaveBeenCalledTimes(1));
    expect(onAdd).toHaveBeenCalledWith(
      expect.objectContaining({
        owner: expected.owner,
        name: expected.name,
      }),
    );
  });

  it("shows validation error for invalid repo url", async () => {
    const user = userEvent.setup();
    const onAdd = vi.fn().mockResolvedValue(undefined);
    renderRepoManager({ onAdd });

    await user.type(screen.getByLabelText("skills.repo.url"), "invalid-url");
    await user.click(screen.getByRole("button", { name: "skills.repo.add" }));

    expect(onAdd).not.toHaveBeenCalled();
    expect(screen.getByText("skills.repo.invalidUrl")).toBeInTheDocument();
  });

  it("adds repo with normalized inputs and clears fields", async () => {
    const user = userEvent.setup();
    const onAdd = vi.fn().mockResolvedValue(undefined);
    renderRepoManager({ onAdd });

    const repoInput = screen.getByLabelText(
      "skills.repo.url",
    ) as HTMLInputElement;
    const branchInput = screen.getByPlaceholderText(
      "skills.repo.branchPlaceholder",
    ) as HTMLInputElement;
    const pathInput = screen.getByPlaceholderText(
      "skills.repo.pathPlaceholder",
    ) as HTMLInputElement;

    await user.type(repoInput, "https://github.com/acme/skills");
    await user.type(branchInput, " main ");
    await user.type(pathInput, " skills ");
    await user.click(screen.getByRole("button", { name: "skills.repo.add" }));

    await waitFor(() =>
      expect(onAdd).toHaveBeenCalledWith({
        owner: "acme",
        name: "skills",
        branch: "main",
        enabled: true,
        skillsPath: "skills",
      }),
    );

    await waitFor(() => {
      expect(repoInput.value).toBe("");
      expect(branchInput.value).toBe("");
      expect(pathInput.value).toBe("");
    });
  });

  it("shows error when add fails and preserves input values", async () => {
    const user = userEvent.setup();
    const onAdd = vi.fn().mockRejectedValue(new Error("Boom"));
    renderRepoManager({ onAdd });

    const repoInput = screen.getByLabelText(
      "skills.repo.url",
    ) as HTMLInputElement;
    const branchInput = screen.getByPlaceholderText(
      "skills.repo.branchPlaceholder",
    ) as HTMLInputElement;
    const pathInput = screen.getByPlaceholderText(
      "skills.repo.pathPlaceholder",
    ) as HTMLInputElement;

    await user.type(repoInput, "owner/name");
    await user.type(branchInput, "dev");
    await user.type(pathInput, "tools");
    await user.click(screen.getByRole("button", { name: "skills.repo.add" }));

    expect(await screen.findByText("Boom")).toBeInTheDocument();
    expect(repoInput.value).toBe("owner/name");
    expect(branchInput.value).toBe("dev");
    expect(pathInput.value).toBe("tools");
  });

  it("renders repo list with skill counts and default branch", () => {
    const repos = [
      createRepo({
        owner: "alice",
        name: "alpha",
        branch: "main",
        skillsPath: "skills",
      }),
      createRepo({ owner: "bob", name: "beta", branch: "" }),
    ];
    const skills = [
      createSkill({
        repoOwner: "alice",
        repoName: "alpha",
        repoBranch: "main",
      }),
      createSkill({
        repoOwner: "alice",
        repoName: "alpha",
        repoBranch: " main ",
      }),
      createSkill({ repoOwner: "alice", repoName: "alpha", repoBranch: "dev" }),
      createSkill({ repoOwner: "bob", repoName: "beta", repoBranch: "" }),
    ];

    renderRepoManager({ repos, skills });

    expect(screen.getByText("alice/alpha")).toBeInTheDocument();
    expect(screen.getByText("bob/beta")).toBeInTheDocument();
    expect(screen.getByText(/skills\.repo\.path/)).toBeInTheDocument();
    expect(screen.getByText("skills.repo.skillCount:2")).toBeInTheDocument();
    expect(screen.getByText(/skills\.repo\.defaultBranch/)).toBeInTheDocument();
  });

  it("shows empty state when repo list is empty", () => {
    renderRepoManager({ repos: [] });

    expect(screen.getByText("skills.repo.empty")).toBeInTheDocument();
  });

  it("calls onRemove when delete button clicked", async () => {
    const user = userEvent.setup();
    const onRemove = vi.fn().mockResolvedValue(undefined);
    const repos = [createRepo({ owner: "acme", name: "skills" })];

    renderRepoManager({ repos, onRemove });

    await user.click(screen.getByTitle("common.delete"));

    expect(onRemove).toHaveBeenCalledTimes(1);
    expect(onRemove).toHaveBeenCalledWith("acme", "skills");
  });

  it("opens GitHub repo link", async () => {
    const user = userEvent.setup();
    const repos = [createRepo({ owner: "acme", name: "skills" })];

    renderRepoManager({ repos });

    await user.click(screen.getByTitle("common.view"));

    expect(openExternalMock).toHaveBeenCalledTimes(1);
    expect(openExternalMock).toHaveBeenCalledWith(
      "https://github.com/acme/skills",
    );
  });

  it("logs error when opening repo link fails", async () => {
    const user = userEvent.setup();
    const repos = [createRepo({ owner: "acme", name: "skills" })];
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);

    openExternalMock.mockRejectedValueOnce(new Error("Nope"));

    renderRepoManager({ repos });

    await user.click(screen.getByTitle("common.view"));

    await waitFor(() =>
      expect(consoleErrorSpy).toHaveBeenCalledWith(
        "Failed to open URL:",
        expect.any(Error),
      ),
    );

    consoleErrorSpy.mockRestore();
  });
});
