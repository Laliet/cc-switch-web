import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Download, Loader2, Plus, Server } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useAllMcpServers, useToggleMcpApp } from "@/hooks/useMcp";
import type { AppId } from "@/lib/api/types";
import type { McpServer } from "@/types";
import McpFormModal from "./McpFormModal";
import { ConfirmDialog } from "../ConfirmDialog";
import { useDeleteMcpServer } from "@/hooks/useMcp";
import { toast } from "sonner";
import UnifiedMcpListItem from "./McpListItem";
import { mcpApi } from "@/lib/api/mcp";

interface UnifiedMcpPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * 统一 MCP 管理面板
 * v3.7.0 新架构：所有 MCP 服务器统一管理，每个服务器通过复选框控制应用到哪些客户端
 */
const UnifiedMcpPanel: React.FC<UnifiedMcpPanelProps> = ({
  open,
  onOpenChange,
}) => {
  const { t } = useTranslation();
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [confirmDialog, setConfirmDialog] = useState<{
    isOpen: boolean;
    title: string;
    message: string;
    onConfirm: () => void;
  } | null>(null);

  // Queries and Mutations
  const {
    data: serversMap,
    isLoading,
    isError,
    error: queryError,
    refetch,
  } = useAllMcpServers();
  const toggleAppMutation = useToggleMcpApp();
  const deleteServerMutation = useDeleteMcpServer();
  const [togglingIds, setTogglingIds] = useState<Set<string>>(() => new Set());
  const [deletingIds, setDeletingIds] = useState<Set<string>>(() => new Set());
  const [isImporting, setIsImporting] = useState(false);
  const isMountedRef = useRef(true);

  useEffect(() => {
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  // Convert serversMap to array for easier rendering
  const serverEntries = useMemo((): Array<[string, McpServer]> => {
    if (!serversMap) return [];
    return Object.entries(serversMap);
  }, [serversMap]);

  // Count enabled servers per app
  const enabledCounts = useMemo(() => {
    const counts = { claude: 0, codex: 0, gemini: 0, opencode: 0 };
    serverEntries.forEach(([_, server]) => {
      if (server.apps?.claude) counts.claude++;
      if (server.apps?.codex) counts.codex++;
      if (server.apps?.gemini) counts.gemini++;
      if (server.apps?.opencode) counts.opencode++;
    });
    return counts;
  }, [serverEntries]);

  const handleToggleApp = async (
    serverId: string,
    app: AppId,
    enabled: boolean,
  ) => {
    if (togglingIds.has(serverId) || deletingIds.has(serverId)) {
      return;
    }

    setTogglingIds((prev) => {
      const next = new Set(prev);
      next.add(serverId);
      return next;
    });

    try {
      await toggleAppMutation.mutateAsync({ serverId, app, enabled });
    } catch (error) {
      toast.error(t("common.error"), {
        description: String(error),
      });
    } finally {
      if (isMountedRef.current) {
        setTogglingIds((prev) => {
          const next = new Set(prev);
          next.delete(serverId);
          return next;
        });
      }
    }
  };

  const handleEdit = (id: string) => {
    setEditingId(id);
    setIsFormOpen(true);
  };

  const handleAdd = () => {
    setEditingId(null);
    setIsFormOpen(true);
  };

  const handleDelete = (id: string) => {
    if (deletingIds.has(id)) {
      return;
    }

    setConfirmDialog({
      isOpen: true,
      title: t("mcp.unifiedPanel.deleteServer"),
      message: t("mcp.unifiedPanel.deleteConfirm", { id }),
      onConfirm: async () => {
        try {
          setDeletingIds((prev) => {
            const next = new Set(prev);
            next.add(id);
            return next;
          });

          await deleteServerMutation.mutateAsync(id);
          if (isMountedRef.current) {
            setConfirmDialog(null);
          }
          toast.success(t("common.success"));
        } catch (error) {
          toast.error(t("common.error"), {
            description: String(error),
          });
        } finally {
          if (isMountedRef.current) {
            setDeletingIds((prev) => {
              const next = new Set(prev);
              next.delete(id);
              return next;
            });
          }
        }
      },
    });
  };

  const handleCloseForm = () => {
    setIsFormOpen(false);
    setEditingId(null);
  };

  const handleImportFromApps = async () => {
    if (isImporting) return;
    setIsImporting(true);
    try {
      const result = await mcpApi.importFromApps();
      await refetch();
      const failed = result.sources.filter((source) => source.error);
      const description = [
        `${result.imported} new`,
        `${result.merged} merged`,
        `${result.conflicts} conflicts kept local`,
        ...(failed.length
          ? [`${failed.map((source) => source.source).join(", ")} failed`]
          : []),
      ].join(" · ");
      if (failed.length) {
        toast.warning(
          t("mcp.importPartial", {
            defaultValue: "MCP import completed with errors",
          }),
          {
            description,
          },
        );
      } else {
        toast.success(
          t("mcp.importComplete", { defaultValue: "MCP import complete" }),
          {
            description,
          },
        );
      }
    } catch (error) {
      toast.error(
        t("mcp.importFailed", { defaultValue: "MCP import failed" }),
        {
          description: String(error),
        },
      );
    } finally {
      if (isMountedRef.current) setIsImporting(false);
    }
  };

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-3xl max-h-[85vh] min-h-[600px] flex flex-col">
          <DialogHeader>
            <div className="flex items-center justify-between pr-8">
              <DialogTitle>{t("mcp.unifiedPanel.title")}</DialogTitle>
              <div className="flex items-center gap-2">
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => void handleImportFromApps()}
                  disabled={isImporting}
                  title={t("mcp.importFromHostApps", {
                    defaultValue:
                      "Import from server-host client configurations",
                  })}
                >
                  {isImporting ? (
                    <Loader2 size={16} className="animate-spin" />
                  ) : (
                    <Download size={16} />
                  )}
                  {t("mcp.import", { defaultValue: "Import" })}
                </Button>
                <Button type="button" variant="mcp" onClick={handleAdd}>
                  <Plus size={16} />
                  {t("mcp.unifiedPanel.addServer")}
                </Button>
              </div>
            </div>
            <DialogDescription>
              {t("mcp.unifiedPanel.description", {
                defaultValue: "集中管理 MCP 服务器并控制其应用范围。",
              })}
            </DialogDescription>
          </DialogHeader>

          {/* Info Section */}
          <div className="flex-shrink-0 px-6 py-4">
            <div className="text-sm text-gray-500 dark:text-gray-400">
              {t("mcp.serverCount", { count: serverEntries.length })} ·{" "}
              {t("mcp.unifiedPanel.apps.claude")}: {enabledCounts.claude} ·{" "}
              {t("mcp.unifiedPanel.apps.codex")}: {enabledCounts.codex} ·{" "}
              {t("mcp.unifiedPanel.apps.gemini")}: {enabledCounts.gemini} ·{" "}
              {t("mcp.unifiedPanel.apps.opencode")}: {enabledCounts.opencode}
            </div>
          </div>

          {/* Content - Scrollable */}
          <div className="flex-1 overflow-y-auto px-6 pb-4">
            {isLoading ? (
              <div className="text-center py-12 text-gray-500 dark:text-gray-400">
                {t("mcp.loading")}
              </div>
            ) : isError ? (
              <div className="text-center py-12">
                <div className="text-red-500 dark:text-red-400 font-medium">
                  {t("mcp.loadFailed", {
                    defaultValue: "Failed to load MCP servers",
                  })}
                </div>
                {queryError && (
                  <p className="mt-2 text-sm text-muted-foreground break-words">
                    {queryError instanceof Error
                      ? queryError.message
                      : String(queryError)}
                  </p>
                )}
                <Button
                  type="button"
                  variant="outline"
                  className="mt-4"
                  onClick={() => refetch()}
                >
                  {t("common.retry", { defaultValue: "Retry" })}
                </Button>
              </div>
            ) : serverEntries.length === 0 ? (
              <div className="text-center py-12">
                <div className="w-16 h-16 mx-auto mb-4 bg-gray-100 dark:bg-gray-800 rounded-full flex items-center justify-center">
                  <Server
                    size={24}
                    className="text-gray-400 dark:text-gray-500"
                  />
                </div>
                <h3 className="text-lg font-medium text-gray-900 dark:text-gray-100 mb-2">
                  {t("mcp.unifiedPanel.noServers")}
                </h3>
                <p className="text-gray-500 dark:text-gray-400 text-sm">
                  {t("mcp.emptyDescription")}
                </p>
              </div>
            ) : (
              <div className="space-y-3">
                {serverEntries.map(([id, server]) => (
                  <UnifiedMcpListItem
                    key={id}
                    id={id}
                    server={server}
                    isBusy={togglingIds.has(id) || deletingIds.has(id)}
                    isDeleting={deletingIds.has(id)}
                    onToggleApp={handleToggleApp}
                    onEdit={handleEdit}
                    onDelete={handleDelete}
                  />
                ))}
              </div>
            )}
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="mcp"
              onClick={() => onOpenChange(false)}
            >
              <Check size={16} />
              {t("common.done")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Form Modal */}
      {isFormOpen && (
        <McpFormModal
          editingId={editingId || undefined}
          initialData={
            editingId && serversMap ? serversMap[editingId] : undefined
          }
          existingIds={serversMap ? Object.keys(serversMap) : []}
          defaultFormat="json"
          onSave={async () => {
            setIsFormOpen(false);
            setEditingId(null);
          }}
          onClose={handleCloseForm}
        />
      )}

      {/* Confirm Dialog */}
      {confirmDialog && (
        <ConfirmDialog
          isOpen={confirmDialog.isOpen}
          title={confirmDialog.title}
          message={confirmDialog.message}
          onConfirm={confirmDialog.onConfirm}
          onCancel={() => setConfirmDialog(null)}
        />
      )}
    </>
  );
};

export default UnifiedMcpPanel;
