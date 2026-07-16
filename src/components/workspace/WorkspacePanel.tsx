import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  CalendarDays,
  FileText,
  History,
  RefreshCw,
  Save,
  RotateCcw,
  Search,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import {
  workspaceApi,
  type DailyMemoryInfo,
  type DailyMemorySearchResult,
  type WorkspaceBackupInfo,
  type WorkspaceFileInfo,
} from "@/lib/api/workspace";
import { useCapabilitiesQuery } from "@/lib/query";

const FILES = [
  "AGENTS.md",
  "SOUL.md",
  "USER.md",
  "IDENTITY.md",
  "TOOLS.md",
  "MEMORY.md",
  "HEARTBEAT.md",
  "BOOTSTRAP.md",
  "BOOT.md",
];

interface WorkspacePanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function WorkspacePanel({ open, onOpenChange }: WorkspacePanelProps) {
  const { t } = useTranslation();
  const { data: capabilities } = useCapabilitiesQuery();
  const [files, setFiles] = useState<WorkspaceFileInfo[]>([]);
  const [selected, setSelected] = useState<string>(FILES[0]);
  const [content, setContent] = useState("");
  const [etag, setEtag] = useState<string | undefined>();
  const [backups, setBackups] = useState<WorkspaceBackupInfo[]>([]);
  const [pendingBackup, setPendingBackup] =
    useState<WorkspaceBackupInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [memoryDate, setMemoryDate] = useState(() =>
    new Date().toISOString().slice(0, 10),
  );
  const [memoryContent, setMemoryContent] = useState("");
  const [memoryEtag, setMemoryEtag] = useState<string | undefined>();
  const [memories, setMemories] = useState<DailyMemoryInfo[]>([]);
  const [memoryLoading, setMemoryLoading] = useState(false);
  const [memorySaving, setMemorySaving] = useState(false);
  const [memorySearch, setMemorySearch] = useState("");
  const [memorySearchResults, setMemorySearchResults] = useState<
    DailyMemorySearchResult[]
  >([]);
  const [memorySearching, setMemorySearching] = useState(false);
  const [pendingMemoryDelete, setPendingMemoryDelete] =
    useState<DailyMemoryInfo | null>(null);

  const hostLabel =
    capabilities?.host === "server"
      ? t("workspace.serverHost", { defaultValue: "服务器主机文件" })
      : t("workspace.localHost", { defaultValue: "本机文件" });

