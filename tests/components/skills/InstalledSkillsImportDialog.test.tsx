import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { InstalledSkillsImportDialog } from "@/components/skills/InstalledSkillsImportDialog";
import type { InstalledSkillDiscovery } from "@/lib/api/skills";

const discoverInstalledMock = vi.hoisted(() => vi.fn());
const importInstalledMock = vi.hoisted(() => vi.fn());
const toastSuccessMock = vi.hoisted(() => vi.fn());
const toastErrorMock = vi.hoisted(() => vi.fn());
const toastWarningMock = vi.hoisted(() => vi.fn());

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      String(options?.defaultValue ?? key),
  }),
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
    discoverInstalled: (...args: unknown[]) => discoverInstalledMock(...args),
    importInstalled: (...args: unknown[]) => importInstalledMock(...args),
  },
}));

const discovery = (
  overrides: Partial<InstalledSkillDiscovery> = {},
): InstalledSkillDiscovery => ({
  directory: "demo",
  name: "Demo",
  description: "Existing Skill",
  sources: [
    {
      source: "claude",
      path: "/home/me/.claude/skills/demo",
      contentHash: "source-hash",
      matchesTarget: false,
    },
  ],
  targetPath: "/home/me/.cc-switch/skills/demo",
  status: "new",
  managedApps: [],
  ...overrides,
});

beforeEach(() => {
  discoverInstalledMock.mockReset();
  importInstalledMock.mockReset();
  toastSuccessMock.mockClear();
  toastErrorMock.mockClear();
  toastWarningMock.mockClear();
});

describe("InstalledSkillsImportDialog", () => {
  it("previews trusted paths and imports the selected Skill", async () => {
    const onImported = vi.fn().mockResolvedValue(undefined);
    discoverInstalledMock
      .mockResolvedValueOnce([discovery()])
      .mockResolvedValueOnce([]);
    importInstalledMock.mockResolvedValueOnce([
      {
        directory: "demo",
        source: "claude",
        targetPath: "/home/me/.cc-switch/skills/demo",
        status: "imported",
        enabledApps: ["claude"],
      },
    ]);
    const user = userEvent.setup();

    render(
      <InstalledSkillsImportDialog
        open
        onOpenChange={vi.fn()}
        currentApp="claude"
        onImported={onImported}
      />,
    );

    expect(await screen.findByText("Demo")).toBeInTheDocument();
    expect(
      screen.getByText("/home/me/.claude/skills/demo"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("/home/me/.cc-switch/skills/demo"),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: "选择 Demo" }));
    await user.click(
      screen.getByRole("button", { name: "导入所选 (1)" }),
    );

    await waitFor(() =>
      expect(importInstalledMock).toHaveBeenCalledWith([
        {
          directory: "demo",
          source: "claude",
          apps: ["claude"],
          overwrite: false,
        },
      ]),
    );
    expect(onImported).toHaveBeenCalledTimes(1);
  });

  it("requires explicit confirmation before overwriting a conflict", async () => {
    discoverInstalledMock
      .mockResolvedValueOnce([discovery({ status: "conflict" })])
      .mockResolvedValueOnce([]);
    importInstalledMock.mockResolvedValueOnce([
      {
        directory: "demo",
        source: "claude",
        targetPath: "/home/me/.cc-switch/skills/demo",
        status: "imported",
        enabledApps: ["claude"],
      },
    ]);
    const user = userEvent.setup();

    render(
      <InstalledSkillsImportDialog
        open
        onOpenChange={vi.fn()}
        currentApp="claude"
        onImported={vi.fn()}
      />,
    );

    await screen.findByText("Demo");
    await user.click(screen.getByRole("checkbox", { name: "选择 Demo" }));
    const importButton = screen.getByRole("button", {
      name: "导入所选 (1)",
    });
    expect(importButton).toBeDisabled();

    await user.click(
      screen.getByRole("checkbox", {
        name: "使用所选来源覆盖统一存储中的不同内容",
      }),
    );
    expect(importButton).toBeEnabled();
    await user.click(importButton);

    await waitFor(() =>
      expect(importInstalledMock).toHaveBeenCalledWith([
        {
          directory: "demo",
          source: "claude",
          apps: ["claude"],
          overwrite: true,
        },
      ]),
    );
  });
});
