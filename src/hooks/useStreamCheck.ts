import { useState, useCallback } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import {
  streamCheckProvider,
  type StreamCheckResult,
} from "@/lib/api/model-test";
import type { AppId } from "@/lib/api";

export interface StreamCheckBatchProgress {
  running: boolean;
  completed: number;
  total: number;
  failed: number;
}

export function useStreamCheck(appId: AppId) {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const [checkingIds, setCheckingIds] = useState<Set<string>>(new Set());
  const [batchProgress, setBatchProgress] = useState<StreamCheckBatchProgress>({
    running: false,
    completed: 0,
    total: 0,
    failed: 0,
  });

  const checkProvider = useCallback(
    async (
      providerId: string,
      providerName: string,
    ): Promise<StreamCheckResult | null> => {
      setCheckingIds((prev) => new Set(prev).add(providerId));

      try {
        const result = await streamCheckProvider(appId, providerId);
        await queryClient.invalidateQueries({
          queryKey: ["stream-check-logs", appId],
        });

        if (result.status === "operational") {
          toast.success(
            t("streamCheck.operational", {
              providerName: providerName,
              responseTimeMs: result.responseTimeMs,
              defaultValue: `${providerName} 运行正常 (${result.responseTimeMs}ms)`,
            }),
            { closeButton: true },
          );
        } else if (result.status === "degraded") {
          toast.warning(
            t("streamCheck.degraded", {
              providerName: providerName,
              responseTimeMs: result.responseTimeMs,
              defaultValue: `${providerName} 响应较慢 (${result.responseTimeMs}ms)`,
            }),
          );
        } else if (result.errorCategory === "modelNotFound") {
          // 专门处理"模型不存在/已下架"：指向配置入口，比通用 404 文案更有指导性
          toast.error(
            t("streamCheck.modelNotFound", {
              providerName: providerName,
              model: result.modelUsed,
              defaultValue: `${providerName} 测试模型 ${result.modelUsed} 不存在或已下架`,
            }),
            {
              description: t("streamCheck.modelNotFoundHint", {
                defaultValue: "",
              }),
              duration: 10000,
              closeButton: true,
            },
          );
        } else if (result.errorCategory === "quotaExceeded") {
          toast.warning(
            t("streamCheck.quotaExceeded", {
              providerName: providerName,
              defaultValue: `${providerName} Coding Plan quota has been exceeded`,
            }),
            {
              description: t("streamCheck.quotaExceededHint", {
                defaultValue: "",
              }),
              duration: 10000,
              closeButton: true,
            },
          );
        } else {
          const httpStatus = result.httpStatus;
          const hintKey = httpStatus
            ? `streamCheck.httpHint.${httpStatus >= 500 ? "5xx" : httpStatus}`
            : null;
          const description =
            (hintKey ? t(hintKey, { defaultValue: "" }) : "") || undefined;

          // 401/403/400 = 检查被拒（供应商可能正常）；429/5xx = 临时问题
          const isProbeRejection =
            httpStatus != null &&
            ([401, 403, 400, 429].includes(httpStatus) || httpStatus >= 500);

          if (isProbeRejection) {
            toast.warning(
              t("streamCheck.rejected", {
                providerName: providerName,
                message: result.message,
                defaultValue: `${providerName} 检查被拒: ${result.message}`,
              }),
              { description, duration: 8000, closeButton: true },
            );
          } else {
            toast.error(
              t("streamCheck.failed", {
                providerName: providerName,
                message: result.message,
                defaultValue: `${providerName} 检查失败: ${result.message}`,
              }),
              { description, duration: 8000, closeButton: true },
            );
          }
        }

        return result;
      } catch (e) {
        toast.error(
          t("streamCheck.error", {
            providerName: providerName,
            error: String(e),
            defaultValue: `${providerName} 检查出错: ${String(e)}`,
          }),
        );
        return null;
      } finally {
        setCheckingIds((prev) => {
          const next = new Set(prev);
          next.delete(providerId);
          return next;
        });
      }
    },
    [appId, queryClient, t],
  );

  const isChecking = useCallback(
    (providerId: string) => checkingIds.has(providerId),
    [checkingIds],
  );

  const checkProviders = useCallback(
    async (providers: Array<{ id: string; name: string }>) => {
      if (providers.length === 0) return;
      setBatchProgress({
        running: true,
        completed: 0,
        total: providers.length,
        failed: 0,
      });

      let failed = 0;
      for (const provider of providers) {
        setCheckingIds((previous) => new Set(previous).add(provider.id));
        try {
          const result = await streamCheckProvider(appId, provider.id);
          if (!result.success) failed += 1;
        } catch {
          failed += 1;
        } finally {
          setCheckingIds((previous) => {
            const next = new Set(previous);
            next.delete(provider.id);
            return next;
          });
          setBatchProgress((previous) => ({
            ...previous,
            completed: previous.completed + 1,
            failed,
          }));
        }
      }

      await queryClient.invalidateQueries({
        queryKey: ["stream-check-logs", appId],
      });
      setBatchProgress((previous) => ({ ...previous, running: false }));
      if (failed === 0) {
        toast.success(
          t("streamCheck.batchSuccess", {
            defaultValue: `已完成 ${providers.length} 个 Provider 的检查`,
            count: providers.length,
          }),
        );
      } else {
        toast.warning(
          t("streamCheck.batchPartial", {
            defaultValue: `检查完成，${failed} 个 Provider 失败`,
            failed,
            total: providers.length,
          }),
        );
      }
    },
    [appId, queryClient, t],
  );

  return { checkProvider, checkProviders, isChecking, batchProgress };
}
