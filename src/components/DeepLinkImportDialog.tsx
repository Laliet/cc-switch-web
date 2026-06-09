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
      await deeplinkApi.importFromDeeplink(request);

      // Invalidate provider queries to refresh the list
      await queryClient.invalidateQueries({
        queryKey: ["providers", request.app],
      });

      toast.success(t("deeplink.importSuccess"), {
        description: t("deeplink.importSuccessDescription", {
          name: request.name,
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

  const maskedApiKey =
    maskApiKey(request.apiKey, { keepStart: 4, keepEnd: 4 }) || "***";

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
          {/* App Type */}
          <div className="grid grid-cols-3 items-center gap-4">
            <div className="font-medium text-sm text-muted-foreground">
              {t("deeplink.app")}
            </div>
            <div className="col-span-2 text-sm font-medium capitalize">
              {request.app}
            </div>
          </div>

          {/* Provider Name */}
          <div className="grid grid-cols-3 items-center gap-4">
            <div className="font-medium text-sm text-muted-foreground">
              {t("deeplink.providerName")}
            </div>
            <div className="col-span-2 text-sm font-medium">{request.name}</div>
          </div>

          {/* Homepage */}
          <div className="grid grid-cols-3 items-center gap-4">
            <div className="font-medium text-sm text-muted-foreground">
              {t("deeplink.homepage")}
            </div>
            <div className="col-span-2 text-sm break-all text-blue-600 dark:text-blue-400">
              {request.homepage}
            </div>
          </div>

          {/* API Endpoint */}
          <div className="grid grid-cols-3 items-center gap-4">
            <div className="font-medium text-sm text-muted-foreground">
              {t("deeplink.endpoint")}
            </div>
            <div className="col-span-2 text-sm break-all">
              {request.endpoint}
            </div>
          </div>

          {/* API Key (masked) */}
          <div className="grid grid-cols-3 items-center gap-4">
            <div className="font-medium text-sm text-muted-foreground">
              {t("deeplink.apiKey")}
            </div>
            <div className="col-span-2 text-sm font-mono text-muted-foreground">
              {maskedApiKey}
            </div>
          </div>

          {/* Model (if present) */}
          {request.model && (
            <div className="grid grid-cols-3 items-center gap-4">
              <div className="font-medium text-sm text-muted-foreground">
                {t("deeplink.model")}
              </div>
              <div className="col-span-2 text-sm font-mono">
                {request.model}
              </div>
            </div>
          )}

          {/* Notes (if present) */}
          {request.notes && (
            <div className="grid grid-cols-3 items-start gap-4">
              <div className="font-medium text-sm text-muted-foreground">
                {t("deeplink.notes")}
              </div>
              <div className="col-span-2 text-sm text-muted-foreground">
                {request.notes}
              </div>
            </div>
          )}

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
