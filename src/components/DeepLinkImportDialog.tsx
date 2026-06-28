import { useState, useEffect } from "react";
import { DeepLinkImportRequest, deeplinkApi } from "@/lib/api/deeplink";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";
import { isWeb } from "@/lib/api/adapter";

interface DeeplinkError {
  url: string;
  error: string;
}

interface DetailRow {
  label: string;
  value: string | null | undefined;
  monospace?: boolean;
  link?: boolean;
  multiline?: boolean;
}

function maskApiKey(
  apiKey: string | null | undefined,
  options?: { keepStart?: number; keepEnd?: number },
): string {
  if (!apiKey) return "";

  const keepStart = options?.keepStart ?? 4;
  const keepEnd = options?.keepEnd ?? 4;
  const value = apiKey;

  if (value.length <= keepStart + keepEnd) {
    if (value.length <= 4) return "***";

    const shortKeepStart = Math.min(2, value.length);
    const shortKeepEnd = Math.min(2, value.length - shortKeepStart);
    return `${value.slice(0, shortKeepStart)}***${
      shortKeepEnd > 0 ? value.slice(-shortKeepEnd) : ""
    }`;
  }

  return `${value.slice(0, keepStart)}***${value.slice(-keepEnd)}`;
}

function decodedPreview(value: string | null | undefined): string | null {
  if (!value) return null;
  try {
    const normalized = value.replace(/\s/g, "+");
    const padded =
      normalized.length % 4 === 0
        ? normalized
        : normalized + "=".repeat(4 - (normalized.length % 4));
    const decoded = decodeURIComponent(
      Array.from(atob(padded))
        .map((char) => `%${char.charCodeAt(0).toString(16).padStart(2, "0")}`)
        .join(""),
    );
    return decoded.length > 240 ? `${decoded.slice(0, 240)}...` : decoded;
  } catch {
    return null;
  }
}

