import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderSearch, Loader2, RefreshCw } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { DIRECTORY_APPS } from "@/config/apps";
import type { AppId } from "@/lib/api";
import {
  skillsApi,
  type ImportInstalledSkillSelection,
  type InstalledSkillDiscovery,
} from "@/lib/api/skills";
import { extractErrorMessage } from "@/utils/errorUtils";

interface InstalledSkillsImportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  currentApp: AppId;
  onImported: () => Promise<unknown> | unknown;
}

interface ImportDraft {
  selected: boolean;
  source: string;
  apps: string[];
  overwrite: boolean;
}

const normalizeSkillsApp = (app: AppId): string =>
  app === "omo" || app === "omo-slim" ? "opencode" : app;

export function InstalledSkillsImportDialog({
  open,
  onOpenChange,
  currentApp,
  onImported,
}: InstalledSkillsImportDialogProps) {
  const { t } = useTranslation();
  const [discoveries, setDiscoveries] = useState<InstalledSkillDiscovery[]>([]);
  const [drafts, setDrafts] = useState<Record<string, ImportDraft>>({});
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);

  const buildDraft = useCallback(
    (discovery: InstalledSkillDiscovery): ImportDraft => {
      const managed = new Set(discovery.managedApps);
      const foundApps = discovery.sources
        .map((source) => source.source)
        .filter(
          (source) =>
            DIRECTORY_APPS.includes(source as (typeof DIRECTORY_APPS)[number]) &&
            !managed.has(source),
        );
      const apps = Array.from(new Set(foundApps));
      if (apps.length === 0) {
        apps.push(normalizeSkillsApp(currentApp));
      }
      const preferredSource =
        discovery.sources.find((source) => source.matchesTarget) ??
        discovery.sources.find((source) => source.source !== "cc-switch") ??
        discovery.sources[0];
      return {
        selected: false,
        source: preferredSource?.source ?? "",
        apps,
        overwrite: false,
      };
    },
    [currentApp],
  );

  const scan = useCallback(async () => {
    setLoading(true);
    try {
      const next = await skillsApi.discoverInstalled();
      setDiscoveries(next);
      setDrafts(
        Object.fromEntries(
          next.map((discovery) => [
            discovery.directory,
            buildDraft(discovery),
          ]),
        ),
      );
    } catch (error) {
      toast.error(extractErrorMessage(error));
    } finally {
      setLoading(false);
    }
  }, [buildDraft]);

  useEffect(() => {
    if (open) void scan();
  }, [open, scan]);

  const updateDraft = (
    directory: string,
    update: (draft: ImportDraft) => ImportDraft,
  ) => {
    setDrafts((current) => {
      const draft = current[directory];
      if (!draft) return current;
      return { ...current, [directory]: update(draft) };
    });
  };

  const selectedImports = useMemo(() => {
    return discoveries.flatMap<ImportInstalledSkillSelection>((discovery) => {
      const draft = drafts[discovery.directory];
      if (!draft?.selected || !draft.source || draft.apps.length === 0) {
        return [];
      }
      return [
        {
          directory: discovery.directory,
          source: draft.source,
          apps: draft.apps,
          overwrite: draft.overwrite,
        },
      ];
    });
  }, [discoveries, drafts]);

  const hasUnconfirmedConflict = discoveries.some((discovery) => {
    const draft = drafts[discovery.directory];
    if (!draft?.selected || discovery.status !== "conflict") return false;
    const source = discovery.sources.find(
      (candidate) => candidate.source === draft.source,
    );
    return !source?.matchesTarget && !draft.overwrite;
  });

  const handleImport = async () => {
    if (selectedImports.length === 0 || hasUnconfirmedConflict) return;
    setImporting(true);
    try {
      const results = await skillsApi.importInstalled(selectedImports);
      const completed = results.filter(
        (result) =>
          result.status === "imported" || result.status === "already_managed",
      ).length;
      const conflicts = results.filter(
        (result) => result.status === "conflict",
      ).length;
      if (completed > 0) {
        toast.success(
          t("skills.discovery.importSuccess", {
            count: completed,
            defaultValue: `已导入 ${completed} 个 Skill`,
          }),
        );
        await onImported();
      }
      if (conflicts > 0) {
        toast.warning(
          t("skills.discovery.conflictsRemain", {
            count: conflicts,
            defaultValue: `${conflicts} 个冲突未导入`,
          }),
        );
      }
      await scan();
    } catch (error) {
      toast.error(extractErrorMessage(error));
    } finally {
      setImporting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        zIndex="nested"
        className="h-[min(780px,90vh)] max-w-[min(980px,96vw)] p-0"
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <FolderSearch className="h-5 w-5" />
            {t("skills.discovery.title", {
              defaultValue: "导入已安装 Skills",
            })}
          </DialogTitle>
          <DialogDescription>
            {t("skills.discovery.count", {
              count: discoveries.length,
              defaultValue: `发现 ${discoveries.length} 个未纳入统一管理的 Skill`,
            })}
          </DialogDescription>
        </DialogHeader>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {loading ? (
            <div className="grid h-full min-h-56 place-items-center">
              <Loader2 className="h-7 w-7 animate-spin text-muted-foreground" />
            </div>
          ) : discoveries.length === 0 ? (
            <div className="grid h-full min-h-56 place-items-center px-6 text-center text-sm text-muted-foreground">
              {t("skills.discovery.empty", {
                defaultValue: "没有发现未管理的 Skill",
              })}
            </div>
          ) : (
            <div className="divide-y divide-border-default">
              {discoveries.map((discovery) => {
                const draft = drafts[discovery.directory];
                const selectedSource = discovery.sources.find(
                  (source) => source.source === draft?.source,
                );
                const needsOverwrite =
                  discovery.status === "conflict" &&
                  !selectedSource?.matchesTarget;
                return (
                  <div
                    key={discovery.directory}
                    className="grid gap-3 px-5 py-4 md:grid-cols-[20px_minmax(0,1fr)]"
                  >
                    <Checkbox
                      aria-label={t("skills.discovery.select", {
                        name: discovery.name,
                        defaultValue: `选择 ${discovery.name}`,
                      })}
                      checked={draft?.selected ?? false}
                      onCheckedChange={(checked) =>
                        updateDraft(discovery.directory, (current) => ({
                          ...current,
                          selected: checked === true,
                        }))
                      }
                    />
                    <div className="min-w-0 space-y-3">
                      <div className="flex flex-wrap items-start justify-between gap-2">
                        <div className="min-w-0">
                          <div className="break-words text-sm font-semibold">
                            {discovery.name}
                          </div>
                          <div className="mt-0.5 break-all font-mono text-xs text-muted-foreground">
                            {discovery.directory}
                          </div>
                        </div>
                        <Badge
                          variant={
                            discovery.status === "conflict"
                              ? "destructive"
                              : discovery.status === "identical"
                                ? "secondary"
                                : "outline"
                          }
                        >
                          {t(`skills.discovery.status.${discovery.status}`, {
                            defaultValue: discovery.status,
                          })}
                        </Badge>
                      </div>

                      {discovery.description ? (
                        <p className="line-clamp-2 text-xs text-muted-foreground">
                          {discovery.description}
                        </p>
                      ) : null}

                      <div className="grid gap-3 lg:grid-cols-2">
                        <div className="space-y-1.5">
                          <div className="text-xs font-medium text-muted-foreground">
                            {t("skills.discovery.source", {
                              defaultValue: "来源",
                            })}
                          </div>
                          <Select
                            value={draft?.source ?? ""}
                            onValueChange={(source) =>
                              updateDraft(discovery.directory, (current) => ({
                                ...current,
                                source,
                                overwrite: false,
                              }))
                            }
                          >
                            <SelectTrigger className="w-full">
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                              {discovery.sources.map((source) => (
                                <SelectItem
                                  key={`${discovery.directory}:${source.source}`}
                                  value={source.source}
                                >
                                  {source.source}
                                  {source.matchesTarget
                                    ? ` (${t("skills.discovery.sameContent", { defaultValue: "内容一致" })})`
                                    : ""}
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                          <div className="break-all font-mono text-[11px] leading-4 text-muted-foreground">
                            {selectedSource?.path ?? "-"}
                          </div>
                        </div>

                        <div className="space-y-1.5">
                          <div className="text-xs font-medium text-muted-foreground">
                            {t("skills.discovery.target", {
                              defaultValue: "统一存储目标",
                            })}
                          </div>
                          <div className="min-h-9 break-all rounded border border-border-default bg-muted/30 px-2.5 py-2 font-mono text-[11px] leading-4 text-muted-foreground">
                            {discovery.targetPath}
                          </div>
                        </div>
                      </div>

                      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
                        <span className="text-xs font-medium text-muted-foreground">
                          {t("skills.discovery.apps", {
                            defaultValue: "启用客户端",
                          })}
                        </span>
                        {DIRECTORY_APPS.map((app) => (
                          <label
                            key={app}
                            className="flex items-center gap-1.5 text-xs"
                          >
                            <Checkbox
                              checked={draft?.apps.includes(app) ?? false}
                              onCheckedChange={(checked) =>
                                updateDraft(
                                  discovery.directory,
                                  (current) => ({
                                    ...current,
                                    apps:
                                      checked === true
                                        ? Array.from(
                                            new Set([...current.apps, app]),
                                          )
                                        : current.apps.filter(
                                            (candidate) => candidate !== app,
                                          ),
                                  }),
                                )
                              }
                            />
                            {t(`apps.${app}`, { defaultValue: app })}
                          </label>
                        ))}
                      </div>

                      {needsOverwrite ? (
                        <label className="flex items-start gap-2 rounded border border-destructive/40 bg-destructive/5 p-2.5 text-xs">
                          <Checkbox
                            checked={draft?.overwrite ?? false}
                            onCheckedChange={(checked) =>
                              updateDraft(discovery.directory, (current) => ({
                                ...current,
                                overwrite: checked === true,
                              }))
                            }
                          />
                          <span>
                            {t("skills.discovery.confirmOverwrite", {
                              defaultValue: "使用所选来源覆盖统一存储中的不同内容",
                            })}
                          </span>
                        </label>
                      ) : null}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            size="icon"
            title={t("common.refresh", { defaultValue: "刷新" })}
            onClick={() => void scan()}
            disabled={loading || importing}
          >
            <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          </Button>
          <Button
            type="button"
            onClick={() => void handleImport()}
            disabled={
              importing ||
              selectedImports.length === 0 ||
              hasUnconfirmedConflict
            }
          >
            {importing ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            {t("skills.discovery.importSelected", {
              count: selectedImports.length,
              defaultValue: `导入所选 (${selectedImports.length})`,
            })}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
