/**
 * Integration tests for skills management user flows
 * Tests complete user journeys: browse → search → filter → install → uninstall
 */
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useState, useEffect, useMemo } from "react";
import type { Skill } from "@/lib/api/skills";

// ===== Mock Setup =====
const getAllSkillsMock = vi.hoisted(() => vi.fn());
const installSkillMock = vi.hoisted(() => vi.fn());
const uninstallSkillMock = vi.hoisted(() => vi.fn());
const getReposMock = vi.hoisted(() => vi.fn());
const toastSuccessMock = vi.hoisted(() => vi.fn());
const toastErrorMock = vi.hoisted(() => vi.fn());

const tMock = vi.fn((key: string, options?: Record<string, unknown>) => {
  if (options?.defaultValue) return String(options.defaultValue);
  if (key === "skills.searchPlaceholder") return "Search skills...";
  if (key === "skills.installSuccess") return "Installed successfully";
  if (key === "skills.uninstallSuccess") return "Uninstalled successfully";
  if (key === "skills.empty") return "No skills available";
  if (key === "skills.noResults") return "No results found";
  if (key === "skills.clearFilters") return "Clear filters";
  return key;
});

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

vi.mock("@/lib/api/skills", () => ({
  skillsApi: {
    getAll: (...args: unknown[]) => getAllSkillsMock(...args),
    getRepos: (...args: unknown[]) => getReposMock(...args),
    install: (...args: unknown[]) => installSkillMock(...args),
    uninstall: (...args: unknown[]) => uninstallSkillMock(...args),
  },
}));

vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => toastSuccessMock(...args),
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

// ===== Test Components =====

interface SkillCardProps {
  skill: Skill;
  onInstall: (directory: string) => Promise<void>;
  onUninstall: (directory: string) => Promise<void>;
}