export function DeepLinkImportDialog() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [request, setRequest] = useState<DeepLinkImportRequest | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    if (isWeb()) {
      return;
    }

    // Listen for deep link import events
    const unlistenImport = import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<DeepLinkImportRequest>("deeplink-import", (event) => {
          console.log("Deep link import event received:", {
            app: event.payload?.app,
            name: event.payload?.name,
            apiKey: maskApiKey(event.payload?.apiKey, {
              keepStart: 2,
              keepEnd: 2,
            }),
          });
          setRequest(event.payload);
          setIsOpen(true);
        }),
      )
      .catch((error) => {
        console.error("Failed to subscribe deeplink-import", error);
        return () => {};
      });

    // Listen for deep link error events
    const unlistenError = import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<DeeplinkError>("deeplink-error", (event) => {
          console.error("Deep link error:", event.payload);
          toast.error(t("deeplink.parseError"), {
            description: event.payload.error,
          });
        }),
      )
      .catch((error) => {
        console.error("Failed to subscribe deeplink-error", error);
        return () => {};
      });

    return () => {
      unlistenImport.then((fn) => fn());
      unlistenError.then((fn) => fn());
    };
  }, [t]);

  const handleImport = async () => {
    if (!request) return;

    setIsImporting(true);

    try {
      const result = await deeplinkApi.importFromDeeplink(request);

      if (result.type === "provider" && request.app) {
        await queryClient.invalidateQueries({
          queryKey: ["providers", request.app],
        });
      } else if (result.type === "prompt" && request.app) {
        await queryClient.invalidateQueries({
          queryKey: ["prompts", request.app],
        });
      } else if (result.type === "mcp") {
        await queryClient.invalidateQueries({ queryKey: ["mcp", "all"] });
      } else if (result.type === "skill") {
        await queryClient.invalidateQueries({ queryKey: ["skills"] });
      }

      const importedName =
        request.name ||
        request.repo ||
        (result.type === "provider" || result.type === "prompt"
          ? result.id
          : result.type === "skill"
            ? result.key || result.result.key
            : result.importedIds?.join(", ")) ||
        result.type;

      toast.success(t("deeplink.importSuccess"), {
        description: t("deeplink.importSuccessDescription", {
          name: importedName,
        }),
      });

      setIsOpen(false);
      setRequest(null);
    } catch (error) {
      console.error("Failed to import provider from deep link:", error);
      toast.error(t("deeplink.importError"), {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setIsImporting(false);
    }
  };

  const handleCancel = () => {
    setIsOpen(false);
    setRequest(null);
  };

  if (!request) return null;

  const resourceLabel = t(`deeplink.resource.${request.resource}`, {
    defaultValue: request.resource,
  });
  const details: DetailRow[] =
    request.resource === "provider"
      ? [
          { label: t("deeplink.app"), value: request.app },
          { label: t("deeplink.providerName"), value: request.name },
          {
            label: t("deeplink.homepage"),
            value: request.homepage,
            link: true,
          },
          { label: t("deeplink.endpoint"), value: request.endpoint },
          {
            label: t("deeplink.apiKey"),
            value:
              maskApiKey(request.apiKey, { keepStart: 4, keepEnd: 4 }) || "***",
            monospace: true,
          },
          { label: t("deeplink.model"), value: request.model, monospace: true },
          { label: t("deeplink.notes"), value: request.notes, multiline: true },
        ]
      : request.resource === "prompt"
        ? [
            { label: t("deeplink.app"), value: request.app },
            {
              label: t("deeplink.name", { defaultValue: "名称" }),
              value: request.name,
            },
            {
              label: t("deeplink.description", { defaultValue: "描述" }),
              value: request.description,
              multiline: true,
            },
            {
              label: t("deeplink.enabled", { defaultValue: "启用" }),
              value: request.enabled
                ? t("common.yes", { defaultValue: "是" })
                : t("common.no", { defaultValue: "否" }),
            },
            {
              label: t("deeplink.contentPreview", { defaultValue: "内容预览" }),
              value: decodedPreview(request.content),
              multiline: true,
            },
          ]
        : request.resource === "mcp"
          ? [
              {
                label: t("deeplink.apps", { defaultValue: "应用" }),
                value: request.apps,
              },
              {
                label: t("deeplink.configPreview", {
                  defaultValue: "配置预览",
                }),
                value: decodedPreview(request.config),
                monospace: true,
                multiline: true,
              },
            ]
          : [
              { label: t("deeplink.app"), value: request.app },
              {
                label: t("deeplink.repo", { defaultValue: "仓库" }),
                value: request.repo,
                monospace: true,
              },
              {
                label: t("deeplink.branch", { defaultValue: "分支" }),
                value: request.branch || "main",
                monospace: true,
              },
              {
                label: t("deeplink.directory", { defaultValue: "目录" }),
                value: request.directory,
                monospace: true,
              },
              {
                label: t("deeplink.skillsPath", {
                  defaultValue: "Skills 路径",
                }),
                value: request.skillsPath,
                monospace: true,
              },
            ];

  return (
    <Dialog open={isOpen} onOpenChange={setIsOpen}>
      <DialogContent className="sm:max-w-[500px]">
        {/* 标题显式左对齐，避免默认居中样式影响 */}
        <DialogHeader className="text-left sm:text-left">
          <DialogTitle>{t("deeplink.confirmImport")}</DialogTitle>
          <DialogDescription>
            {t("deeplink.confirmImportDescription")}
          </DialogDescription>
        </DialogHeader>

        {/* 主体内容整体右移，略大于标题内边距，让内容看起来不贴边 */}
        <div className="space-y-4 px-8 py-4">
          <div className="grid grid-cols-3 items-center gap-4">
            <div className="font-medium text-sm text-muted-foreground">
              {t("deeplink.resourceType", { defaultValue: "类型" })}
            </div>
            <div className="col-span-2 text-sm font-medium">
              {resourceLabel}
            </div>
          </div>

          {details
            .filter(
              (row) =>
                row.value !== undefined &&
                row.value !== null &&
                row.value !== "",
            )
            .map((row) => (
              <div
                key={row.label}
                className={`grid grid-cols-3 gap-4 ${
                  row.multiline ? "items-start" : "items-center"
                }`}
              >
                <div className="font-medium text-sm text-muted-foreground">
                  {row.label}
                </div>
                <div
                  className={`col-span-2 text-sm break-all ${
                    row.monospace ? "font-mono" : ""
                  } ${
                    row.link
                      ? "text-blue-600 dark:text-blue-400"
                      : "text-foreground"
                  } ${
                    row.multiline
                      ? "max-h-32 overflow-auto whitespace-pre-wrap rounded border bg-muted/40 p-2"
                      : ""
                  }`}
                >
                  {row.value}
                </div>
              </div>
            ))}

          {/* Warning */}
          <div className="rounded-lg bg-yellow-50 dark:bg-yellow-900/20 p-3 text-sm text-yellow-800 dark:text-yellow-200">
            {t("deeplink.warning")}
          </div>
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            onClick={handleCancel}
            disabled={isImporting}
          >
            {t("common.cancel")}
          </Button>
          <Button onClick={handleImport} disabled={isImporting}>
            {isImporting ? t("deeplink.importing") : t("deeplink.import")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
