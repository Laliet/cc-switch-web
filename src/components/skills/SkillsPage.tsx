import {
  Component,
  useState,
  useEffect,
  useMemo,
  useRef,
  type ChangeEvent,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Archive,
  RefreshCw,
  RotateCcw,
  Settings,
  Trash2,
  Upload,
} from "lucide-react";
import { toast } from "sonner";
import { SkillCard } from "./SkillCard";
import { RepoManager } from "./RepoManager";
import {
  skillsApi,
  type Skill,
  type SkillBackupEntry,
  type SkillRepo,
} from "@/lib/api/skills";
import { formatSkillError } from "@/lib/errors/skillErrorParser";
import type { AppId } from "@/lib/api";
import { isSkillsApp, SKILLS_APPS } from "@/config/apps";

interface SkillsPageProps {
  onClose?: () => void;
  appId?: AppId;
}

const getRepoKey = (skill: Skill) => {
  if (skill.repoOwner && skill.repoName) {
    const branch = skill.repoBranch || "main";
    return `${skill.repoOwner}/${skill.repoName}@${branch}`;
  }
  return "__local__";
};

export function SkillsPage({ onClose: _onClose, appId }: SkillsPageProps = {}) {
  return (
    <SkillsErrorBoundary>
      <SkillsPageContent onClose={_onClose} appId={appId} />
    </SkillsErrorBoundary>
  );
}

