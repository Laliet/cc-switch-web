import type { ReactNode } from "react";
import { Children, cloneElement, isValidElement } from "react";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Skill } from "@/lib/api/skills";
import { SkillsPage } from "@/components/skills/SkillsPage";

const getAllMock = vi.hoisted(() => vi.fn());
const getReposMock = vi.hoisted(() => vi.fn());
const addRepoMock = vi.hoisted(() => vi.fn());
const removeRepoMock = vi.hoisted(() => vi.fn());
const installMock = vi.hoisted(() => vi.fn());
const uninstallMock = vi.hoisted(() => vi.fn());
const toastSuccessMock = vi.hoisted(() => vi.fn());
const toastErrorMock = vi.hoisted(() => vi.fn());
const toastWarningMock = vi.hoisted(() => vi.fn());
const skillCardState = vi.hoisted(() => ({ shouldThrow: false }));

const tMock = vi.fn((key: string, options?: Record<string, unknown>) => {
  if (key === "skills.depthGroup") {
    return `depth-${String(options?.depth ?? "")}`;
  }
  if (key === "skills.depthUnknown") {
    return "depth-unknown";
  }
  if (key === "skills.repo.local") {
    return "local";
  }
  if (key === "skills.repo.fetchWarning") {
    return "skills.repo.fetchWarning";
  }
  return key;
});

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => toastSuccessMock(...args),
    error: (...args: unknown[]) => toastErrorMock(...args),
    warning: (...args: unknown[]) => toastWarningMock(...args),
  },
}));

vi.mock("@/lib/api/skills", () => ({
  skillsApi: {
    getAll: (...args: unknown[]) => getAllMock(...args),
    getRepos: (...args: unknown[]) => getReposMock(...args),
    install: (...args: unknown[]) => installMock(...args),
    uninstall: (...args: unknown[]) => uninstallMock(...args),
    addRepo: (...args: unknown[]) => addRepoMock(...args),
    removeRepo: (...args: unknown[]) => removeRepoMock(...args),
  },
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
    <input value={value} onChange={onChange} {...rest} />
  ),
}));

vi.mock("@/components/ui/switch", () => ({
  Switch: ({ checked, onCheckedChange, ...rest }: any) => (
    <input
      type="checkbox"
      role="checkbox"
      checked={!!checked}
      onChange={(event) => onCheckedChange?.(event.target.checked)}
      {...rest}
    />
  ),
}));

const collectSelectItems = (
  nodes: ReactNode,
  items: Array<ReturnType<typeof cloneElement>>,
) => {
  Children.forEach(nodes, (child) => {
    if (!isValidElement(child)) {
      return;
    }
    const type = child.type as { __isSelectItem?: boolean };
    if (type?.__isSelectItem) {
      items.push(child);
      return;
    }
    if (child.props?.children) {
      collectSelectItems(child.props.children, items);
    }
  });
};

vi.mock("@/components/ui/select", () => {
  const Select = ({ value, onValueChange, children }: any) => {
    const items: Array<ReturnType<typeof cloneElement>> = [];
    collectSelectItems(children, items);
    return (
      <select
        value={value}
        onChange={(event) => onValueChange?.(event.target.value)}
      >
        {items.map((item, index) =>
          cloneElement(item, { key: item.key ?? index }),
        )}
      </select>
    );
  };

  const SelectTrigger = ({ children }: any) => <>{children}</>;
  const SelectValue = ({ children }: any) => <>{children}</>;
  const SelectContent = ({ children }: any) => <>{children}</>;
  const SelectItem = ({ value, children }: any) => (
    <option value={value}>{children}</option>
  );
  SelectItem.__isSelectItem = true;

  return {
    Select,
    SelectTrigger,
    SelectValue,
    SelectContent,
    SelectItem,
  };
});

vi.mock("@/components/skills/RepoManager", () => ({
  RepoManager: ({ open, onAdd, onRemove }: any) =>
    open ? (
      <div>
        repo-manager
        <button
          type="button"
          data-testid="repo-add"
          onClick={() =>
            onAdd?.({
              owner: "acme",
              name: "skills",
              branch: "main",
              enabled: true,
            })
          }
        >
          repo-add
        </button>
        <button
          type="button"
          data-testid="repo-remove"
          onClick={() => onRemove?.("acme", "skills")}
        >
          repo-remove
        </button>
      </div>
    ) : null,
}));

