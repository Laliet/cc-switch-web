import { invoke } from "./adapter";
import type { AppId } from "./types";
import type { SkillStorageLocation } from "@/types";

export interface SkillCommand {
  name: string;
  description: string;
  filePath: string;
}

export interface Skill {
  key: string;
  name: string;
  description: string;
  directory: string;
  parentPath?: string;
  depth?: number;
  commands?: SkillCommand[];
  readmeUrl?: string;
  installed: boolean;
  installedApps?: string[];
  repoOwner?: string;
  repoName?: string;
  repoBranch?: string;
  skillsPath?: string; // 技能所在的子目录路径，如 "skills"
}

export interface SkillRepo {
  owner: string;
  name: string;
  branch: string;
  enabled: boolean;
  skillsPath?: string; // 可选：技能所在的子目录路径，如 "skills"
}

export interface SkillBackupEntry {
  backupId: string;
  backupPath: string;
  createdAt: string;
  app: string;
  directory: string;
  name: string;
  description: string;
  sourcePath: string;
}

export interface SkillUninstallResult {
  success: boolean;
  backup?: SkillBackupEntry;
}

export interface MigrationResult {
  migratedCount: number;
  skippedCount: number;
  errors: string[];
}

export interface SkillsResponse {
  skills: Skill[];
  warnings?: string[];
  cacheHit?: boolean;
  refreshing?: boolean;
}

const toBoolean = (value: unknown): boolean =>
  typeof value === "boolean" ? value : false;

const resolveSkillsApp = (app?: AppId): AppId | undefined =>
  app === "omo" || app === "omo-slim" ? "opencode" : app;

async function fileToBase64(file: File): Promise<string> {
  const buffer =
    typeof file.arrayBuffer === "function"
      ? await file.arrayBuffer()
      : await new Promise<ArrayBuffer>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => {
            if (reader.result instanceof ArrayBuffer) {
              resolve(reader.result);
            } else {
              reject(new Error("Failed to read ZIP file"));
            }
          };
          reader.onerror = () =>
            reject(reader.error ?? new Error("Failed to read ZIP file"));
          reader.readAsArrayBuffer(file);
        });
  const bytes = new Uint8Array(buffer);
  let binary = "";
  const chunkSize = 0x8000;
  for (let index = 0; index < bytes.length; index += chunkSize) {
    const chunk = bytes.subarray(index, index + chunkSize);
    binary += String.fromCharCode(...chunk);
  }
  return btoa(binary);
}

export const skillsApi = {
  async getAll(app?: AppId): Promise<SkillsResponse> {
    const targetApp = resolveSkillsApp(app);
    const result =
      targetApp !== undefined
        ? await invoke("get_skills", { app: targetApp })
        : await invoke("get_skills");

    if (Array.isArray(result)) {
      return {
        skills: result as Skill[],
        warnings: [],
        cacheHit: false,
        refreshing: false,
      };
    }

    const response =
      result && typeof result === "object"
        ? (result as Record<string, unknown>)
        : {};
    const cacheHitValue = response.cacheHit ?? response["cache_hit"];
    return {
      skills: Array.isArray(response.skills)
        ? (response.skills as Skill[])
        : [],
      warnings: Array.isArray(response.warnings)
        ? (response.warnings as string[])
        : [],
      cacheHit: toBoolean(cacheHitValue),
      refreshing: toBoolean(response.refreshing),
    };
  },

  async install(
    directory: string,
    force?: boolean,
    app?: AppId,
  ): Promise<boolean> {
    const targetApp = resolveSkillsApp(app);
    const payload: Record<string, unknown> = { directory };
    if (typeof force === "boolean") {
      payload.force = force;
    }
    if (targetApp) {
      payload.app = targetApp;
    }
    return await invoke("install_skill", payload);
  },

  async uninstall(
    directory: string,
    app?: AppId,
  ): Promise<SkillUninstallResult> {
    const targetApp = resolveSkillsApp(app);
    const result = await invoke(
      "uninstall_skill",
      targetApp ? { directory, app: targetApp } : { directory },
    );
    if (typeof result === "boolean") {
      return { success: result };
    }
    return result as SkillUninstallResult;
  },

  async getBackups(): Promise<SkillBackupEntry[]> {
    return await invoke("get_skill_backups");
  },

  async deleteBackup(backupId: string): Promise<boolean> {
    return await invoke("delete_skill_backup", { backupId });
  },

  async restoreBackup(
    backupId: string,
    app?: AppId,
    force?: boolean,
  ): Promise<SkillBackupEntry> {
    const targetApp = resolveSkillsApp(app);
    const payload: Record<string, unknown> = { backupId };
    if (targetApp) {
      payload.app = targetApp;
    }
    if (typeof force === "boolean") {
      payload.force = force;
    }
    return await invoke("restore_skill_backup", payload);
  },

  async installFromZipFile(
    file: File,
    app?: AppId,
    force?: boolean,
  ): Promise<Skill[]> {
    const targetApp = resolveSkillsApp(app);
    const payload: Record<string, unknown> = {
      contentBase64: await fileToBase64(file),
      fileName: file.name,
    };
    if (targetApp) {
      payload.app = targetApp;
    }
    if (typeof force === "boolean") {
      payload.force = force;
    }
    return await invoke("install_skills_from_zip", payload);
  },

  async installFromZipPath(
    filePath: string,
    app?: AppId,
    force?: boolean,
  ): Promise<Skill[]> {
    const targetApp = resolveSkillsApp(app);
    const payload: Record<string, unknown> = { filePath };
    if (targetApp) {
      payload.app = targetApp;
    }
    if (typeof force === "boolean") {
      payload.force = force;
    }
    return await invoke("install_skills_from_zip", payload);
  },

  async migrateStorage(target: SkillStorageLocation): Promise<MigrationResult> {
    return await invoke("migrate_skill_storage", { target });
  },

  async getRepos(): Promise<SkillRepo[]> {
    return await invoke("get_skill_repos");
  },

  async addRepo(repo: SkillRepo): Promise<boolean> {
    return await invoke("add_skill_repo", { repo });
  },

  async removeRepo(owner: string, name: string): Promise<boolean> {
    return await invoke("remove_skill_repo", { owner, name });
  },
};