  const refreshFiles = useCallback(async () => {
    setLoading(true);
    try {
      setFiles(await workspaceApi.listFiles());
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : t("workspace.loadFailed", { defaultValue: "读取 Workspace 失败" }),
      );
    } finally {
      setLoading(false);
    }
  }, [t]);

  const loadFile = useCallback(async (name: string) => {
    setSelected(name);
    try {
      const value = await workspaceApi.readFile(name);
      setContent(value.content);
      setEtag(value.etag);
      setBackups(await workspaceApi.listBackups(name));
    } catch {
      setContent("");
      setEtag(undefined);
      setBackups([]);
    }
  }, []);

  const refreshMemories = useCallback(async () => {
    try {
      setMemories(await workspaceApi.listDailyMemory());
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : t("workspace.memoryLoadFailed", {
              defaultValue: "读取 Daily Memory 列表失败",
            }),
      );
    }
  }, [t]);

  const loadMemory = useCallback(async (date: string) => {
    setMemoryDate(date);
    setMemoryLoading(true);
    try {
      const value = await workspaceApi.readDailyMemory(date);
      setMemoryContent(value.content);
      setMemoryEtag(value.etag);
    } catch {
      setMemoryContent("");
      setMemoryEtag(undefined);
    } finally {
      setMemoryLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!open) return;
    void refreshFiles();
    void loadFile(FILES[0]);
    void refreshMemories();
    void loadMemory(new Date().toISOString().slice(0, 10));
  }, [open, refreshFiles, loadFile, refreshMemories, loadMemory]);

  useEffect(() => {
    if (!open || !memorySearch.trim()) {
      setMemorySearchResults([]);
      setMemorySearching(false);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setMemorySearching(true);
      workspaceApi
        .searchDailyMemory(memorySearch.trim())
        .then((results) => {
          if (!cancelled) setMemorySearchResults(results);
        })
        .catch((error) => {
          if (!cancelled) {
            setMemorySearchResults([]);
            toast.error(
              error instanceof Error
                ? error.message
                : t("workspace.memorySearchFailed"),
            );
          }
        })
        .finally(() => {
          if (!cancelled) setMemorySearching(false);
        });
    }, 300);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [memorySearch, open, t]);

  const saveFile = async () => {
    setSaving(true);
    try {
      const result = await workspaceApi.writeFile(selected, content, etag);
      setEtag(result.etag);
      setBackups(await workspaceApi.listBackups(selected));
      await refreshFiles();
      toast.success(
        t("workspace.saveSuccess", { defaultValue: "Workspace 文件已保存" }),
      );
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : t("workspace.saveFailed", {
              defaultValue: "保存失败，文件可能已被修改",
            }),
      );
    } finally {
      setSaving(false);
    }
  };

  const restoreBackup = async (backup: WorkspaceBackupInfo) => {
    try {
      const result = await workspaceApi.restoreBackup(
        selected,
        backup.id,
        etag,
      );
      setContent((await workspaceApi.readFile(selected)).content);
      setEtag(result.etag);
      setBackups(await workspaceApi.listBackups(selected));
      await refreshFiles();
      toast.success(
        t("workspace.restoreSuccess", {
          defaultValue: "已恢复 Workspace 备份",
        }),
      );
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : t("workspace.restoreFailed", { defaultValue: "恢复备份失败" }),
      );
    }
  };

  const saveMemory = async () => {
    setMemorySaving(true);
    try {
      const result = await workspaceApi.writeDailyMemory(
        memoryDate,
        memoryContent,
        memoryEtag,
      );
      setMemoryEtag(result.etag);
      await refreshMemories();
      toast.success(
        t("workspace.memorySaved", { defaultValue: "Daily Memory 已保存" }),
      );
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : t("workspace.saveFailed", {
              defaultValue: "保存失败，文件可能已被修改",
            }),
      );
    } finally {
      setMemorySaving(false);
    }
  };

  const deleteMemory = async (memory: DailyMemoryInfo) => {
    try {
      await workspaceApi.deleteDailyMemory(memory.date, memory.etag);
      if (memoryDate === memory.date) {
        setMemoryContent("");
        setMemoryEtag(undefined);
      }
      await refreshMemories();
      if (memorySearch.trim()) {
        setMemorySearchResults(
          await workspaceApi.searchDailyMemory(memorySearch.trim()),
        );
      }
      toast.success(t("workspace.memoryDeleted"));
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : t("workspace.memoryDeleteFailed"),
      );
    }
  };

  const selectedInfo = useMemo(
    () => files.find((file) => file.name === selected),
    [files, selected],
  );
  const displayedMemories = useMemo<DailyMemoryInfo[]>(
    () =>
      memorySearch.trim()
        ? memorySearchResults.map((result) => ({
            date: result.date,
            sizeBytes: result.sizeBytes,
            modifiedAt: result.modifiedAt,
            etag: result.etag,
            preview: result.snippet,
          }))
        : memories,
    [memories, memorySearch, memorySearchResults],
  );

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="flex h-[min(850px,90vh)] max-w-6xl flex-col gap-0 p-0">
          <DialogHeader className="border-b px-6 py-4">
            <DialogTitle className="flex items-center gap-2">
              <FileText className="h-4 w-4" />
              {t("workspace.title", { defaultValue: "OpenClaw Workspace" })}
            </DialogTitle>
            <DialogDescription className="flex items-center gap-2">
              {hostLabel}
              <span className="text-muted-foreground">
                ~/.openclaw/workspace
              </span>
            </DialogDescription>
          </DialogHeader>

          <Tabs defaultValue="files" className="flex min-h-0 flex-1 flex-col">
            <TabsList className="mx-6 mt-3 w-fit">
              <TabsTrigger value="files">
                {t("workspace.filesTab", { defaultValue: "Workspace 文件" })}
              </TabsTrigger>
              <TabsTrigger value="memory">
                {t("workspace.memoryTab", { defaultValue: "Daily Memory" })}
              </TabsTrigger>
            </TabsList>
            <TabsContent
              value="files"
              className="min-h-0 flex-1 px-6 pb-6 pt-3"
            >
              <div className="grid h-full min-h-0 gap-4 md:grid-cols-[220px_1fr]">
                <aside className="min-h-0 overflow-y-auto border-r pr-3">
                  <div className="mb-2 flex items-center justify-between text-xs text-muted-foreground">
                    <span>
                      {loading
                        ? t("common.loading", { defaultValue: "加载中" })
                        : `${files.filter((f) => f.exists).length}/${FILES.length}`}
                    </span>
                    <Button
                      size="icon"
                      variant="ghost"
                      onClick={() => void refreshFiles()}
                      title={t("common.refresh", { defaultValue: "刷新" })}
                    >
                      <RefreshCw className="h-4 w-4" />
                    </Button>
                  </div>
                  <div className="space-y-1">
                    {FILES.map((name) => {
                      const info = files.find((file) => file.name === name);
                      return (
                        <button
                          key={name}
                          type="button"
                          onClick={() => void loadFile(name)}
                          className={`flex w-full items-center justify-between rounded-md px-3 py-2 text-left text-sm ${selected === name ? "bg-accent text-accent-foreground" : "hover:bg-muted"}`}
                        >
                          <span>{name}</span>
                          <span
                            className={`h-1.5 w-1.5 rounded-full ${info?.exists ? "bg-emerald-500" : "bg-muted-foreground/30"}`}
                          />
                        </button>
                      );
                    })}
                  </div>
                </aside>
                <section className="flex min-h-0 flex-col gap-3">
                  <div className="flex items-center justify-between gap-2">
                    <div className="text-sm font-medium">{selected}</div>
                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                      {selectedInfo?.sizeBytes
                        ? `${selectedInfo.sizeBytes} B`
                        : t("workspace.newFile", { defaultValue: "新文件" })}
                      <Button
                        size="sm"
                        onClick={() => void saveFile()}
                        disabled={saving}
                      >
                        <Save className="h-3.5 w-3.5" />
                        {t("common.save", { defaultValue: "保存" })}
                      </Button>
                    </div>
                  </div>
                  <Textarea
                    value={content}
                    onChange={(event) => setContent(event.target.value)}
                    className="min-h-0 flex-1 resize-none font-mono text-sm"
                  />
                  <div className="border-t pt-2">
                    <div className="mb-2 flex items-center gap-2 text-xs font-medium text-muted-foreground">
                      <History className="h-3.5 w-3.5" />
                      {t("workspace.backups", { defaultValue: "备份" })}
                    </div>
                    {backups.length === 0 ? (
                      <span className="text-xs text-muted-foreground">
                        {t("workspace.noBackups", { defaultValue: "暂无备份" })}
                      </span>
                    ) : (
                      <div className="flex flex-wrap gap-2">
                        {backups.slice(0, 8).map((backup) => (
                          <Button
                            key={backup.id}
                            size="sm"
                            variant="outline"
                            onClick={() => setPendingBackup(backup)}
                            title={backup.id}
                          >
                            <RotateCcw className="h-3.5 w-3.5" />
                            {new Date(backup.createdAt * 1000).toLocaleString()}
                          </Button>
                        ))}
                      </div>
                    )}
                  </div>
                </section>
              </div>
            </TabsContent>
            <TabsContent
              value="memory"
              className="min-h-0 flex-1 px-6 pb-6 pt-3"
            >
              <div className="grid h-full min-h-0 gap-4 md:grid-cols-[220px_1fr]">
                <aside className="min-h-0 overflow-y-auto border-r pr-3">
                  <div className="relative mb-3">
                    <Search className="pointer-events-none absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                    <input
                      type="search"
                      value={memorySearch}
                      onChange={(event) => setMemorySearch(event.target.value)}
                      placeholder={t("workspace.memorySearch")}
                      className="h-9 w-full rounded-md border bg-background pl-8 pr-3 text-sm"
                    />
                  </div>
                  <div className="mb-2 flex items-center justify-between text-xs text-muted-foreground">
                    <span>
                      {t("workspace.memoryEntries", {
                        defaultValue: "{{count}} 条记录",
                        count: displayedMemories.length,
                      })}
                      {memorySearching ? ` · ${t("common.loading")}` : ""}
                    </span>
                    <Button
                      size="icon"
                      variant="ghost"
                      onClick={() => void refreshMemories()}
                      title={t("common.refresh", { defaultValue: "刷新" })}
                    >
                      <RefreshCw className="h-4 w-4" />
                    </Button>
                  </div>
                  <div className="space-y-1">
                    {displayedMemories.map((memory) => (
                      <div key={memory.date} className="flex items-start gap-1">
                        <button
                          type="button"
                          onClick={() => void loadMemory(memory.date)}
                          className={`min-w-0 flex-1 rounded-md px-3 py-2 text-left ${
                            memoryDate === memory.date
                              ? "bg-accent text-accent-foreground"
                              : "hover:bg-muted"
                          }`}
                        >
                          <span className="flex items-center gap-2 text-sm font-medium">
                            <CalendarDays className="h-3.5 w-3.5" />
                            {memory.date}
                          </span>
                          <span className="mt-1 block truncate text-xs text-muted-foreground">
                            {memory.preview || `${memory.sizeBytes} B`}
                          </span>
                        </button>
                        <Button
                          type="button"
                          size="icon"
                          variant="ghost"
                          className="h-9 w-9 shrink-0 text-muted-foreground hover:text-destructive"
                          title={t("common.delete")}
                          onClick={() => setPendingMemoryDelete(memory)}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    ))}
                    {displayedMemories.length === 0 ? (
                      <p className="px-3 py-2 text-xs text-muted-foreground">
                        {t("workspace.noMemory", {
                          defaultValue: "暂无 Daily Memory",
                        })}
                      </p>
                    ) : null}
                  </div>
                </aside>
                <section className="flex min-h-0 flex-col gap-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <input
                      type="date"
                      value={memoryDate}
                      onChange={(event) => void loadMemory(event.target.value)}
                      className="h-9 rounded-md border bg-background px-3 text-sm"
                    />
                    <Button
                      size="sm"
                      onClick={() => void saveMemory()}
                      disabled={memoryLoading || memorySaving}
                    >
                      <Save className="h-3.5 w-3.5" />
                      {memorySaving
                        ? t("common.saving", { defaultValue: "保存中..." })
                        : t("common.save", { defaultValue: "保存" })}
                    </Button>
                  </div>
                  <Textarea
                    value={memoryContent}
                    onChange={(event) => setMemoryContent(event.target.value)}
                    disabled={memoryLoading}
                    className="min-h-0 flex-1 resize-none font-mono text-sm"
                  />
                </section>
              </div>
            </TabsContent>
          </Tabs>
        </DialogContent>
      </Dialog>
      <ConfirmDialog
        isOpen={pendingBackup !== null}
        variant="info"
        title={t("workspace.restoreTitle", {
          defaultValue: "恢复 Workspace 备份",
        })}
        message={t("workspace.restoreConfirm", {
          defaultValue:
            "将把 {{file}} 恢复到 {{time}} 的内容。当前内容会先自动备份。",
          file: selected,
          time: pendingBackup
            ? new Date(pendingBackup.createdAt * 1000).toLocaleString()
            : "",
        })}
        confirmText={t("workspace.restore", { defaultValue: "恢复" })}
        onCancel={() => setPendingBackup(null)}
        onConfirm={() => {
          const backup = pendingBackup;
          setPendingBackup(null);
          if (backup) void restoreBackup(backup);
        }}
      />
      <ConfirmDialog
        isOpen={pendingMemoryDelete !== null}
        title={t("workspace.memoryDeleteTitle")}
        message={t("workspace.memoryDeleteConfirm", {
          date: pendingMemoryDelete?.date ?? "",
        })}
        confirmText={t("common.delete")}
        onCancel={() => setPendingMemoryDelete(null)}
        onConfirm={() => {
          const memory = pendingMemoryDelete;
          setPendingMemoryDelete(null);
          if (memory) void deleteMemory(memory);
        }}
      />
    </>
  );
}