vi.mock("@/components/skills/SkillCard", () => ({
  SkillCard: ({ skill, onInstall, onUninstall }: any) => {
    if (skillCardState.shouldThrow) {
      throw new Error("SkillCard crash");
    }
    return (
      <div data-testid={`skill-card-${skill.key}`}>
        <span>{skill.name}</span>
        {skill.installed ? (
          <button type="button" onClick={() => onUninstall(skill.directory)}>
            uninstall
          </button>
        ) : (
          <button type="button" onClick={() => onInstall(skill.directory)}>
            install
          </button>
        )}
      </div>
    );
  },
}));

const createSkill = (overrides: Partial<Skill> = {}): Skill => ({
  key: overrides.key ?? "skill-1",
  name: overrides.name ?? "Skill One",
  description: overrides.description ?? "Skill description",
  directory: overrides.directory ?? "skills/skill-one",
  installed: overrides.installed ?? false,
  parentPath: overrides.parentPath,
  depth: overrides.depth,
  commands: overrides.commands,
  readmeUrl: overrides.readmeUrl,
  repoOwner: overrides.repoOwner,
  repoName: overrides.repoName,
  repoBranch: overrides.repoBranch,
  skillsPath: overrides.skillsPath,
  installedApps: overrides.installedApps,
});

beforeEach(() => {
  getAllMock.mockReset();
  getReposMock.mockReset();
  addRepoMock.mockReset();
  removeRepoMock.mockReset();
  installMock.mockReset();
  uninstallMock.mockReset();
  toastSuccessMock.mockClear();
  toastErrorMock.mockClear();
  toastWarningMock.mockClear();
  tMock.mockClear();
  skillCardState.shouldThrow = false;

  getReposMock.mockResolvedValue([]);
  addRepoMock.mockResolvedValue(true);
  removeRepoMock.mockResolvedValue(true);
  installMock.mockResolvedValue(true);
  uninstallMock.mockResolvedValue(true);
});