function MockSkillCard({ skill, onInstall, onUninstall }: SkillCardProps) {
  const [isLoading, setIsLoading] = useState(false);

  const handleAction = async () => {
    setIsLoading(true);
    try {
      if (skill.installed) {
        await onUninstall(skill.directory);
      } else {
        await onInstall(skill.directory);
      }
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div data-testid={`skill-card-${skill.key}`}>
      <span data-testid="skill-name">{skill.name}</span>
      <span data-testid="skill-description">{skill.description}</span>
      {skill.installed && <span data-testid="installed-badge">Installed</span>}
      <button onClick={handleAction} disabled={isLoading}>
        {isLoading ? "Loading..." : skill.installed ? "Uninstall" : "Install"}
      </button>
    </div>
  );
}

// Full Skills Management Page for integration testing
function SkillsManagementPage() {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<
    "all" | "installed" | "uninstalled"
  >("all");

  const loadSkills = async () => {
    setIsLoading(true);
    try {
      const result = await getAllSkillsMock();
      setSkills(result.skills || []);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadSkills();
  }, []);

  const filteredSkills = useMemo(() => {
    return skills.filter((skill) => {
      // Search filter
      const query = searchQuery.toLowerCase().trim();
      if (query) {
        const matchesName = skill.name.toLowerCase().includes(query);
        const matchesDescription = skill.description
          ?.toLowerCase()
          .includes(query);
        if (!matchesName && !matchesDescription) return false;
      }

      // Status filter
      if (statusFilter === "installed" && !skill.installed) return false;
      if (statusFilter === "uninstalled" && skill.installed) return false;

      return true;
    });
  }, [skills, searchQuery, statusFilter]);

  const handleInstall = async (directory: string) => {
    try {
      await installSkillMock(directory);
      toastSuccessMock("skills.installSuccess");
      // Refresh skills list
      await loadSkills();
    } catch (err) {
      console.error("Install failed:", err);
    }
  };

  const handleUninstall = async (directory: string) => {
    try {
      await uninstallSkillMock(directory);
      toastSuccessMock("skills.uninstallSuccess");
      // Refresh skills list
      await loadSkills();
    } catch (err) {
      console.error("Uninstall failed:", err);
    }
  };

  const clearFilters = () => {
    setSearchQuery("");
    setStatusFilter("all");
  };

  return (
    <div data-testid="skills-management-page">
      <header>
        <h1>Skills Management</h1>
        <button onClick={loadSkills}>Refresh</button>
      </header>

      {/* Search and Filters */}
      <div data-testid="filters">
        <input
          type="text"
          placeholder="Search skills..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          data-testid="search-input"
        />
        <select
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value as any)}
          data-testid="status-filter"
        >
          <option value="all">All</option>
          <option value="installed">Installed</option>
          <option value="uninstalled">Not Installed</option>
        </select>
      </div>

      {/* Content */}
      {isLoading ? (
        <div data-testid="loading">Loading skills...</div>
      ) : skills.length === 0 ? (
        <div data-testid="empty-state">
          <p>No skills available</p>
        </div>
      ) : filteredSkills.length === 0 ? (
        <div data-testid="no-results">
          <p>No results found</p>
          <button onClick={clearFilters}>Clear filters</button>
        </div>
      ) : (
        <div data-testid="skills-list">
          <div data-testid="skills-count">{filteredSkills.length} skills</div>
          {filteredSkills.map((skill) => (
            <MockSkillCard
              key={skill.key}
              skill={skill}
              onInstall={handleInstall}
              onUninstall={handleUninstall}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ===== Helper Functions =====

const createSkill = (overrides: Partial<Skill> = {}): Skill => ({
  key: overrides.key ?? `skill-${Date.now()}`,
  name: overrides.name ?? "Test Skill",
  description: overrides.description ?? "A test skill description",
  directory: overrides.directory ?? "skills/test-skill",
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

// ===== Tests =====

describe("Skills Management User Flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tMock.mockClear();
    toastSuccessMock.mockClear();
    toastErrorMock.mockClear();

    getReposMock.mockResolvedValue([]);
    installSkillMock.mockResolvedValue(true);
    uninstallSkillMock.mockResolvedValue(true);
  });

  describe("Complete Browse Skills Flow", () => {
    it("user can view skills list on page load", async () => {
      const skills = [
        createSkill({
          key: "skill-1",
          name: "Code Review",
          description: "Automated code review",
        }),
        createSkill({
          key: "skill-2",
          name: "Test Generator",
          description: "Generate unit tests",
        }),
        createSkill({
          key: "skill-3",
          name: "Doc Writer",
          description: "Write documentation",
          installed: true,
        }),
      ];

      getAllSkillsMock.mockResolvedValue({ skills });

      render(<SkillsManagementPage />);

      // 1. Shows loading state first
      expect(screen.getByTestId("loading")).toBeInTheDocument();

      // 2. Wait for skills to load
      await waitFor(() => {
        expect(screen.queryByTestId("loading")).not.toBeInTheDocument();
      });

      // 3. Verify all skills are displayed
      expect(screen.getByText("Code Review")).toBeInTheDocument();
      expect(screen.getByText("Test Generator")).toBeInTheDocument();
      expect(screen.getByText("Doc Writer")).toBeInTheDocument();

      // 4. Verify count
      expect(screen.getByTestId("skills-count")).toHaveTextContent("3 skills");
    });

    it("shows empty state when no skills available", async () => {
      getAllSkillsMock.mockResolvedValue({ skills: [] });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByTestId("empty-state")).toBeInTheDocument();
      });

      expect(screen.getByText("No skills available")).toBeInTheDocument();
    });
  });

  describe("Complete Search Skills Flow", () => {
    it("user can search skills by name", async () => {
      const user = userEvent.setup();
      const skills = [
        createSkill({ key: "skill-1", name: "Code Review" }),
        createSkill({ key: "skill-2", name: "Test Generator" }),
        createSkill({ key: "skill-3", name: "Code Formatter" }),
      ];

      getAllSkillsMock.mockResolvedValue({ skills });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByText("Code Review")).toBeInTheDocument();
      });

      // 1. Type search query
      const searchInput = screen.getByTestId("search-input");
      await user.type(searchInput, "code");

      // 2. Verify filtering
      expect(screen.getByText("Code Review")).toBeInTheDocument();
      expect(screen.getByText("Code Formatter")).toBeInTheDocument();
      expect(screen.queryByText("Test Generator")).not.toBeInTheDocument();

      // 3. Verify count updates
      expect(screen.getByTestId("skills-count")).toHaveTextContent("2 skills");
    });

    it("user can search skills by description", async () => {
      const user = userEvent.setup();
      const skills = [
        createSkill({
          key: "skill-1",
          name: "Skill A",
          description: "Generates unit tests",
        }),
        createSkill({
          key: "skill-2",
          name: "Skill B",
          description: "Reviews code quality",
        }),
      ];

      getAllSkillsMock.mockResolvedValue({ skills });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByText("Skill A")).toBeInTheDocument();
      });

      // Search by description keyword
      await user.type(screen.getByTestId("search-input"), "unit tests");

      expect(screen.getByText("Skill A")).toBeInTheDocument();
      expect(screen.queryByText("Skill B")).not.toBeInTheDocument();
    });

    it("shows no results when search has no matches", async () => {
      const user = userEvent.setup();
      const skills = [createSkill({ key: "skill-1", name: "Code Review" })];

      getAllSkillsMock.mockResolvedValue({ skills });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByText("Code Review")).toBeInTheDocument();
      });

      // Search for non-existent skill
      await user.type(screen.getByTestId("search-input"), "nonexistent");

      expect(screen.getByTestId("no-results")).toBeInTheDocument();
      expect(screen.getByText("No results found")).toBeInTheDocument();
    });

    it("user can clear search to see all skills again", async () => {
      const user = userEvent.setup();
      const skills = [
        createSkill({ key: "skill-1", name: "Skill A" }),
        createSkill({ key: "skill-2", name: "Skill B" }),
      ];

      getAllSkillsMock.mockResolvedValue({ skills });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByText("Skill A")).toBeInTheDocument();
      });

      // Search to filter
      const searchInput = screen.getByTestId("search-input");
      await user.type(searchInput, "Skill A");

      expect(screen.queryByText("Skill B")).not.toBeInTheDocument();

      // Clear search
      await user.clear(searchInput);

      // Both skills should be visible again
      expect(screen.getByText("Skill A")).toBeInTheDocument();
      expect(screen.getByText("Skill B")).toBeInTheDocument();
    });
  });

  describe("Complete Filter Skills Flow", () => {
    it("user can filter by installation status", async () => {
      const user = userEvent.setup();
      const skills = [
        createSkill({
          key: "skill-1",
          name: "Installed Skill",
          installed: true,
        }),
        createSkill({
          key: "skill-2",
          name: "Uninstalled Skill",
          installed: false,
        }),
      ];

      getAllSkillsMock.mockResolvedValue({ skills });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByText("Installed Skill")).toBeInTheDocument();
        expect(screen.getByText("Uninstalled Skill")).toBeInTheDocument();
      });

      // Filter to installed only
      const statusFilter = screen.getByTestId("status-filter");
      await user.selectOptions(statusFilter, "installed");

      expect(screen.getByText("Installed Skill")).toBeInTheDocument();
      expect(screen.queryByText("Uninstalled Skill")).not.toBeInTheDocument();

      // Filter to uninstalled only
      await user.selectOptions(statusFilter, "uninstalled");

      expect(screen.queryByText("Installed Skill")).not.toBeInTheDocument();
      expect(screen.getByText("Uninstalled Skill")).toBeInTheDocument();

      // Show all
      await user.selectOptions(statusFilter, "all");

      expect(screen.getByText("Installed Skill")).toBeInTheDocument();
      expect(screen.getByText("Uninstalled Skill")).toBeInTheDocument();
    });

    it("user can combine search and filter", async () => {
      const user = userEvent.setup();
      const skills = [
        createSkill({ key: "skill-1", name: "Code Review", installed: true }),
        createSkill({
          key: "skill-2",
          name: "Code Generator",
          installed: false,
        }),
        createSkill({ key: "skill-3", name: "Test Runner", installed: true }),
      ];

      getAllSkillsMock.mockResolvedValue({ skills });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByTestId("skills-count")).toHaveTextContent(
          "3 skills",
        );
      });

      // Search for "Code"
      await user.type(screen.getByTestId("search-input"), "Code");

      expect(screen.getByTestId("skills-count")).toHaveTextContent("2 skills");

      // Then filter by installed
      await user.selectOptions(
        screen.getByTestId("status-filter"),
        "installed",
      );

      // Should only show "Code Review" (installed + matches "Code")
      expect(screen.getByText("Code Review")).toBeInTheDocument();
      expect(screen.queryByText("Code Generator")).not.toBeInTheDocument();
      expect(screen.queryByText("Test Runner")).not.toBeInTheDocument();
      expect(screen.getByTestId("skills-count")).toHaveTextContent("1 skills");
    });

    it("user can clear all filters", async () => {
      const user = userEvent.setup();
      const skills = [
        createSkill({ key: "skill-1", name: "Skill A", installed: true }),
        createSkill({ key: "skill-2", name: "Skill B", installed: false }),
      ];

      getAllSkillsMock.mockResolvedValue({ skills });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByTestId("skills-count")).toHaveTextContent(
          "2 skills",
        );
      });

      // Apply filters to get no results
      await user.type(screen.getByTestId("search-input"), "nonexistent");

      expect(screen.getByTestId("no-results")).toBeInTheDocument();

      // Click clear filters
      await user.click(screen.getByText("Clear filters"));

      // All skills should be visible again
      expect(screen.getByText("Skill A")).toBeInTheDocument();
      expect(screen.getByText("Skill B")).toBeInTheDocument();
    });
  });

  describe("Complete Install Skill Flow", () => {
    it("user can install a skill", async () => {
      const user = userEvent.setup();
      const skill = createSkill({
        key: "skill-1",
        name: "New Skill",
        directory: "skills/new-skill",
        installed: false,
      });

      // First call returns uninstalled, second call returns installed
      getAllSkillsMock
        .mockResolvedValueOnce({ skills: [skill] })
        .mockResolvedValueOnce({ skills: [{ ...skill, installed: true }] });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByText("New Skill")).toBeInTheDocument();
      });

      // 1. Verify skill is not installed
      const skillCard = screen.getByTestId("skill-card-skill-1");
      expect(
        within(skillCard).queryByTestId("installed-badge"),
      ).not.toBeInTheDocument();

      // 2. Click Install button
      await user.click(within(skillCard).getByText("Install"));

      // 3. Verify install API was called
      await waitFor(() => {
        expect(installSkillMock).toHaveBeenCalledWith("skills/new-skill");
      });

      // 4. Verify success toast
      expect(toastSuccessMock).toHaveBeenCalledWith("skills.installSuccess");

      // 5. Verify skills list was refreshed
      expect(getAllSkillsMock).toHaveBeenCalledTimes(2);
    });

    it("handles install error gracefully", async () => {
      const user = userEvent.setup();
      const consoleSpy = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});

      const skill = createSkill({
        key: "skill-1",
        name: "Skill",
        installed: false,
      });

      getAllSkillsMock.mockResolvedValue({ skills: [skill] });
      installSkillMock.mockRejectedValueOnce(new Error("Install failed"));

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByText("Skill")).toBeInTheDocument();
      });

      const skillCard = screen.getByTestId("skill-card-skill-1");
      await user.click(within(skillCard).getByText("Install"));

      await waitFor(() => {
        expect(installSkillMock).toHaveBeenCalled();
      });

      consoleSpy.mockRestore();
    });
  });

  describe("Complete Uninstall Skill Flow", () => {
    it("user can uninstall a skill", async () => {
      const user = userEvent.setup();
      const skill = createSkill({
        key: "skill-1",
        name: "Installed Skill",
        directory: "skills/installed-skill",
        installed: true,
      });

      // First call returns installed, second call returns uninstalled
      getAllSkillsMock
        .mockResolvedValueOnce({ skills: [skill] })
        .mockResolvedValueOnce({ skills: [{ ...skill, installed: false }] });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByText("Installed Skill")).toBeInTheDocument();
      });

      // 1. Verify skill is installed
      const skillCard = screen.getByTestId("skill-card-skill-1");
      expect(
        within(skillCard).getByTestId("installed-badge"),
      ).toBeInTheDocument();

      // 2. Click Uninstall button
      await user.click(within(skillCard).getByText("Uninstall"));

      // 3. Verify uninstall API was called
      await waitFor(() => {
        expect(uninstallSkillMock).toHaveBeenCalledWith(
          "skills/installed-skill",
        );
      });

      // 4. Verify success toast
      expect(toastSuccessMock).toHaveBeenCalledWith("skills.uninstallSuccess");

      // 5. Verify skills list was refreshed
      expect(getAllSkillsMock).toHaveBeenCalledTimes(2);
    });
  });

  describe("Complete Refresh Skills Flow", () => {
    it("user can refresh skills list", async () => {
      const user = userEvent.setup();

      const initialSkills = [
        createSkill({ key: "skill-1", name: "Initial Skill" }),
      ];
      const updatedSkills = [
        createSkill({ key: "skill-1", name: "Initial Skill" }),
        createSkill({ key: "skill-2", name: "New Skill" }),
      ];

      getAllSkillsMock
        .mockResolvedValueOnce({ skills: initialSkills })
        .mockResolvedValueOnce({ skills: updatedSkills });

      render(<SkillsManagementPage />);

      await waitFor(() => {
        expect(screen.getByText("Initial Skill")).toBeInTheDocument();
      });

      expect(screen.queryByText("New Skill")).not.toBeInTheDocument();

      // Click Refresh button
      await user.click(screen.getByText("Refresh"));

      // Wait for refresh to complete
      await waitFor(() => {
        expect(screen.getByText("New Skill")).toBeInTheDocument();
      });

      expect(getAllSkillsMock).toHaveBeenCalledTimes(2);
    });
  });

  describe("Complete User Journey: Install → Use → Uninstall", () => {
    it("full lifecycle of skill management", async () => {
      const user = userEvent.setup();

      const skill = createSkill({
        key: "code-review",
        name: "Code Review",
        description: "Automated code review tool",
        directory: "skills/code-review",
        installed: false,
      });

      let skillInstalled = false;

      getAllSkillsMock.mockImplementation(async () => ({
        skills: [{ ...skill, installed: skillInstalled }],
      }));

      installSkillMock.mockImplementation(async () => {
        skillInstalled = true;
        return true;
      });

      uninstallSkillMock.mockImplementation(async () => {
        skillInstalled = false;
        return true;
      });

      render(<SkillsManagementPage />);

      // Step 1: Browse and find skill
      await waitFor(() => {
        expect(screen.getByText("Code Review")).toBeInTheDocument();
      });

      // Step 2: Search to find specific skill
      await user.type(screen.getByTestId("search-input"), "code");
      expect(screen.getByText("Code Review")).toBeInTheDocument();

      // Step 3: Install the skill
      const skillCard = screen.getByTestId("skill-card-code-review");
      expect(
        within(skillCard).queryByTestId("installed-badge"),
      ).not.toBeInTheDocument();

      await user.click(within(skillCard).getByText("Install"));

      await waitFor(() => {
        expect(installSkillMock).toHaveBeenCalled();
      });

      // Step 4: Verify skill is now installed (after refresh)
      await waitFor(() => {
        expect(
          within(screen.getByTestId("skill-card-code-review")).getByTestId(
            "installed-badge",
          ),
        ).toBeInTheDocument();
      });

      // Step 5: Filter to show only installed skills
      await user.selectOptions(
        screen.getByTestId("status-filter"),
        "installed",
      );
      expect(screen.getByText("Code Review")).toBeInTheDocument();

      // Step 6: Uninstall the skill
      await user.click(
        within(screen.getByTestId("skill-card-code-review")).getByText(
          "Uninstall",
        ),
      );

      await waitFor(() => {
        expect(uninstallSkillMock).toHaveBeenCalled();
      });

      // Step 7: With "installed" filter still on, skill should disappear
      await waitFor(() => {
        expect(screen.queryByText("Code Review")).not.toBeInTheDocument();
      });

      // Step 8: Clear filters to see the skill again (uninstalled)
      await user.selectOptions(screen.getByTestId("status-filter"), "all");

      await waitFor(() => {
        expect(screen.getByText("Code Review")).toBeInTheDocument();
      });

      const finalSkillCard = screen.getByTestId("skill-card-code-review");
      expect(
        within(finalSkillCard).queryByTestId("installed-badge"),
      ).not.toBeInTheDocument();
    });
  });
});
