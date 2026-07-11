import { beforeEach, describe, expect, it, vi } from "vitest";
import { skillsApi } from "@/lib/api/skills";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api/adapter", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const skill = {
  key: "skill-1",
  name: "Sample Skill",
  description: "Skill description",
  directory: "skills/sample",
  installed: false,
};

describe("skills API module", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("getAll normalizes array response", async () => {
    invokeMock.mockResolvedValueOnce([skill]);

    const result = await skillsApi.getAll();

    expect(result).toEqual({
      skills: [skill],
      warnings: [],
      cacheHit: false,
      refreshing: false,
    });
    expect(invokeMock).toHaveBeenCalledWith("get_skills");
  });

  it("getAll handles non-object response", async () => {
    invokeMock.mockResolvedValueOnce(null);

    const result = await skillsApi.getAll();

    expect(result).toEqual({
      skills: [],
      warnings: [],
      cacheHit: false,
      refreshing: false,
    });
  });

  it("getAll returns cache metadata", async () => {
    invokeMock.mockResolvedValueOnce({
      skills: [skill],
      warnings: ["warn"],
      cacheHit: true,
      refreshing: true,
    });

    const result = await skillsApi.getAll();

    expect(result).toEqual({
      skills: [skill],
      warnings: ["warn"],
      cacheHit: true,
      refreshing: true,
    });
  });

  it("getAll reads snake_case cache metadata", async () => {
    invokeMock.mockResolvedValueOnce({
      skills: [skill],
      warnings: ["warn"],
      cache_hit: true,
      refreshing: false,
    });

    const result = await skillsApi.getAll();

    expect(result).toEqual({
      skills: [skill],
      warnings: ["warn"],
      cacheHit: true,
      refreshing: false,
    });
  });

  it("getAll defaults missing metadata", async () => {
    invokeMock.mockResolvedValueOnce({
      skills: [skill],
      warnings: [],
    });

    const result = await skillsApi.getAll();

    expect(result).toEqual({
      skills: [skill],
      warnings: [],
      cacheHit: false,
      refreshing: false,
    });
  });

  it("getAll forwards app parameter", async () => {
    invokeMock.mockResolvedValueOnce({ skills: [skill] });

    await skillsApi.getAll("codex");

    expect(invokeMock).toHaveBeenCalledWith("get_skills", { app: "codex" });
  });

  it("getAll maps OMO to OpenCode skills", async () => {
    invokeMock.mockResolvedValueOnce({ skills: [skill] });

    await skillsApi.getAll("omo");

    expect(invokeMock).toHaveBeenCalledWith("get_skills", {
      app: "opencode",
    });
  });

  it("install forwards directory, force and app", async () => {
    invokeMock.mockResolvedValueOnce(true);

    await skillsApi.install("skills/sample", true, "gemini");

    expect(invokeMock).toHaveBeenCalledWith("install_skill", {
      directory: "skills/sample",
      force: true,
      app: "gemini",
    });
  });

  it("install maps OMO to OpenCode skills", async () => {
    invokeMock.mockResolvedValueOnce(true);

    await skillsApi.install("skills/sample", undefined, "omo");

    expect(invokeMock).toHaveBeenCalledWith("install_skill", {
      directory: "skills/sample",
      app: "opencode",
    });
  });

  it("uninstall forwards directory and app", async () => {
    invokeMock.mockResolvedValueOnce(true);

    const result = await skillsApi.uninstall("skills/sample", "claude");

    expect(invokeMock).toHaveBeenCalledWith("uninstall_skill", {
      directory: "skills/sample",
      app: "claude",
    });
    expect(result).toEqual({ success: true });
  });

  it("uninstall maps OMO to OpenCode skills", async () => {
    invokeMock.mockResolvedValueOnce(true);

    await skillsApi.uninstall("skills/sample", "omo");

    expect(invokeMock).toHaveBeenCalledWith("uninstall_skill", {
      directory: "skills/sample",
      app: "opencode",
    });
  });

  it("lists and restores skill backups", async () => {
    invokeMock.mockResolvedValueOnce([
      {
        backupId: "backup-1",
        backupPath: "/tmp/backup-1",
        createdAt: "2026-06-27T00:00:00Z",
        app: "claude",
        directory: "skills/sample",
        name: "Sample",
        description: "",
        sourcePath: "/home/me/.claude/skills/sample",
      },
    ]);
    invokeMock.mockResolvedValueOnce({
      backupId: "backup-1",
      backupPath: "/tmp/backup-1",
      createdAt: "2026-06-27T00:00:00Z",
      app: "claude",
      directory: "skills/sample",
      name: "Sample",
      description: "",
      sourcePath: "/home/me/.claude/skills/sample",
    });

    await skillsApi.getBackups();
    await skillsApi.restoreBackup("backup-1", "omo", false);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_skill_backups");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "restore_skill_backup", {
      backupId: "backup-1",
      app: "opencode",
      force: false,
    });
  });

  it("imports skills from zip file content", async () => {
    invokeMock.mockResolvedValueOnce([
      {
        key: "local:demo",
        name: "Demo",
        description: "",
        directory: "demo",
        installed: true,
      },
    ]);
    const file = new File([new Uint8Array([1, 2, 3])], "demo.skill", {
      type: "application/zip",
    });

    await skillsApi.installFromZipFile(file, "claude", false);

    expect(invokeMock).toHaveBeenCalledWith("install_skills_from_zip", {
      contentBase64: "AQID",
      fileName: "demo.skill",
      app: "claude",
      force: false,
    });
  });

  it("migrates skill storage location", async () => {
    invokeMock.mockResolvedValueOnce({
      migratedCount: 2,
      skippedCount: 1,
      errors: [],
    });

    const result = await skillsApi.migrateStorage("unified");

    expect(result).toEqual({
      migratedCount: 2,
      skippedCount: 1,
      errors: [],
    });
    expect(invokeMock).toHaveBeenCalledWith("migrate_skill_storage", {
      target: "unified",
    });
  });

  it("checks, applies, and searches Skill updates", async () => {
    invokeMock.mockResolvedValue([]);

    await skillsApi.checkUpdates();
    await skillsApi.updateSkill("owner/repo:demo");
    await skillsApi.searchSkillsSh("review", 20, 40);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "check_skill_updates");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "update_skill", {
      id: "owner/repo:demo",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "search_skills_sh", {
      query: "review",
      limit: 20,
      offset: 40,
    });
  });

  it("installs a skills.sh result into the selected app", async () => {
    invokeMock.mockResolvedValue(true);
    const catalogSkill = {
      key: "owner/repo:demo",
      name: "Demo",
      directory: "demo",
      repoOwner: "owner",
      repoName: "repo",
      repoBranch: "main",
      installs: 42,
    };

    await skillsApi.installCatalogSkill(catalogSkill, "omo", false);

    expect(invokeMock).toHaveBeenCalledWith("install_catalog_skill", {
      directory: "demo",
      repoOwner: "owner",
      repoName: "repo",
      repoBranch: "main",
      app: "opencode",
      force: false,
    });
  });
});