describe("SkillsPage", () => {
  it("shows loading state", async () => {
    getAllMock.mockReturnValue(new Promise(() => {}));

    const { container } = render(<SkillsPage />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "skills.refreshing" }),
      ).toBeDisabled(),
    );
    expect(container.querySelector("svg.animate-spin")).toBeInTheDocument();
  });

  it("renders skill list", async () => {
    const skillOne = createSkill({ key: "skill-1", name: "Skill One" });
    const skillTwo = createSkill({ key: "skill-2", name: "Skill Two" });
    getAllMock.mockResolvedValueOnce({ skills: [skillOne, skillTwo] });

    render(<SkillsPage />);

    await waitFor(() => {
      expect(screen.getByText("Skill One")).toBeInTheDocument();
      expect(screen.getByText("Skill Two")).toBeInTheDocument();
    });
  });

  it("shows refreshing status when metadata returned", async () => {
    getAllMock.mockResolvedValueOnce({
      skills: [createSkill()],
      cacheHit: true,
      refreshing: true,
    });

    render(<SkillsPage />);

    await waitFor(() =>
      expect(
        screen.getByText("skills.cacheStatus.refreshing"),
      ).toBeInTheDocument(),
    );
  });

  it("shows cache hit status when cache hit", async () => {
    getAllMock.mockResolvedValueOnce({
      skills: [createSkill()],
      cacheHit: true,
      refreshing: false,
    });

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("skills.cacheStatus.hit")).toBeInTheDocument(),
    );
  });

  it("filters by search query", async () => {
    const user = userEvent.setup();
    const skillOne = createSkill({ key: "skill-1", name: "Focus Mode" });
    const skillTwo = createSkill({ key: "skill-2", name: "Time Tracker" });
    getAllMock.mockResolvedValueOnce({ skills: [skillOne, skillTwo] });

    render(<SkillsPage />);

    await waitFor(() => expect(screen.getByText("Focus Mode")).toBeInTheDocument());

    await user.type(
      screen.getByPlaceholderText("skills.searchPlaceholder"),
      "  focus  ",
    );

    expect(screen.getByText("Focus Mode")).toBeInTheDocument();
    expect(screen.queryByText("Time Tracker")).not.toBeInTheDocument();
  });

  it("filters by install status", async () => {
    const user = userEvent.setup();
    const installedSkill = createSkill({
      key: "skill-1",
      name: "Installed Skill",
      installed: true,
    });
    const uninstalledSkill = createSkill({
      key: "skill-2",
      name: "Uninstalled Skill",
      installed: false,
    });
    getAllMock.mockResolvedValueOnce({
      skills: [installedSkill, uninstalledSkill],
    });

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("Installed Skill")).toBeInTheDocument(),
    );

    const [installSelect] = screen.getAllByRole("combobox");
    await user.selectOptions(installSelect, "installed");

    expect(screen.getByText("Installed Skill")).toBeInTheDocument();
    expect(screen.queryByText("Uninstalled Skill")).not.toBeInTheDocument();

    await user.selectOptions(installSelect, "uninstalled");

    expect(screen.getByText("Uninstalled Skill")).toBeInTheDocument();
    expect(screen.queryByText("Installed Skill")).not.toBeInTheDocument();
  });

  it("filters by repository", async () => {
    const user = userEvent.setup();
    const repoSkill = createSkill({
      key: "skill-1",
      name: "Repo Skill",
      repoOwner: "acme",
      repoName: "skills",
      repoBranch: "dev",
    });
    const localSkill = createSkill({
      key: "skill-2",
      name: "Local Skill",
    });
    getAllMock.mockResolvedValueOnce({ skills: [repoSkill, localSkill] });

    render(<SkillsPage />);

    await waitFor(() => expect(screen.getByText("Repo Skill")).toBeInTheDocument());

    const [, repoSelect] = screen.getAllByRole("combobox");
    await user.selectOptions(repoSelect, "acme/skills@dev");

    expect(screen.getByText("Repo Skill")).toBeInTheDocument();
    expect(screen.queryByText("Local Skill")).not.toBeInTheDocument();
  });

  it("groups by depth when enabled", async () => {
    const user = userEvent.setup();
    const negativeDepth = createSkill({
      key: "skill-1",
      name: "Depth Negative",
      depth: -1,
    });
    const depthTwo = createSkill({
      key: "skill-2",
      name: "Depth Two",
      depth: 2,
    });
    const unknownDepth = createSkill({
      key: "skill-3",
      name: "Depth Unknown",
      depth: undefined,
    });
    getAllMock.mockResolvedValueOnce({
      skills: [negativeDepth, depthTwo, unknownDepth],
    });

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("Depth Negative")).toBeInTheDocument(),
    );

    await user.click(
      screen.getByRole("checkbox", { name: "skills.groupByDepth" }),
    );

    const depthZeroLabel = screen.getByText("depth-0");
    const depthTwoLabel = screen.getByText("depth-2");
    const depthUnknownLabel = screen.getByText("depth-unknown");

    expect(depthZeroLabel).toBeInTheDocument();
    expect(depthTwoLabel).toBeInTheDocument();
    expect(depthUnknownLabel).toBeInTheDocument();
    expect(
      depthZeroLabel.compareDocumentPosition(depthTwoLabel) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      depthTwoLabel.compareDocumentPosition(depthUnknownLabel) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("handles install and uninstall flows", async () => {
    const user = userEvent.setup();
    const notInstalled = createSkill({
      key: "skill-1",
      name: "Not Installed",
      directory: "skills/not-installed",
      installed: false,
    });
    const installed = createSkill({
      key: "skill-2",
      name: "Installed",
      directory: "skills/installed",
      installed: true,
    });

    getAllMock
      .mockResolvedValueOnce({ skills: [notInstalled, installed] })
      .mockResolvedValueOnce({
        skills: [
          { ...notInstalled, installed: true },
          installed,
        ],
      })
      .mockResolvedValueOnce({
        skills: [
          { ...notInstalled, installed: true },
          { ...installed, installed: false },
        ],
      });

    render(<SkillsPage />);

    await waitFor(() => expect(screen.getByText("Not Installed")).toBeInTheDocument());

    const notInstalledCard = screen.getByTestId("skill-card-skill-1");
    await user.click(within(notInstalledCard).getByRole("button", { name: "install" }));

    await waitFor(() => {
      expect(installMock).toHaveBeenCalledWith(
        "skills/not-installed",
        undefined,
        "claude",
      );
      expect(getAllMock).toHaveBeenCalledTimes(2);
    });
    expect(toastSuccessMock).toHaveBeenCalledWith("skills.installSuccess");

    const installedCard = screen.getByTestId("skill-card-skill-2");
    await user.click(within(installedCard).getByRole("button", { name: "uninstall" }));

    await waitFor(() => {
      expect(uninstallMock).toHaveBeenCalledWith("skills/installed", "claude");
      expect(getAllMock).toHaveBeenCalledTimes(3);
    });
    expect(toastSuccessMock).toHaveBeenCalledWith("skills.uninstallSuccess");
  });

  it("switches target app without relying on global app switch", async () => {
    const user = userEvent.setup();
    const skill = createSkill({
      key: "skill-1",
      name: "Target Switch Skill",
      directory: "skills/target-switch",
      installed: false,
    });

    getAllMock
      .mockResolvedValueOnce({ skills: [skill] })
      .mockResolvedValueOnce({ skills: [skill] })
      .mockResolvedValueOnce({
        skills: [{ ...skill, installed: true }],
      });

    render(<SkillsPage appId="claude" />);

    await waitFor(() =>
      expect(screen.getByText("Target Switch Skill")).toBeInTheDocument(),
    );
    expect(getAllMock).toHaveBeenCalledWith("claude");

    await user.click(screen.getByRole("button", { name: "apps.codex" }));

    await waitFor(() => expect(getAllMock).toHaveBeenLastCalledWith("codex"));

    const card = screen.getByTestId("skill-card-skill-1");
    await user.click(within(card).getByRole("button", { name: "install" }));

    await waitFor(() =>
      expect(installMock).toHaveBeenCalledWith(
        "skills/target-switch",
        undefined,
        "codex",
      ),
    );
  });

  it("shows cross-app warning before install when already installed in another app", async () => {
    const user = userEvent.setup();
    const skill = createSkill({
      key: "skill-1",
      name: "Shared Skill",
      directory: "skills/shared",
      installed: false,
      installedApps: ["codex"],
    });

    getAllMock
      .mockResolvedValueOnce({ skills: [skill] })
      .mockResolvedValueOnce({
        skills: [{ ...skill, installed: true, installedApps: ["claude", "codex"] }],
      });

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("Shared Skill")).toBeInTheDocument(),
    );
    const card = screen.getByTestId("skill-card-skill-1");
    await user.click(within(card).getByRole("button", { name: "install" }));

    await waitFor(() =>
      expect(toastWarningMock).toHaveBeenCalledWith(
        "skills.crossAppInstallHintTitle",
        expect.objectContaining({ duration: 7000 }),
      ),
    );
    expect(installMock).toHaveBeenCalledWith("skills/shared", undefined, "claude");
  });

  it("renders error boundary fallback and retries", async () => {
    const user = userEvent.setup();
    const consoleSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);

    skillCardState.shouldThrow = true;
    const skill = createSkill({ key: "skill-1", name: "Crash Skill" });
    getAllMock
      .mockResolvedValueOnce({ skills: [skill] })
      .mockResolvedValueOnce({ skills: [skill] });

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("skills.errorBoundaryTitle")).toBeInTheDocument(),
    );

    skillCardState.shouldThrow = false;
    await user.click(
      screen.getByRole("button", { name: "skills.errorBoundaryRetry" }),
    );

    await waitFor(() =>
      expect(screen.getByText("Crash Skill")).toBeInTheDocument(),
    );

    consoleSpy.mockRestore();
  });

  it("shows empty state when no skills", async () => {
    getAllMock.mockResolvedValueOnce({ skills: [] });

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("skills.empty")).toBeInTheDocument(),
    );
    expect(screen.getByText("skills.emptyDescription")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "skills.addRepo" })).toBeInTheDocument();
  });

  it("shows no results state when filters match nothing", async () => {
    const user = userEvent.setup();
    const skill = createSkill({ key: "skill-1", name: "Focus Mode" });
    getAllMock.mockResolvedValueOnce({ skills: [skill] });

    render(<SkillsPage />);

    await waitFor(() => expect(screen.getByText("Focus Mode")).toBeInTheDocument());

    await user.type(
      screen.getByPlaceholderText("skills.searchPlaceholder"),
      "nonexistent",
    );

    await waitFor(() =>
      expect(screen.getByText("skills.noResults")).toBeInTheDocument(),
    );
    expect(screen.getByText("skills.noResultsDescription")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "skills.clearFilters" })).toBeInTheDocument();
  });

  it("clears filters when clear button clicked", async () => {
    const user = userEvent.setup();
    const skill = createSkill({ key: "skill-1", name: "Focus Mode" });
    getAllMock.mockResolvedValueOnce({ skills: [skill] });

    render(<SkillsPage />);

    await waitFor(() => expect(screen.getByText("Focus Mode")).toBeInTheDocument());

    await user.type(
      screen.getByPlaceholderText("skills.searchPlaceholder"),
      "nonexistent",
    );

    await waitFor(() =>
      expect(screen.getByText("skills.noResults")).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: "skills.clearFilters" }));

    await waitFor(() =>
      expect(screen.getByText("Focus Mode")).toBeInTheDocument(),
    );
  });

  it("shows error toast on install failure", async () => {
    const user = userEvent.setup();
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const skill = createSkill({
      key: "skill-1",
      name: "Fail Skill",
      directory: "skills/fail",
      installed: false,
    });
    getAllMock.mockResolvedValue({ skills: [skill] });
    installMock.mockRejectedValueOnce(new Error("Install failed"));

    render(<SkillsPage />);

    await waitFor(() => expect(screen.getByText("Fail Skill")).toBeInTheDocument());

    const card = screen.getByTestId("skill-card-skill-1");
    await user.click(within(card).getByRole("button", { name: "install" }));

    await waitFor(() => expect(toastErrorMock).toHaveBeenCalled());
    consoleSpy.mockRestore();
  });

  it("maps unsupported-app install error to localized toast message", async () => {
    const user = userEvent.setup();
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const skill = createSkill({
      key: "skill-1",
      name: "Coming Soon Skill",
      directory: "skills/coming-soon",
      installed: false,
    });
    getAllMock.mockResolvedValue({ skills: [skill] });
    installMock.mockRejectedValueOnce(new Error("App 'omo' is not supported yet."));

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("Coming Soon Skill")).toBeInTheDocument(),
    );
    const card = screen.getByTestId("skill-card-skill-1");
    await user.click(within(card).getByRole("button", { name: "install" }));

    await waitFor(() =>
      expect(toastErrorMock).toHaveBeenCalledWith(
        "skills.installFailed",
        expect.objectContaining({
          description: "skills.error.appNotSupported",
        }),
      ),
    );
    consoleSpy.mockRestore();
  });

  it("shows error toast on uninstall failure", async () => {
    const user = userEvent.setup();
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const skill = createSkill({
      key: "skill-1",
      name: "Fail Skill",
      directory: "skills/fail",
      installed: true,
    });
    getAllMock.mockResolvedValue({ skills: [skill] });
    uninstallMock.mockRejectedValueOnce(new Error("Uninstall failed"));

    render(<SkillsPage />);

    await waitFor(() => expect(screen.getByText("Fail Skill")).toBeInTheDocument());

    const card = screen.getByTestId("skill-card-skill-1");
    await user.click(within(card).getByRole("button", { name: "uninstall" }));

    await waitFor(() => expect(toastErrorMock).toHaveBeenCalled());
    consoleSpy.mockRestore();
  });

  it("shows warning toast when getAll returns warnings", async () => {
    getAllMock.mockResolvedValueOnce({
      skills: [createSkill()],
      warnings: ["Failed to fetch repo A", "Failed to fetch repo B"],
    });

    render(<SkillsPage />);

    await waitFor(() => expect(toastWarningMock).toHaveBeenCalled());
    expect(toastWarningMock).toHaveBeenCalledWith(
      "skills.repo.fetchWarning",
      expect.objectContaining({ description: expect.any(String) }),
    );
  });

  it("shows error toast on load failure", async () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    getAllMock.mockRejectedValueOnce(new Error("Network error"));

    render(<SkillsPage />);

    await waitFor(() => expect(toastErrorMock).toHaveBeenCalled());
    consoleSpy.mockRestore();
  });

  it("refreshes skills when refresh button clicked", async () => {
    const user = userEvent.setup();
    const skill = createSkill({ key: "skill-1", name: "Skill One" });
    getAllMock
      .mockResolvedValueOnce({ skills: [skill] })
      .mockResolvedValueOnce({ skills: [skill] });

    render(<SkillsPage />);

    await waitFor(() => expect(screen.getByText("Skill One")).toBeInTheDocument());
    expect(getAllMock).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "skills.refresh" }));

    await waitFor(() => expect(getAllMock).toHaveBeenCalledTimes(2));
  });

  it("opens repo manager when button clicked", async () => {
    const user = userEvent.setup();
    getAllMock.mockResolvedValueOnce({ skills: [] });

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("skills.empty")).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: "skills.repoManager" }));

    await waitFor(() =>
      expect(screen.getByText("repo-manager")).toBeInTheDocument(),
    );
  });

  it("adds repo and shows success toast", async () => {
    const user = userEvent.setup();
    const repoSkill = createSkill({
      key: "skill-1",
      name: "Repo Skill",
      repoOwner: "acme",
      repoName: "skills",
      repoBranch: "main",
    });

    getAllMock
      .mockResolvedValueOnce({ skills: [] })
      .mockResolvedValueOnce({ skills: [repoSkill] });
    getReposMock.mockResolvedValueOnce([]);

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("skills.empty")).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: "skills.repoManager" }));
    await user.click(await screen.findByTestId("repo-add"));

    await waitFor(() =>
      expect(addRepoMock).toHaveBeenCalledWith({
        owner: "acme",
        name: "skills",
        branch: "main",
        enabled: true,
      }),
    );
    await waitFor(() =>
      expect(toastSuccessMock).toHaveBeenCalledWith("skills.repo.addSuccess"),
    );
  });

  it("warns when repo refresh fails after add", async () => {
    const user = userEvent.setup();
    const consoleSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);

    getAllMock
      .mockResolvedValueOnce({ skills: [] })
      .mockRejectedValueOnce(new Error("fail"));
    getReposMock.mockResolvedValueOnce([]);
    addRepoMock.mockResolvedValueOnce(true);

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("skills.empty")).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: "skills.repoManager" }));
    await user.click(await screen.findByTestId("repo-add"));

    await waitFor(() =>
      expect(toastSuccessMock).toHaveBeenCalledWith(
        "skills.repo.addSuccessSimple",
      ),
    );
    await waitFor(() =>
      expect(toastWarningMock).toHaveBeenCalledWith(
        "skills.repo.refreshFailed",
        expect.objectContaining({ duration: 8000 }),
      ),
    );

    consoleSpy.mockRestore();
  });

  it("remove repo shows error when remove fails", async () => {
    const user = userEvent.setup();
    const consoleSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);

    getAllMock.mockResolvedValueOnce({ skills: [] });
    getReposMock.mockResolvedValueOnce([]);
    removeRepoMock.mockRejectedValueOnce(new Error("nope"));

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("skills.empty")).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: "skills.repoManager" }));
    await user.click(await screen.findByTestId("repo-remove"));

    await waitFor(() => expect(toastErrorMock).toHaveBeenCalled());

    consoleSpy.mockRestore();
  });

  it("remove repo warns when refresh fails", async () => {
    const user = userEvent.setup();
    const consoleSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);

    getAllMock
      .mockResolvedValueOnce({ skills: [] })
      .mockResolvedValueOnce({ skills: [] });
    getReposMock
      .mockResolvedValueOnce([])
      .mockRejectedValueOnce(new Error("fail"));
    removeRepoMock.mockResolvedValueOnce(true);

    render(<SkillsPage />);

    await waitFor(() =>
      expect(screen.getByText("skills.empty")).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: "skills.repoManager" }));
    await user.click(await screen.findByTestId("repo-remove"));

    await waitFor(() =>
      expect(toastWarningMock).toHaveBeenCalledWith(
        "skills.repo.refreshFailed",
        expect.objectContaining({ duration: 8000 }),
      ),
    );

    consoleSpy.mockRestore();
  });

  it("resets repo filter when filtered repo no longer exists", async () => {
    const user = userEvent.setup();
    const repoSkill = createSkill({
      key: "skill-1",
      name: "Repo Skill",
      repoOwner: "acme",
      repoName: "skills",
    });
    const localSkill = createSkill({
      key: "skill-2",
      name: "Local Skill",
    });

    getAllMock
      .mockResolvedValueOnce({ skills: [repoSkill, localSkill] })
      .mockResolvedValueOnce({ skills: [localSkill] });

    render(<SkillsPage />);

    await waitFor(() => expect(screen.getByText("Repo Skill")).toBeInTheDocument());

    const [, repoSelect] = screen.getAllByRole("combobox");
    await user.selectOptions(repoSelect, "acme/skills@main");

    expect(screen.getByText("Repo Skill")).toBeInTheDocument();
    expect(screen.queryByText("Local Skill")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "skills.refresh" }));

    await waitFor(() => expect(screen.getByText("Local Skill")).toBeInTheDocument());
  });
});
