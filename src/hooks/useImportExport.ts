import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { settingsApi } from "@/lib/api";
import { syncCurrentProvidersLiveSafe } from "@/utils/postChangeSync";
import {
  buildWebApiUrl,
  buildWebAuthHeadersForUrl,
  isWeb,
} from "@/lib/api/adapter";

export type ImportStatus =
  | "idle"
  | "importing"
  | "success"
  | "partial-success"
  | "error";

export interface UseImportExportOptions {
  onImportSuccess?: () => void | Promise<void>;
}

export interface UseImportExportResult {
  selectedFile: string;
  status: ImportStatus;
  errorMessage: string | null;
  backupId: string | null;
  isImporting: boolean;
  selectImportFile: () => Promise<void>;
  clearSelection: () => void;
  importConfig: () => Promise<void>;
  exportConfig: () => Promise<void>;
  resetStatus: () => void;
}

export function useImportExport(
  options: UseImportExportOptions = {},
): UseImportExportResult {
  const { t } = useTranslation();
  const { onImportSuccess } = options;

  const [selectedFile, setSelectedFile] = useState("");
  const [selectedFileContent, setSelectedFileContent] = useState<string | null>(
    null,
  );
  const [status, setStatus] = useState<ImportStatus>("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [backupId, setBackupId] = useState<string | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  const successTimerRef = useRef<number | null>(null);

  const clearSuccessTimer = useCallback(() => {
    if (successTimerRef.current) {
      window.clearTimeout(successTimerRef.current);
      successTimerRef.current = null;
    }
  }, []);

  useEffect(() => {
    return () => {
      clearSuccessTimer();
    };
  }, [clearSuccessTimer]);

  const clearSelection = useCallback(() => {
    setSelectedFile("");
    setSelectedFileContent(null);
    setStatus("idle");
    setErrorMessage(null);
    setBackupId(null);
  }, []);

  const selectImportFile = useCallback(async () => {
    if (isWeb()) {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = ".sql,.json,application/sql,application/json,text/plain";
      input.onchange = async () => {
        const file = input.files?.[0];
        if (!file) return;
        try {
          const text = await file.text();
          setSelectedFile(file.name);
          setSelectedFileContent(text);
          setStatus("idle");
          setErrorMessage(null);
        } catch (error) {
          console.error("[useImportExport] Failed to read file", error);
          toast.error(
            t("settings.selectFileFailed", {
              defaultValue: "选择文件失败",
            }),
          );
        }
      };
      input.click();
      return;
    }

    try {
      const filePath = await settingsApi.openFileDialog();
      if (filePath) {
        setSelectedFile(filePath);
        setSelectedFileContent(null);
        setStatus("idle");
        setErrorMessage(null);
      }
    } catch (error) {
      console.error("[useImportExport] Failed to open file dialog", error);
      toast.error(
        t("settings.selectFileFailed", {
          defaultValue: "选择文件失败",
        }),
      );
    }
  }, [t]);

  const importConfig = useCallback(async () => {
    if (!selectedFile) {
      toast.error(
        t("settings.selectFileFailed", {
          defaultValue: "请选择有效的配置文件",
        }),
      );
      return;
    }

    if (isImporting) return;

    clearSuccessTimer();
    setIsImporting(true);
    setStatus("importing");
    setErrorMessage(null);

    try {
      const result = await settingsApi.importConfigFromFile(
        selectedFile,
        selectedFileContent ?? undefined,
      );
      if (!result.success) {
        setStatus("error");
        const message =
          result.message ||
          t("settings.configCorrupted", {
            defaultValue: "配置文件已损坏或格式不正确",
          });
        setErrorMessage(message);
        toast.error(message);
        return;
      }

      setBackupId(result.backupId ?? null);

      const syncResult = await syncCurrentProvidersLiveSafe();
      if (syncResult.ok) {
        setStatus("success");
        toast.success(
          t("settings.importSuccess", {
            defaultValue: "配置导入成功",
          }),
        );

        successTimerRef.current = window.setTimeout(() => {
          successTimerRef.current = null;
          void onImportSuccess?.();
        }, 1500);
      } else {
        console.error(
          "[useImportExport] Failed to sync live config",
          syncResult.error,
        );
        setStatus("partial-success");
        toast.warning(
          t("settings.importPartialSuccess", {
            defaultValue:
              "配置已导入，但同步到当前供应商失败。请手动重新选择一次供应商。",
          }),
        );
      }
    } catch (error) {
      console.error("[useImportExport] Failed to import config", error);
      setStatus("error");
      const message =
        error instanceof Error ? error.message : String(error ?? "");
      setErrorMessage(message);
      toast.error(
        t("settings.importFailedError", {
          defaultValue: "导入配置失败: {{message}}",
          message,
        }),
      );
    } finally {
      setIsImporting(false);
    }
  }, [
    clearSuccessTimer,
    isImporting,
    onImportSuccess,
    selectedFile,
    selectedFileContent,
    t,
  ]);

  const exportConfig = useCallback(async () => {
    if (isWeb()) {
      try {
        const defaultName = `cc-switch-backup-${
          new Date().toISOString().split("T")[0]
        }.sql`;
        const exportUrl = buildWebApiUrl("/config/export");
        const headers: Record<string, string> = {
          Accept: "application/sql,text/plain",
          ...buildWebAuthHeadersForUrl(exportUrl),
        };
        const response = await fetch(exportUrl, {
          credentials: "include",
          headers,
        });
        if (!response.ok) {
          throw new Error(await response.text());
        }
        const data = await response.text();
        const blob = new Blob([data], {
          type: "application/sql;charset=utf-8",
        });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;
        link.download = defaultName;
        link.click();
        window.setTimeout(() => URL.revokeObjectURL(url), 0);
        toast.success(
          t("settings.configExported", {
            defaultValue: "配置已导出",
          }),
        );
      } catch (error) {
        console.error("[useImportExport] Failed to export config (web)", error);
        toast.error(
          t("settings.exportFailedError", {
            defaultValue: "导出配置失败: {{message}}",
            message:
              error instanceof Error ? error.message : String(error ?? ""),
          }),
        );
      }
      return;
    }

    try {
      const defaultName = `cc-switch-backup-${
        new Date().toISOString().split("T")[0]
      }.sql`;
      const destination = await settingsApi.saveFileDialog(defaultName);
      if (!destination) {
        toast.error(
          t("settings.selectFileFailed", {
            defaultValue: "选择保存位置失败",
          }),
        );
        return;
      }

      const result = await settingsApi.exportConfigToFile(destination);
      if (result.success) {
        const displayPath = result.filePath ?? destination;
        toast.success(
          t("settings.configExported", {
            defaultValue: "配置已导出",
          }) + `\n${displayPath}`,
        );
      } else {
        toast.error(
          t("settings.exportFailed", {
            defaultValue: "导出配置失败",
          }) + (result.message ? `: ${result.message}` : ""),
        );
      }
    } catch (error) {
      console.error("[useImportExport] Failed to export config", error);
      toast.error(
        t("settings.exportFailedError", {
          defaultValue: "导出配置失败: {{message}}",
          message: error instanceof Error ? error.message : String(error ?? ""),
        }),
      );
    }
  }, [t]);

  const resetStatus = useCallback(() => {
    setStatus("idle");
    setErrorMessage(null);
    setBackupId(null);
  }, []);

  return {
    selectedFile,
    status,
    errorMessage,
    backupId,
    isImporting,
    selectImportFile,
    clearSelection,
    importConfig,
    exportConfig,
    resetStatus,
  };
}
