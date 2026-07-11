import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  ExternalLink,
  Download,
  Trash2,
  Loader2,
  ChevronDown,
  RefreshCw,
} from "lucide-react";
import { settingsApi } from "@/lib/api";
import type { Skill } from "@/lib/api/skills";
import type { AppId } from "@/lib/api";
import { Switch } from "@/components/ui/switch";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";

interface SkillCardProps {
  skill: Skill;
  onInstall: (directory: string) => Promise<void>;
  onUninstall: (directory: string) => Promise<void>;
  onUpdate?: () => Promise<void>;
  onToggleApp?: (app: AppId, enabled: boolean) => Promise<void>;
  hasUpdate?: boolean;
  updating?: boolean;
  installs?: number;
}

const MANAGED_SKILL_APPS: AppId[] = ["claude", "codex", "gemini", "opencode"];

export function SkillCard({
  skill,
  onInstall,
  onUninstall,
  onUpdate,
  onToggleApp,
  hasUpdate = false,
  updating = false,
  installs,
}: SkillCardProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const isMountedRef = useRef(true);

  useEffect(() => {
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const handleInstall = async () => {
    setLoading(true);
    try {
      await onInstall(skill.directory);
    } finally {
      if (isMountedRef.current) {
        setLoading(false);
      }
    }
  };

  const handleUninstall = async () => {
    setLoading(true);
    try {
      await onUninstall(skill.directory);
    } finally {
      if (isMountedRef.current) {
        setLoading(false);
      }
    }
  };

  const handleOpenGithub = async () => {
    if (skill.readmeUrl) {
      try {
        await settingsApi.openExternal(skill.readmeUrl);
      } catch (error) {
        console.error("Failed to open URL:", error);
      }
    }
  };

  const showDirectory =
    Boolean(skill.directory) &&
    skill.directory.trim().toLowerCase() !== skill.name.trim().toLowerCase();
  const parentPath = skill.parentPath?.trim();
  const depth =
    typeof skill.depth === "number" && !Number.isNaN(skill.depth)
      ? Math.max(0, skill.depth)
      : null;
  const indentStyle =
    depth !== null && depth > 0
      ? { paddingLeft: `${24 + depth * 12}px` }
      : undefined;
  const commands = skill.commands ?? [];
  const hasCommands = commands.length > 0;

  return (
    <Card className="flex flex-col h-full border-border-default bg-card transition-[border-color,box-shadow] duration-200 hover:border-border-hover hover:shadow-md">
      <CardHeader className="pb-3" style={indentStyle}>
        <div className="flex items-start justify-between gap-2">
          <div className="flex-1 min-w-0">
            <CardTitle className="text-base font-semibold truncate">
              {skill.name}
            </CardTitle>
            <div className="flex items-center gap-2 mt-1.5">
              {showDirectory && (
                <CardDescription className="text-xs truncate">
                  {skill.directory}
                </CardDescription>
              )}
              {skill.repoOwner && skill.repoName && (
                <Badge
                  variant="outline"
                  className="shrink-0 text-[10px] px-1.5 py-0 h-4 border-border-default"
                >
                  {skill.repoOwner}/{skill.repoName}
                </Badge>
              )}
            </div>
          </div>
          {skill.installed && (
            <Badge
              variant="default"
              className="shrink-0 bg-green-600/90 hover:bg-green-600 dark:bg-green-700/90 dark:hover:bg-green-700 text-white border-0"
            >
              {t("skills.installed")}
            </Badge>
          )}
          {hasUpdate && (
            <Badge
              variant="outline"
              className="shrink-0 border-amber-500 text-amber-600"
            >
              {t("skills.updateAvailable", { defaultValue: "有更新" })}
            </Badge>
          )}
        </div>
      </CardHeader>
      <CardContent className="flex-1 pt-0" style={indentStyle}>
        <div className="space-y-3">
          <p className="text-sm text-muted-foreground/90 line-clamp-4 leading-relaxed">
            {skill.description || t("skills.noDescription")}
          </p>
          {(parentPath || depth !== null) && (
            <div className="space-y-1 text-xs text-muted-foreground/90">
              {parentPath && (
                <div className="flex items-start gap-2 min-w-0">
                  <span className="shrink-0 font-medium text-foreground/80">
                    {t("skills.parentPath")}
                  </span>
                  <span className="truncate">{parentPath}</span>
                </div>
              )}
              {depth !== null && (
                <div className="flex items-center gap-2">
                  <span className="shrink-0 font-medium text-foreground/80">
                    {t("skills.depth")}
                  </span>
                  <span>{depth}</span>
                </div>
              )}
            </div>
          )}
          {hasCommands && (
            <Collapsible className="rounded-md border border-border-default/60 bg-muted/20">
              <CollapsibleTrigger className="group flex w-full items-center justify-between px-2.5 py-2 text-xs font-medium text-muted-foreground transition hover:text-foreground">
                <span>
                  {t("skills.workflows")} ({commands.length})
                </span>
                <ChevronDown className="h-3.5 w-3.5 transition-transform group-data-[state=open]:rotate-180" />
              </CollapsibleTrigger>
              <CollapsibleContent className="space-y-2 border-t border-border-default/60 px-2.5 py-2">
                {commands.map((command, index) => (
                  <div
                    key={`${command.name}-${command.filePath}-${index}`}
                    className="rounded-md border border-border-default/60 bg-card/60 px-2.5 py-2"
                  >
                    <div className="text-xs font-semibold text-foreground">
                      {command.name}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {command.description || t("skills.noDescription")}
                    </div>
                  </div>
                ))}
              </CollapsibleContent>
            </Collapsible>
          )}
          {typeof installs === "number" && (
            <p className="text-xs text-muted-foreground">
              {t("skills.catalog.installs", {
                count: installs,
                defaultValue: "{{count}} 次安装",
              })}
            </p>
          )}
          {onToggleApp && (skill.installedApps?.length ?? 0) > 0 && (
            <div className="grid grid-cols-2 gap-x-3 gap-y-2 border-t border-border-default/60 pt-3">
              {MANAGED_SKILL_APPS.map((app) => {
                const enabled = (skill.installedApps ?? []).includes(app);
                return (
                  <label
                    key={app}
                    className="flex items-center justify-between gap-2 text-xs"
                  >
                    <span className="truncate">
                      {t(`apps.${app}`, { defaultValue: app })}
                    </span>
                    <Switch
                      checked={enabled}
                      disabled={loading || updating}
                      onCheckedChange={(checked) =>
                        void onToggleApp(app, checked)
                      }
                      aria-label={t("skills.toggleApp", {
                        app: t(`apps.${app}`, { defaultValue: app }),
                        defaultValue: "切换 {{app}}",
                      })}
                    />
                  </label>
                );
              })}
            </div>
          )}
        </div>
      </CardContent>
      <CardFooter
        className="flex gap-2 pt-3 border-t border-border-default"
        style={indentStyle}
      >
        {hasUpdate && onUpdate && (
          <Button
            variant="outline"
            size="sm"
            onClick={() => void onUpdate()}
            disabled={loading || updating}
            className="flex-1 border-amber-300 text-amber-700 dark:text-amber-400"
          >
            {updating ? (
              <Loader2 className="h-3.5 w-3.5 mr-1.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5 mr-1.5" />
            )}
            {t("skills.update", { defaultValue: "更新" })}
          </Button>
        )}
        {skill.readmeUrl && (
          <Button
            variant="ghost"
            size="sm"
            onClick={handleOpenGithub}
            disabled={loading}
            className="flex-1"
          >
            <ExternalLink className="h-3.5 w-3.5 mr-1.5" />
            {t("skills.view")}
          </Button>
        )}
        {skill.installed ? (
          <Button
            variant="outline"
            size="sm"
            onClick={handleUninstall}
            disabled={loading}
            className="flex-1 border-red-200 text-red-600 hover:bg-red-50 hover:text-red-700 dark:border-red-900/50 dark:text-red-400 dark:hover:bg-red-950/50 dark:hover:text-red-300"
          >
            {loading ? (
              <Loader2 className="h-3.5 w-3.5 mr-1.5 animate-spin" />
            ) : (
              <Trash2 className="h-3.5 w-3.5 mr-1.5" />
            )}
            {loading ? t("skills.uninstalling") : t("skills.uninstall")}
          </Button>
        ) : (
          <Button
            variant="mcp"
            size="sm"
            onClick={handleInstall}
            disabled={loading || !skill.repoOwner}
            className="flex-1"
          >
            {loading ? (
              <Loader2 className="h-3.5 w-3.5 mr-1.5 animate-spin" />
            ) : (
              <Download className="h-3.5 w-3.5 mr-1.5" />
            )}
            {loading ? t("skills.installing") : t("skills.install")}
          </Button>
        )}
      </CardFooter>
    </Card>
  );
}