function SkillsPageContent({ onClose: _onClose, appId }: SkillsPageProps = {}) {
  const [selectedApp, setSelectedApp] = useState<AppId>(() =>
    appId && isSkillsApp(appId) ? appId : "claude",
  );
  const currentApp: AppId = selectedApp;
  const { t } = useTranslation();
  const [skills, setSkills] = useState<Skill[]>([]);
  const [repos, setRepos] = useState<SkillRepo[]>([]);
  const [loading, setLoading] = useState(true);
  const [cacheStatus, setCacheStatus] = useState({
    cacheHit: false,
    refreshing: false,
  });
  const loadSkillsRequestId = useRef(0);
  const isMountedRef = useRef(true);
  const zipInputRef = useRef<HTMLInputElement | null>(null);
  const [repoManagerOpen, setRepoManagerOpen] = useState(false);
  const [skillBackups, setSkillBackups] = useState<SkillBackupEntry[]>([]);
  const [backupActionId, setBackupActionId] = useState<string | null>(null);
  const [zipImporting, setZipImporting] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [installFilter, setInstallFilter] = useState<
    "all" | "installed" | "uninstalled"
  >("all");
  const [repoFilter, setRepoFilter] = useState("all");
  const [groupByDepth, setGroupByDepth] = useState(false);

  const repoOptions = useMemo(() => {
    const options = new Map<string, string>();
    skills.forEach((skill) => {
      const key = getRepoKey(skill);
      if (key === "__local__") {
        options.set(key, t("skills.repo.local", { defaultValue: "本地" }));
        return;
      }
      const [repo, branch] = key.split("@");
      const label = branch && branch !== "main" ? `${repo}@${branch}` : repo;
      options.set(key, label);
    });

    const sorted = Array.from(options.entries()).sort((a, b) =>
      a[1].localeCompare(b[1]),
    );

    return [
      {
        value: "all",
        label: t("skills.filter.allRepos", { defaultValue: "全部仓库" }),
      },
      ...sorted.map(([value, label]) => ({ value, label })),
    ];
  }, [skills, t]);

  useEffect(() => {
    if (
      repoFilter !== "all" &&
      !repoOptions.some((option) => option.value === repoFilter)
    ) {
      setRepoFilter("all");
    }
  }, [repoFilter, repoOptions]);

  useEffect(() => {
    return () => {
      isMountedRef.current = false;
      loadSkillsRequestId.current += 1;
    };
  }, []);

  const filteredSkills = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    return skills.filter((skill) => {
      if (installFilter === "installed" && !skill.installed) {
        return false;
      }
      if (installFilter === "uninstalled" && skill.installed) {
        return false;
      }
      if (repoFilter !== "all" && getRepoKey(skill) !== repoFilter) {
        return false;
      }
      if (!query) {
        return true;
      }
      const haystack = `${skill.name} ${skill.description || ""}`.toLowerCase();
      return haystack.includes(query);
    });
  }, [skills, searchQuery, installFilter, repoFilter]);

  const groupedSkills = useMemo(() => {
    if (!groupByDepth) {
      return null;
    }
    const groups = new Map<
      string,
      { key: string; label: string; depth: number | null; items: Skill[] }
    >();

    filteredSkills.forEach((skill) => {
      const depthValue =
        typeof skill.depth === "number" && !Number.isNaN(skill.depth)
          ? Math.max(0, skill.depth)
          : null;
      const key = depthValue === null ? "unknown" : String(depthValue);
      if (!groups.has(key)) {
        const label =
          depthValue === null
            ? t("skills.depthUnknown", { defaultValue: "深度未知" })
            : t("skills.depthGroup", {
                depth: depthValue,
                defaultValue: `深度 ${depthValue}`,
              });
        groups.set(key, {
          key,
          label,
          depth: depthValue,
          items: [],
        });
      }
      groups.get(key)?.items.push(skill);
    });

    return Array.from(groups.values()).sort((a, b) => {
      const depthA = a.depth ?? Number.POSITIVE_INFINITY;
      const depthB = b.depth ?? Number.POSITIVE_INFINITY;
      if (depthA === depthB) {
        return a.label.localeCompare(b.label);
      }
      return depthA - depthB;
    });
  }, [filteredSkills, groupByDepth, t]);

  const statusLabel = useMemo(() => {
    if (cacheStatus.refreshing) {
      return t("skills.cacheStatus.refreshing", {
        defaultValue: "Background refresh",
      });
    }
    if (cacheStatus.cacheHit) {
      return t("skills.cacheStatus.hit", {
        defaultValue: "Cache hit",
      });
    }
    return "";
  }, [cacheStatus.cacheHit, cacheStatus.refreshing, t]);

  const hasActiveFilters =
    searchQuery.trim().length > 0 ||
    installFilter !== "all" ||
    repoFilter !== "all";
  const isOmoSkillsView = currentApp === "omo";

  const handleClearFilters = () => {
    setSearchQuery("");
    setInstallFilter("all");
    setRepoFilter("all");
  };

  const loadSkills = async (
    afterLoad?: (data: Skill[]) => void,
    options?: { suppressErrorToast?: boolean },
  ): Promise<{
    ok: boolean;
    stale?: boolean;
    errorMessage?: string;
    formattedError?: { title: string; description: string };
  }> => {
    const requestId = ++loadSkillsRequestId.current;
    try {
      setLoading(true);
      const {
        skills: data,
        warnings,
        cacheHit = false,
        refreshing = false,
      } = await skillsApi.getAll(currentApp);
      const isLatestRequest = requestId === loadSkillsRequestId.current;
      if (isLatestRequest && isMountedRef.current) {
        setSkills(data);
        setCacheStatus({ cacheHit, refreshing });
      }
      if (afterLoad && isLatestRequest && isMountedRef.current) {
        afterLoad(data);
      }
      if (
        isLatestRequest &&
        isMountedRef.current &&
        warnings &&
        warnings.length > 0
      ) {
        toast.warning(
          t("skills.repo.fetchWarning", {
            defaultValue: "部分技能仓库获取失败，已显示本地技能",
          }),
          {
            description: warnings.join("\n"),
            duration: 8000,
          },
        );
      }
      return { ok: true, stale: !isLatestRequest };
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      const isLatestRequest = requestId === loadSkillsRequestId.current;

      // 传入 "skills.loadFailed" 作为标题
      const formattedError = formatSkillError(
        errorMessage,
        t,
        "skills.loadFailed",
      );

      if (
        !options?.suppressErrorToast &&
        isLatestRequest &&
        isMountedRef.current
      ) {
        toast.error(formattedError.title, {
          description: formattedError.description,
          duration: 8000,
        });
      }

      if (isLatestRequest && isMountedRef.current) {
        console.error("Load skills failed:", error);
        setCacheStatus({ cacheHit: false, refreshing: false });
        return { ok: false, errorMessage, formattedError };
      }
      return { ok: true, stale: true };
    } finally {
      if (requestId === loadSkillsRequestId.current && isMountedRef.current) {
        setLoading(false);
      }
    }
  };

  const loadRepos = async (): Promise<{
    ok: boolean;
    errorMessage?: string;
  }> => {
    try {
      const data = await skillsApi.getRepos();
      if (isMountedRef.current) {
        setRepos(data);
      }
      return { ok: true };
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      console.error("Failed to load repos:", error);
      return { ok: false, errorMessage };
    }
  };

  const loadBackups = async () => {
    try {
      const backups = await skillsApi.getBackups();
      if (isMountedRef.current) {
        setSkillBackups(backups);
      }
    } catch (error) {
      console.error("Failed to load skill backups:", error);
    }
  };

  useEffect(() => {
    Promise.all([loadSkills(), loadRepos(), loadBackups()]);
  }, [currentApp]);

  const handleInstall = async (directory: string) => {
    const targetSkill = skills.find((item) => item.directory === directory);
    const otherInstalledApps = (targetSkill?.installedApps ?? []).filter(
      (app) => app !== currentApp,
    );
    if (otherInstalledApps.length > 0) {
      const otherAppNames = otherInstalledApps
        .map((app) =>
          t(`apps.${app}`, {
            defaultValue: app,
          }),
        )
        .join(" / ");
      toast.warning(
        t("skills.crossAppInstallHintTitle", {
          defaultValue: "该技能已安装到其他客户端",
        }),
        {
          description: t("skills.crossAppInstallHintDescription", {
            targetApp: t(`apps.${currentApp}`, {
              defaultValue: currentApp,
            }),
            installedApps: otherAppNames,
            defaultValue:
              "当前会继续安装到 {{targetApp}}。已安装客户端：{{installedApps}}",
          }),
          duration: 7000,
        },
      );
    }

    try {
      await skillsApi.install(directory, undefined, currentApp);
      toast.success(t("skills.installSuccess", { name: directory }));
      await loadSkills();
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);

      // 使用错误解析器格式化错误，传入 "skills.installFailed"
      const { title, description } = formatSkillError(
        errorMessage,
        t,
        "skills.installFailed",
      );

      toast.error(title, {
        description,
        duration: 10000, // 延长显示时间让用户看清
      });

      // 打印到控制台方便调试
      console.error("Install skill failed:", {
        directory,
        error,
        message: errorMessage,
      });
    }
  };

  const handleUninstall = async (directory: string) => {
    try {
      const result = await skillsApi.uninstall(directory, currentApp);
      toast.success(t("skills.uninstallSuccess", { name: directory }), {
        description: result.backup
          ? t("skills.backup.created", {
              defaultValue: "卸载前已创建备份：{{path}}",
              path: result.backup.backupPath,
            })
          : undefined,
        duration: result.backup ? 8000 : undefined,
      });
      await Promise.all([loadSkills(), loadBackups()]);
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);

      // 使用错误解析器格式化错误，传入 "skills.uninstallFailed"
      const { title, description } = formatSkillError(
        errorMessage,
        t,
        "skills.uninstallFailed",
      );

      toast.error(title, {
        description,
        duration: 10000,
      });

      console.error("Uninstall skill failed:", {
        directory,
        error,
        message: errorMessage,
      });
    }
  };

  const handleRestoreBackup = async (backup: SkillBackupEntry) => {
    setBackupActionId(backup.backupId);
    try {
      await skillsApi.restoreBackup(backup.backupId, currentApp, false);
      toast.success(
        t("skills.backup.restoreSuccess", {
          defaultValue: "Skill 已从备份恢复",
        }),
        {
          description: backup.name || backup.directory,
        },
      );
      await Promise.all([loadSkills(), loadBackups()]);
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      const { title, description } = formatSkillError(
        errorMessage,
        t,
        "skills.backup.restoreFailed",
      );
      toast.error(title, { description, duration: 10000 });
    } finally {
      if (isMountedRef.current) {
        setBackupActionId(null);
      }
    }
  };

  const handleDeleteBackup = async (backup: SkillBackupEntry) => {
    setBackupActionId(backup.backupId);
    try {
      await skillsApi.deleteBackup(backup.backupId);
      toast.success(
        t("skills.backup.deleteSuccess", {
          defaultValue: "Skill 备份已删除",
        }),
      );
      await loadBackups();
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      const { title, description } = formatSkillError(
        errorMessage,
        t,
        "skills.backup.deleteFailed",
      );
      toast.error(title, { description, duration: 10000 });
    } finally {
      if (isMountedRef.current) {
        setBackupActionId(null);
      }
    }
  };

  const handleZipSelected = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;

    setZipImporting(true);
    try {
      const installed = await skillsApi.installFromZipFile(
        file,
        currentApp,
        false,
      );
      toast.success(
        t("skills.zip.importSuccess", {
          defaultValue: "已导入 {{count}} 个 Skill",
          count: installed.length,
        }),
        {
          description: installed.map((skill) => skill.name).join(", "),
        },
      );
      await loadSkills();
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      const { title, description } = formatSkillError(
        errorMessage,
        t,
        "skills.zip.importFailed",
      );
      toast.error(title, { description, duration: 10000 });
    } finally {
      if (isMountedRef.current) {
        setZipImporting(false);
      }
    }
  };

  const handleAddRepo = async (repo: SkillRepo) => {
    try {
      await skillsApi.addRepo(repo);
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      const { title, description } = formatSkillError(
        errorMessage,
        t,
        "skills.repo.addFailed",
      );

      toast.error(title, {
        description,
        duration: 10000,
      });

      console.error("Add repo failed:", {
        repo,
        error,
        message: errorMessage,
      });

      throw new Error(description);
    }

    let repoSkillCount = 0;
    const [reposResult, skillsResult] = await Promise.all([
      loadRepos(),
      loadSkills(
        (data) => {
          repoSkillCount = data.filter(
            (skill) =>
              skill.repoOwner === repo.owner &&
              skill.repoName === repo.name &&
              (skill.repoBranch || "main") === (repo.branch || "main"),
          ).length;
        },
        { suppressErrorToast: true },
      ),
    ]);

    if (skillsResult.ok) {
      toast.success(
        t("skills.repo.addSuccess", {
          owner: repo.owner,
          name: repo.name,
          count: repoSkillCount,
        }),
      );
    } else {
      toast.success(
        t("skills.repo.addSuccessSimple", {
          owner: repo.owner,
          name: repo.name,
        }),
      );
    }

    if (!reposResult.ok || !skillsResult.ok) {
      const refreshDescription = [
        !skillsResult.ok ? skillsResult.formattedError?.description : undefined,
        !reposResult.ok ? reposResult.errorMessage : undefined,
      ]
        .filter(Boolean)
        .join("\n");

      toast.warning(
        t("skills.repo.refreshFailed"),
        refreshDescription
          ? { description: refreshDescription, duration: 8000 }
          : { duration: 8000 },
      );
    }
  };

  const handleRemoveRepo = async (owner: string, name: string) => {
    try {
      await skillsApi.removeRepo(owner, name);
      toast.success(t("skills.repo.removeSuccess", { owner, name }));
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      const { title, description } = formatSkillError(
        errorMessage,
        t,
        "skills.repo.removeFailed",
      );
      toast.error(title, {
        description,
        duration: 10000,
      });
      console.error("Remove repo failed:", {
        owner,
        name,
        error,
        message: errorMessage,
      });
      return;
    }

    const [reposResult, skillsResult] = await Promise.all([
      loadRepos(),
      loadSkills(undefined, { suppressErrorToast: true }),
    ]);

    if (!reposResult.ok || !skillsResult.ok) {
      const refreshDescription = [
        !skillsResult.ok ? skillsResult.formattedError?.description : undefined,
        !reposResult.ok ? reposResult.errorMessage : undefined,
      ]
        .filter(Boolean)
        .join("\n");

      toast.warning(
        t("skills.repo.refreshFailed"),
        refreshDescription
          ? { description: refreshDescription, duration: 8000 }
          : { duration: 8000 },
      );
    }
  };

  return (
    <div className="flex flex-col h-full min-h-0 bg-background">
      {/* 顶部操作栏（固定区域） */}
      <div className="flex-shrink-0 border-b border-border-default bg-muted/20 px-6 py-4">
        <div className="flex items-center justify-between pr-8">
          <h1 className="text-lg font-semibold leading-tight tracking-tight text-gray-900 dark:text-gray-100">
            {t("skills.title")}
          </h1>
          <div className="flex gap-2">
            <input
              ref={zipInputRef}
              type="file"
              accept=".zip,.skill,application/zip"
              className="hidden"
              onChange={handleZipSelected}
            />
            <Button
              variant="mcp"
              size="sm"
              onClick={() => zipInputRef.current?.click()}
              disabled={zipImporting}
            >
              <Upload className="h-4 w-4 mr-2" />
              {zipImporting
                ? t("skills.zip.importing", {
                    defaultValue: "导入中",
                  })
                : t("skills.zip.import", {
                    defaultValue: "导入 ZIP/.skill",
                  })}
            </Button>
            <Button
              variant="mcp"
              size="sm"
              onClick={() => loadSkills()}
              disabled={loading}
            >
              <RefreshCw
                className={`h-4 w-4 mr-2 ${loading ? "animate-spin" : ""}`}
              />
              {loading ? t("skills.refreshing") : t("skills.refresh")}
            </Button>
            <Button
              variant="mcp"
              size="sm"
              onClick={() => setRepoManagerOpen(true)}
            >
              <Settings className="h-4 w-4 mr-2" />
              {t("skills.repoManager")}
            </Button>
          </div>
        </div>

        {/* 描述 */}
        <p className="mt-1.5 text-sm text-gray-500 dark:text-gray-400">
          {t("skills.description")}
        </p>
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <span className="text-sm text-muted-foreground">
            {t("skills.targetApp", { defaultValue: "安装目标客户端" })}
          </span>
          {SKILLS_APPS.map((app) => (
            <Button
              key={app}
              variant={currentApp === app ? "default" : "mcp"}
              size="sm"
              onClick={() => setSelectedApp(app)}
            >
              {t(`apps.${app}`, { defaultValue: app })}
            </Button>
          ))}
        </div>
        {isOmoSkillsView ? (
          <p className="mt-2 text-xs text-muted-foreground">
            {t("skills.omoUsesOpencode", {
              defaultValue:
                "OMO 暂无独立 Skills 目录，此处复用 OpenCode 的 ~/.config/opencode/skills。",
            })}
          </p>
        ) : null}
        {statusLabel && (
          <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
            {statusLabel}
          </p>
        )}

        {/* 搜索与过滤 */}
        <div className="mt-4 flex flex-col gap-3 lg:flex-row lg:items-center">
          <div className="flex-1 min-w-[220px]">
            <Input
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder={t("skills.searchPlaceholder", {
                defaultValue: "搜索技能名称或描述",
              })}
            />
          </div>
          <div className="flex flex-1 flex-col gap-3 sm:flex-row sm:items-center lg:justify-end">
            <Select
              value={installFilter}
              onValueChange={(value) =>
                setInstallFilter(value as "all" | "installed" | "uninstalled")
              }
            >
              <SelectTrigger className="h-9 w-full sm:w-[170px]">
                <SelectValue
                  placeholder={t("skills.filter.installStatus", {
                    defaultValue: "安装状态",
                  })}
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">
                  {t("skills.filter.all", { defaultValue: "全部" })}
                </SelectItem>
                <SelectItem value="installed">
                  {t("skills.filter.installed", {
                    defaultValue: "已安装",
                  })}
                </SelectItem>
                <SelectItem value="uninstalled">
                  {t("skills.filter.uninstalled", {
                    defaultValue: "未安装",
                  })}
                </SelectItem>
              </SelectContent>
            </Select>
            <Select value={repoFilter} onValueChange={setRepoFilter}>
              <SelectTrigger className="h-9 w-full sm:w-[220px]">
                <SelectValue
                  placeholder={t("skills.filter.repo", {
                    defaultValue: "仓库",
                  })}
                />
              </SelectTrigger>
              <SelectContent>
                {repoOptions.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <label className="flex items-center gap-2 text-sm text-muted-foreground">
              <Switch
                checked={groupByDepth}
                onCheckedChange={setGroupByDepth}
                aria-label={t("skills.groupByDepth", {
                  defaultValue: "按深度分组展示",
                })}
              />
              <span>
                {t("skills.groupByDepth", {
                  defaultValue: "按深度分组展示",
                })}
              </span>
            </label>
          </div>
        </div>
      </div>

      {/* 技能网格（可滚动详情区域） */}
      <div className="flex-1 min-h-0 overflow-y-auto px-6 py-6 bg-muted/10">
        {skillBackups.length > 0 ? (
          <section className="mb-6 rounded-md border border-border-default bg-card p-4">
            <div className="mb-3 flex items-center justify-between gap-3">
              <div className="flex items-center gap-2">
                <Archive className="h-4 w-4 text-muted-foreground" />
                <h2 className="text-sm font-semibold">
                  {t("skills.backup.title", {
                    defaultValue: "Skill 备份",
                  })}
                </h2>
              </div>
              <span className="text-xs text-muted-foreground">
                {skillBackups.length}
              </span>
            </div>
            <div className="grid gap-2 md:grid-cols-2 xl:grid-cols-3">
              {skillBackups.slice(0, 6).map((backup) => (
                <div
                  key={backup.backupId}
                  className="flex min-w-0 flex-col gap-2 rounded-md border border-border-default p-3"
                >
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium">
                      {backup.name || backup.directory}
                    </div>
                    <div className="mt-1 truncate text-xs text-muted-foreground">
                      {backup.app} · {backup.directory}
                    </div>
                    <div className="mt-1 truncate text-xs text-muted-foreground">
                      {backup.createdAt}
                    </div>
                  </div>
                  <div className="flex gap-2">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => handleRestoreBackup(backup)}
                      disabled={backupActionId !== null}
                      className="flex-1"
                    >
                      <RotateCcw className="mr-1.5 h-3.5 w-3.5" />
                      {t("skills.backup.restore", {
                        defaultValue: "恢复",
                      })}
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => handleDeleteBackup(backup)}
                      disabled={backupActionId !== null}
                      className="border-red-200 text-red-600 hover:bg-red-50 hover:text-red-700 dark:border-red-900/50 dark:text-red-400"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          </section>
        ) : null}
        {loading ? (
          <div className="flex items-center justify-center h-64">
            <RefreshCw className="h-8 w-8 animate-spin text-muted-foreground" />
          </div>
        ) : skills.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-64 text-center">
            <p className="text-lg font-medium text-gray-900 dark:text-gray-100">
              {t("skills.empty")}
            </p>
            <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">
              {t("skills.emptyDescription")}
            </p>
            <Button
              variant="link"
              onClick={() => setRepoManagerOpen(true)}
              className="mt-3 text-sm font-normal"
            >
              {t("skills.addRepo")}
            </Button>
          </div>
        ) : filteredSkills.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-64 text-center">
            <p className="text-lg font-medium text-gray-900 dark:text-gray-100">
              {t("skills.noResults", { defaultValue: "未找到匹配的技能" })}
            </p>
            <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">
              {t("skills.noResultsDescription", {
                defaultValue: "请调整搜索或过滤条件后重试",
              })}
            </p>
            {hasActiveFilters && (
              <Button
                variant="link"
                onClick={handleClearFilters}
                className="mt-3 text-sm font-normal"
              >
                {t("skills.clearFilters", { defaultValue: "清除筛选" })}
              </Button>
            )}
          </div>
        ) : (
          <>
            {groupByDepth && groupedSkills ? (
              <div className="space-y-6">
                {groupedSkills.map((group) => (
                  <div key={group.key} className="space-y-3">
                    <div className="text-sm font-semibold text-muted-foreground">
                      {group.label}
                    </div>
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                      {group.items.map((skill) => (
                        <SkillCard
                          key={skill.key}
                          skill={skill}
                          onInstall={handleInstall}
                          onUninstall={handleUninstall}
                        />
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {filteredSkills.map((skill) => (
                  <SkillCard
                    key={skill.key}
                    skill={skill}
                    onInstall={handleInstall}
                    onUninstall={handleUninstall}
                  />
                ))}
              </div>
            )}
          </>
        )}
      </div>

      {/* 仓库管理对话框 */}
      <RepoManager
        open={repoManagerOpen}
        onOpenChange={setRepoManagerOpen}
        repos={repos}
        skills={skills}
        onAdd={handleAddRepo}
        onRemove={handleRemoveRepo}
      />
    </div>
  );
}

interface SkillsErrorBoundaryProps {
  children: ReactNode;
}

interface SkillsErrorBoundaryState {
  hasError: boolean;
}

class SkillsErrorBoundary extends Component<
  SkillsErrorBoundaryProps,
  SkillsErrorBoundaryState
> {
  state: SkillsErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): SkillsErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: unknown) {
    console.error("SkillsPage crashed:", error);
  }

  handleRetry = () => {
    this.setState({ hasError: false });
  };

  render() {
    if (this.state.hasError) {
      return <SkillsErrorFallback onRetry={this.handleRetry} />;
    }
    return this.props.children;
  }
}

interface SkillsErrorFallbackProps {
  onRetry: () => void;
}

function SkillsErrorFallback({ onRetry }: SkillsErrorFallbackProps) {
  const { t } = useTranslation();

  return (
    <div className="flex h-full flex-col items-center justify-center bg-background px-6 text-center">
      <p className="text-lg font-medium text-gray-900 dark:text-gray-100">
        {t("skills.errorBoundaryTitle", {
          defaultValue: "技能页面出现错误",
        })}
      </p>
      <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">
        {t("skills.errorBoundaryDescription", {
          defaultValue: "请重试或刷新页面。",
        })}
      </p>
      <Button variant="mcp" size="sm" onClick={onRetry} className="mt-3">
        {t("skills.errorBoundaryRetry", { defaultValue: "重试" })}
      </Button>
    </div>
  );
}
