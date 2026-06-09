import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Skill } from "@/lib/api/skills";
import { SkillCard } from "@/components/skills/SkillCard";

const openExternalMock = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));

vi.mock("@/lib/api", () => ({
  settingsApi: {
    openExternal: (...args: unknown[]) => openExternalMock(...args),
  },
}));

const tMock = vi.fn((key: string) => key);

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

const createSkill = (overrides: Partial<Skill> = {}): Skill => ({
  key: overrides.key ?? "skill-1",
  name: overrides.name ?? "Focus Mode",
  description: overrides.description ?? "Skill description",
  directory: overrides.directory ?? "skills/focus-mode",
  installed: overrides.installed ?? false,
  parentPath: overrides.parentPath,
  depth: overrides.depth,
  commands: overrides.commands,
  readmeUrl: overrides.readmeUrl,
  repoOwner: overrides.repoOwner,
  repoName: overrides.repoName,
  repoBranch: overrides.repoBranch,
  skillsPath: overrides.skillsPath,
});

const renderSkill = (
  skillOverrides: Partial<Skill> = {},
  options: {
    onInstall?: (directory: string) => Promise<void>;
    onUninstall?: (directory: string) => Promise<void>;
  } = {},
) => {
  const skill = createSkill(skillOverrides);
  const onInstall = options.onInstall ?? vi.fn().mockResolvedValue(undefined);
  const onUninstall =
    options.onUninstall ?? vi.fn().mockResolvedValue(undefined);

  const renderResult = render(
    <SkillCard skill={skill} onInstall={onInstall} onUninstall={onUninstall} />,
  );

  return { skill, onInstall, onUninstall, ...renderResult };
};

beforeEach(() => {
  tMock.mockClear();
  openExternalMock.mockClear();
});

describe("SkillCard", () => {
  it("renders name and description", () => {
    renderSkill({ name: "Project Scout", description: "Scans repositories" });

    expect(screen.getByText("Project Scout")).toBeInTheDocument();
    expect(screen.getByText("Scans repositories")).toBeInTheDocument();
  });

  it("shows installed badge when installed", () => {
    renderSkill({ installed: true });

    expect(screen.getByText("skills.installed")).toBeInTheDocument();
  });

  it("calls onInstall when install button clicked", async () => {
    const user = userEvent.setup();
    const onInstall = vi.fn().mockResolvedValue(undefined);
    const { skill } = renderSkill(
      { repoOwner: "acme", repoName: "skills" },
      { onInstall },
    );

    await user.click(screen.getByRole("button", { name: "skills.install" }));

    expect(onInstall).toHaveBeenCalledTimes(1);
    expect(onInstall).toHaveBeenCalledWith(skill.directory);
  });

  it("calls onUninstall when uninstall button clicked", async () => {
    const user = userEvent.setup();
    const onUninstall = vi.fn().mockResolvedValue(undefined);
    const { skill } = renderSkill({ installed: true }, { onUninstall });

    await user.click(screen.getByRole("button", { name: "skills.uninstall" }));

    expect(onUninstall).toHaveBeenCalledTimes(1);
    expect(onUninstall).toHaveBeenCalledWith(skill.directory);
  });

  it("shows loader icon while installing", async () => {
    const user = userEvent.setup();
    const onInstall = vi.fn(() => new Promise<void>(() => {}));
    const { container } = renderSkill(
      { repoOwner: "acme", repoName: "skills" },
      { onInstall },
    );

    await user.click(screen.getByRole("button", { name: "skills.install" }));

    expect(
      screen.getByRole("button", { name: "skills.installing" }),
    ).toBeInTheDocument();
    expect(container.querySelector("svg.animate-spin")).toBeInTheDocument();
  });

  it("toggles command list visibility", async () => {
    const user = userEvent.setup();
    renderSkill({
      commands: [
        {
          name: "Run analysis",
          description: "Analyze the repository",
          filePath: "commands/analyze.yml",
        },
      ],
    });

    const trigger = screen.getByRole("button", {
      name: "skills.workflows (1)",
    });
    const commandName = screen.getByText("Run analysis");

    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(commandName).not.toBeVisible();

    await user.click(trigger);

    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(commandName).toBeVisible();
  });

  it("opens readme link via settingsApi", async () => {
    const user = userEvent.setup();
    const readmeUrl = "https://github.com/acme/skills";
    renderSkill({ readmeUrl });

    await user.click(screen.getByRole("button", { name: "skills.view" }));

    expect(openExternalMock).toHaveBeenCalledTimes(1);
    expect(openExternalMock).toHaveBeenCalledWith(readmeUrl);
  });
});
