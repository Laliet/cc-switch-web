import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { observeElementRect, useVirtualizer } from "@tanstack/react-virtual";
import { toast } from "sonner";
import {
  CheckSquare,
  ChevronDown,
  Clock3,
  Copy,
  FolderOpen,
  ListTree,
  MessagesSquare,
  RefreshCw,
  Search,
  Server,
  Trash2,
} from "lucide-react";

import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { sessionsApi, type DeleteSessionResult } from "@/lib/api/sessions";
import type { SessionMessage, SessionMeta } from "@/types";
import { extractErrorMessage } from "@/utils/errorUtils";

type ProviderFilter =
  | "all"
  | "claude"
  | "codex"
  | "gemini"
  | "opencode"
  | "openclaw";

const providerLabels: Record<Exclude<ProviderFilter, "all">, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  opencode: "OpenCode",
  openclaw: "OpenClaw",
};

const sessionKey = (session: SessionMeta) =>
  `${session.providerId}:${session.sessionId}:${session.sourcePath ?? ""}`;

const formatTime = (timestamp: number | undefined, language: string) => {
  if (!timestamp) return "-";
  return new Intl.DateTimeFormat(language.startsWith("zh") ? "zh-CN" : "en", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp));
};

interface SessionManagerPageProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function SessionManagerPage({
  open,
  onOpenChange,
}: SessionManagerPageProps) {
  const { t, i18n } = useTranslation();
  const [sessions, setSessions] = useState<SessionMeta[]>([]);
  const [messages, setMessages] = useState<SessionMessage[]>([]);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const [provider, setProvider] = useState<ProviderFilter>("all");
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [nextCursor, setNextCursor] = useState<string | undefined>();
  const [totalSessions, setTotalSessions] = useState(0);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [deleteTargets, setDeleteTargets] = useState<SessionMeta[] | null>(
    null,
  );
  const messageScrollRef = useRef<HTMLDivElement | null>(null);
  const tocScrollRef = useRef<HTMLDivElement | null>(null);
  const sessionRequestVersionRef = useRef(0);
  const [effectiveSearch, setEffectiveSearch] = useState("");

  const selectedSession = useMemo(
    () =>
      sessions.find((session) => sessionKey(session) === selectedKey) ?? null,
    [selectedKey, sessions],
  );

  const filteredSessions = sessions;

  const userMessageToc = useMemo(
    () =>
      messages
        .map((message, index) => ({ message, index }))
        .filter(({ message }) => message.role.toLocaleLowerCase() === "user"),
    [messages],
  );

  const messageVirtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => messageScrollRef.current,
    estimateSize: () => 132,
    initialRect: { width: 800, height: 600 },
    observeElementRect: (instance, callback) =>
      observeElementRect(instance, (rect) =>
        callback({
          width: rect.width || 800,
          height: rect.height || 600,
        }),
      ),
    measureElement: (element) => element.getBoundingClientRect().height || 132,
    overscan: 6,
    gap: 12,
    getItemKey: (index) =>
      `${messages[index]?.ts ?? "message"}-${index}-${messages[index]?.role ?? ""}`,
  });
  const tocVirtualizer = useVirtualizer({
    count: userMessageToc.length,
    getScrollElement: () => tocScrollRef.current,
    estimateSize: () => 44,
    initialRect: { width: 220, height: 560 },
    observeElementRect: (instance, callback) =>
      observeElementRect(instance, (rect) =>
        callback({
          width: rect.width || 220,
          height: rect.height || 560,
        }),
      ),
    measureElement: (element) => element.getBoundingClientRect().height || 44,
    overscan: 8,
    gap: 4,
    getItemKey: (index) => {
      const item = userMessageToc[index];
      return `${item?.message.ts ?? "toc"}-${item?.index ?? index}`;
    },
  });

  useEffect(() => {
    const timer = window.setTimeout(
      () => setEffectiveSearch(search.trim()),
      300,
    );
    return () => window.clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    if (filteredSessions.length === 0) {
      setSelectedKey(null);
      return;
    }
    if (
      !selectedKey ||
      !filteredSessions.some((session) => sessionKey(session) === selectedKey)
    ) {
      setSelectedKey(sessionKey(filteredSessions[0]));
    }
  }, [filteredSessions, selectedKey]);

  const loadFirstPage = useCallback(
    async (refresh = false) => {
      const requestVersion = ++sessionRequestVersionRef.current;
      setLoading(true);
      setLoadingMore(false);
      setNextCursor(undefined);
      try {
        const page = await sessionsApi.listPage({
          limit: 100,
          providerId: provider === "all" ? undefined : provider,
          query: effectiveSearch || undefined,
          refresh,
        });
        if (requestVersion !== sessionRequestVersionRef.current) return;
        setSessions(page.sessions);
        setNextCursor(page.nextCursor);
        setTotalSessions(page.total);
        setSelectedKeys((current) => {
          const valid = new Set(page.sessions.map(sessionKey));
          return new Set([...current].filter((key) => valid.has(key)));
        });
        setSelectedKey((current) => {
          if (
            current &&
            page.sessions.some((session) => sessionKey(session) === current)
          ) {
            return current;
          }
          return page.sessions[0] ? sessionKey(page.sessions[0]) : null;
        });
      } catch (error) {
        if (requestVersion === sessionRequestVersionRef.current) {
          toast.error(extractErrorMessage(error));
        }
      } finally {
        if (requestVersion === sessionRequestVersionRef.current) {
          setLoading(false);
        }
      }
    },
    [effectiveSearch, provider],
  );

  const loadMoreSessions = useCallback(async () => {
    if (!nextCursor || loadingMore) return;
    const requestVersion = sessionRequestVersionRef.current;
    setLoadingMore(true);
    try {
      const page = await sessionsApi.listPage({
        cursor: nextCursor,
        limit: 100,
        providerId: provider === "all" ? undefined : provider,
        query: effectiveSearch || undefined,
      });
      if (requestVersion !== sessionRequestVersionRef.current) return;
      setSessions((current) => {
        const known = new Set(current.map(sessionKey));
        return [
          ...current,
          ...page.sessions.filter((session) => !known.has(sessionKey(session))),
        ];
      });
      setNextCursor(page.nextCursor);
      setTotalSessions(page.total);
    } catch (error) {
      if (requestVersion === sessionRequestVersionRef.current) {
        toast.error(extractErrorMessage(error));
      }
    } finally {
      if (requestVersion === sessionRequestVersionRef.current) {
        setLoadingMore(false);
      }
    }
  }, [effectiveSearch, loadingMore, nextCursor, provider]);

  useEffect(() => {
    if (open) void loadFirstPage(false);
  }, [loadFirstPage, open]);

  useEffect(() => {
    if (!selectedSession?.sourcePath) {
      setMessages([]);
      return;
    }
    let cancelled = false;
    setLoadingMessages(true);
    sessionsApi
      .getMessages(selectedSession.providerId, selectedSession.sourcePath)
      .then((next) => {
        if (!cancelled) setMessages(next);
      })
      .catch((error) => {
        if (!cancelled) {
          setMessages([]);
          toast.error(extractErrorMessage(error));
        }
      })
      .finally(() => {
        if (!cancelled) setLoadingMessages(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedSession]);

  useEffect(() => {
    if (messageScrollRef.current) {
      messageScrollRef.current.scrollTop = 0;
    }
  }, [selectedKey]);

  const copyText = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(t("sessionManager.copied"));
    } catch (error) {
      toast.error(extractErrorMessage(error));
    }
  };

  const toggleSelection = (key: string) => {
    setSelectedKeys((current) => {
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const confirmDelete = async () => {
    const targets = deleteTargets ?? [];
    if (targets.length === 0) return;
    setDeleting(true);
    try {
      const items = targets.flatMap((session) =>
        session.sourcePath
          ? [
              {
                providerId: session.providerId,
                sessionId: session.sessionId,
                sourcePath: session.sourcePath,
              },
            ]
          : [],
      );
      const results: DeleteSessionResult[] =
        items.length === 1
          ? [{ ...items[0], success: await sessionsApi.delete(items[0]) }]
          : await sessionsApi.deleteMany(items);
      const failures = results.filter((result) => !result.success);
      if (failures.length > 0) {
        toast.error(
          t("sessionManager.deletePartial", {
            failed: failures.length,
            total: results.length,
          }),
        );
      } else {
        toast.success(
          t("sessionManager.deleteSuccess", { count: results.length }),
        );
      }
      setDeleteTargets(null);
      await loadFirstPage(true);
    } catch (error) {
      toast.error(extractErrorMessage(error));
    } finally {
      setDeleting(false);
    }
  };

  const selectedTargets = sessions.filter((session) =>
    selectedKeys.has(sessionKey(session)),
  );

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="h-[min(880px,94vh)] max-h-[94vh] max-w-[min(1500px,98vw)] p-0">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-lg">
              <MessagesSquare className="h-5 w-5" />
              {t("sessionManager.title")}
            </DialogTitle>
            <DialogDescription className="flex items-center gap-1.5">
              <Server className="h-4 w-4 shrink-0" />
              {t("sessionManager.serverPathNotice")}
            </DialogDescription>
          </DialogHeader>

          <div className="grid min-h-0 flex-1 grid-cols-1 overflow-y-auto lg:grid-cols-[320px_minmax(420px,1fr)_220px] lg:overflow-hidden">
            <section className="flex min-h-[240px] flex-col border-b border-border-default lg:min-h-0 lg:border-b-0 lg:border-r">
              <div className="space-y-2 border-b border-border-default p-3">
                <div className="flex gap-2">
                  <div className="relative min-w-0 flex-1">
                    <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                    <Input
                      aria-label={t("sessionManager.search")}
                      className="pl-8"
                      value={search}
                      onChange={(event) => setSearch(event.target.value)}
                      placeholder={t("sessionManager.search")}
                    />
                  </div>
                  <Button
                    size="icon"
                    variant="outline"
                    title={t("sessionManager.refresh")}
                    onClick={() => void loadFirstPage(true)}
                    disabled={loading}
                  >
                    <RefreshCw
                      className={`h-4 w-4 ${loading ? "animate-spin" : ""}`}
                    />
                  </Button>
                </div>
                <div className="flex gap-2">
                  <Select
                    value={provider}
                    onValueChange={(value) =>
                      setProvider(value as ProviderFilter)
                    }
                  >
                    <SelectTrigger className="min-w-0 flex-1">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">
                        {t("sessionManager.allProviders")}
                      </SelectItem>
                      {Object.entries(providerLabels).map(([id, label]) => (
                        <SelectItem key={id} value={id}>
                          {label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Button
                    variant="outline"
                    size="icon"
                    title={t("sessionManager.selectVisible")}
                    onClick={() => {
                      const visible = filteredSessions.map(sessionKey);
                      const allSelected =
                        visible.length > 0 &&
                        visible.every((key) => selectedKeys.has(key));
                      setSelectedKeys((current) => {
                        const next = new Set(current);
                        visible.forEach((key) =>
                          allSelected ? next.delete(key) : next.add(key),
                        );
                        return next;
                      });
                    }}
                  >
                    <CheckSquare className="h-4 w-4" />
                  </Button>
                </div>
                {selectedTargets.length > 0 ? (
                  <Button
                    className="w-full"
                    variant="destructive"
                    onClick={() => setDeleteTargets(selectedTargets)}
                    disabled={deleting}
                  >
                    <Trash2 className="h-4 w-4" />
                    {t("sessionManager.deleteSelected", {
                      count: selectedTargets.length,
                    })}
                  </Button>
                ) : null}
              </div>

              <div className="min-h-0 flex-1 overflow-y-auto">
                {loading && sessions.length === 0 ? (
                  <div className="p-6 text-center text-sm text-muted-foreground">
                    {t("common.loading")}
                  </div>
                ) : filteredSessions.length === 0 ? (
                  <div className="p-6 text-center text-sm text-muted-foreground">
                    {t("sessionManager.empty")}
                  </div>
                ) : (
                  filteredSessions.map((session) => {
                    const key = sessionKey(session);
                    const selected = key === selectedKey;
                    return (
                      <div
                        key={key}
                        className={`flex border-b border-border-default ${selected ? "bg-accent" : "hover:bg-muted/40"}`}
                      >
                        <div className="flex items-start px-3 pt-4">
                          <Checkbox
                            aria-label={t("sessionManager.selectSession")}
                            checked={selectedKeys.has(key)}
                            onCheckedChange={() => toggleSelection(key)}
                          />
                        </div>
                        <button
                          type="button"
                          className="min-w-0 flex-1 px-1 py-3 pr-3 text-left"
                          onClick={() => setSelectedKey(key)}
                        >
                          <div className="mb-1 flex items-center justify-between gap-2">
                            <Badge variant="outline">
                              {providerLabels[
                                session.providerId as keyof typeof providerLabels
                              ] ?? session.providerId}
                            </Badge>
                            <span className="truncate text-xs text-muted-foreground">
                              {formatTime(
                                session.lastActiveAt ?? session.createdAt,
                                i18n.language,
                              )}
                            </span>
                          </div>
                          <div className="truncate text-sm font-medium">
                            {session.title || session.sessionId}
                          </div>
                          {session.summary ? (
                            <div className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                              {session.summary}
                            </div>
                          ) : null}
                        </button>
                      </div>
                    );
                  })
                )}
                {nextCursor ? (
                  <div className="border-t border-border-default p-3">
                    <Button
                      className="w-full"
                      variant="outline"
                      size="sm"
                      onClick={() => void loadMoreSessions()}
                      disabled={loadingMore}
                    >
                      <ChevronDown
                        className={`h-4 w-4 ${loadingMore ? "animate-bounce" : ""}`}
                      />
                      {loadingMore
                        ? t("common.loading")
                        : t("sessionManager.loadMore", {
                            loaded: sessions.length,
                            total: totalSessions,
                          })}
                    </Button>
                  </div>
                ) : null}
              </div>
            </section>

            <section className="flex min-h-[360px] min-w-0 flex-col overflow-hidden border-b border-border-default lg:min-h-0 lg:border-b-0 lg:border-r">
              {selectedSession ? (
                <>
                  <div className="border-b border-border-default p-4">
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div className="min-w-0 flex-1">
                        <h3 className="truncate text-base font-semibold">
                          {selectedSession.title || selectedSession.sessionId}
                        </h3>
                        <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
                          <span className="flex min-w-0 items-center gap-1">
                            <FolderOpen className="h-3.5 w-3.5 shrink-0" />
                            <span className="truncate">
                              {selectedSession.projectDir || "-"}
                            </span>
                          </span>
                          <span className="flex items-center gap-1">
                            <Clock3 className="h-3.5 w-3.5" />
                            {formatTime(
                              selectedSession.lastActiveAt ??
                                selectedSession.createdAt,
                              i18n.language,
                            )}
                          </span>
                        </div>
                      </div>
                      <div className="flex gap-2">
                        {selectedSession.resumeCommand ? (
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() =>
                              void copyText(selectedSession.resumeCommand!)
                            }
                          >
                            <Copy className="h-4 w-4" />
                            {t("sessionManager.copyResume")}
                          </Button>
                        ) : null}
                        <Button
                          variant="destructive"
                          size="icon"
                          title={t("common.delete")}
                          onClick={() => setDeleteTargets([selectedSession])}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    </div>
                    {selectedSession.sourcePath ? (
                      <div className="mt-3 break-all rounded border border-border-default bg-muted/30 px-2.5 py-2 font-mono text-xs text-muted-foreground">
                        {t("sessionManager.sourcePath")}:{" "}
                        {selectedSession.sourcePath}
                      </div>
                    ) : null}
                  </div>
                  <div
                    ref={messageScrollRef}
                    data-testid="session-message-scroll"
                    className="min-h-0 flex-1 overflow-y-auto p-4"
                  >
                    {loadingMessages ? (
                      <div className="py-8 text-center text-sm text-muted-foreground">
                        {t("common.loading")}
                      </div>
                    ) : messages.length === 0 ? (
                      <div className="py-8 text-center text-sm text-muted-foreground">
                        {t("sessionManager.noMessages")}
                      </div>
                    ) : (
                      <div
                        className="relative w-full"
                        style={{ height: messageVirtualizer.getTotalSize() }}
                      >
                        {messageVirtualizer
                          .getVirtualItems()
                          .map((virtualItem) => {
                            const index = virtualItem.index;
                            const message = messages[index];
                            return (
                              <div
                                key={virtualItem.key}
                                data-index={index}
                                ref={messageVirtualizer.measureElement}
                                className="absolute left-0 top-0 w-full"
                                style={{
                                  transform: `translateY(${virtualItem.start}px)`,
                                }}
                              >
                                <div className="rounded-md border border-border-default bg-background p-3">
                                  <div className="mb-2 flex items-center justify-between gap-2">
                                    <Badge
                                      variant={
                                        message.role === "user"
                                          ? "default"
                                          : "secondary"
                                      }
                                    >
                                      {message.role}
                                    </Badge>
                                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                                      {message.ts
                                        ? formatTime(message.ts, i18n.language)
                                        : null}
                                      <Button
                                        variant="ghost"
                                        size="icon"
                                        className="h-7 w-7"
                                        title={t("common.copy")}
                                        onClick={() =>
                                          void copyText(message.content)
                                        }
                                      >
                                        <Copy className="h-3.5 w-3.5" />
                                      </Button>
                                    </div>
                                  </div>
                                  <pre className="whitespace-pre-wrap break-words font-sans text-sm leading-6">
                                    {message.content}
                                  </pre>
                                </div>
                              </div>
                            );
                          })}
                      </div>
                    )}
                  </div>
                </>
              ) : (
                <div className="grid flex-1 place-items-center p-8 text-sm text-muted-foreground">
                  {t("sessionManager.selectPrompt")}
                </div>
              )}
            </section>

            <aside className="flex min-h-[180px] flex-col border-t border-border-default lg:min-h-0 lg:border-t-0">
              <div className="flex items-center gap-2 border-b border-border-default px-4 py-3 text-sm font-medium">
                <ListTree className="h-4 w-4" />
                {t("sessionManager.messageToc")}
              </div>
              <div
                ref={tocScrollRef}
                data-testid="session-message-toc"
                className="min-h-0 flex-1 overflow-y-auto p-2"
              >
                {userMessageToc.length === 0 ? (
                  <div className="p-3 text-xs text-muted-foreground">
                    {t("sessionManager.noToc")}
                  </div>
                ) : (
                  <div
                    className="relative w-full"
                    style={{ height: tocVirtualizer.getTotalSize() }}
                  >
                    {tocVirtualizer.getVirtualItems().map((virtualItem) => {
                      const { message, index } =
                        userMessageToc[virtualItem.index];
                      return (
                        <button
                          type="button"
                          key={virtualItem.key}
                          data-index={virtualItem.index}
                          ref={tocVirtualizer.measureElement}
                          className="absolute left-0 top-0 block w-full rounded px-2 py-2 text-left text-xs hover:bg-accent"
                          style={{
                            transform: `translateY(${virtualItem.start}px)`,
                          }}
                          onClick={() =>
                            messageVirtualizer.scrollToIndex(index, {
                              align: "center",
                            })
                          }
                        >
                          <span className="mr-1 text-muted-foreground">
                            {virtualItem.index + 1}.
                          </span>
                          {message.content.slice(0, 70)}
                          {message.content.length > 70 ? "..." : ""}
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>
            </aside>
          </div>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        isOpen={Boolean(deleteTargets)}
        title={t("sessionManager.deleteTitle")}
        message={t("sessionManager.deleteConfirm", {
          count: deleteTargets?.length ?? 0,
        })}
        confirmText={deleting ? t("common.loading") : t("common.delete")}
        onConfirm={() => void confirmDelete()}
        onCancel={() => {
          if (!deleting) setDeleteTargets(null);
        }}
      />
    </>
  );
}
