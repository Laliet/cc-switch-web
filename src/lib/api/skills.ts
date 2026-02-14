import { invoke } from "./adapter";
import type { AppId } from "./types";

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

export interface SkillsResponse {
  skills: Skill[];
  warnings?: string[];
  cacheHit?: boolean;
  refreshing?: boolean;
}

const toBoolean = (value: unknown): boolean =>
  typeof value === "boolean" ? value : false;

export const skillsApi = {
  async getAll(app?: AppId): Promise<SkillsResponse> {
    const result =
      app !== undefined
        ? await invoke("get_skills", { app })
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
      skills: Array.isArray(response.skills) ? (response.skills as Skill[]) : [],
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
    const payload: Record<string, unknown> = { directory };
    if (typeof force === "boolean") {
      payload.force = force;
    }
    if (app) {
      payload.app = app;
    }
    return await invoke("install_skill", payload);
  },

  async uninstall(directory: string, app?: AppId): Promise<boolean> {
    return await invoke("uninstall_skill", app ? { directory, app } : { directory });
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
