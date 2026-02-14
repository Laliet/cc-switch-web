import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { skillsApi, type SkillRepo, type SkillsResponse } from "@/lib/api/skills";
import type { AppId } from "@/lib/api";

/**
 * 查询所有技能
 */
export function useAllSkills(app: AppId = "claude") {
  return useQuery<SkillsResponse>({
    queryKey: ["skills", "all", app],
    queryFn: () => skillsApi.getAll(app),
  });
}

/**
 * 查询技能仓库列表
 */
export function useSkillRepos() {
  return useQuery<SkillRepo[]>({
    queryKey: ["skills", "repos"],
    queryFn: () => skillsApi.getRepos(),
  });
}

/**
 * 安装技能
 */
export function useInstallSkill(app: AppId = "claude") {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { directory: string; force?: boolean } | string) => {
      if (typeof input === "string") {
        return skillsApi.install(input, undefined, app);
      }
      return skillsApi.install(input.directory, input.force, app);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["skills", "all", app] });
    },
  });
}

/**
 * 卸载技能
 */
export function useUninstallSkill(app: AppId = "claude") {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (directory: string) => skillsApi.uninstall(directory, app),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["skills", "all", app] });
    },
  });
}

/**
 * 添加技能仓库
 */
export function useAddSkillRepo() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (repo: SkillRepo) => skillsApi.addRepo(repo),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["skills", "repos"] });
      queryClient.invalidateQueries({ queryKey: ["skills", "all"] });
    },
  });
}

/**
 * 删除技能仓库
 */
export function useRemoveSkillRepo() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ owner, name }: { owner: string; name: string }) =>
      skillsApi.removeRepo(owner, name),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["skills", "repos"] });
      queryClient.invalidateQueries({ queryKey: ["skills", "all"] });
    },
  });
}
