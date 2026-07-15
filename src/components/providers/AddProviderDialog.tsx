import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Plus } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import type { Provider, CustomEndpoint } from "@/types";
import type { AppId } from "@/lib/api";
import {
  ProviderForm,
  type ProviderFormValues,
} from "@/components/providers/forms/ProviderForm";
import { providerPresets } from "@/config/claudeProviderPresets";
import { codexProviderPresets } from "@/config/codexProviderPresets";
import { geminiProviderPresets } from "@/config/geminiProviderPresets";
import { claudeDesktopProviderPresets } from "@/config/claudeDesktopProviderPresets";

interface AddProviderDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  appId: AppId;
  onSubmit: (
    provider: Omit<Provider, "id"> & { id?: string },
  ) => Promise<void> | void;
}

export function AddProviderDialog({
  open,
  onOpenChange,
  appId,
  onSubmit,
}: AddProviderDialogProps) {
  const { t } = useTranslation();

  const handleSubmit = useCallback(
    async (values: ProviderFormValues) => {
      const parsedConfig = JSON.parse(values.settingsConfig) as Record<
        string,
        unknown
      >;

      // 构造基础提交数据
      const providerData: Omit<Provider, "id"> & { id?: string } = {
        ...(appId === "openclaw" && values.providerKey
          ? { id: values.providerKey }
          : {}),
        name: values.name.trim(),
        notes: values.notes?.trim() || undefined,
        websiteUrl: values.websiteUrl?.trim() || undefined,
        settingsConfig: parsedConfig,
        ...(values.presetCategory ? { category: values.presetCategory } : {}),
        ...(values.meta ? { meta: values.meta } : {}),
      };

      const hasCustomEndpoints =
        providerData.meta?.custom_endpoints &&
        Object.keys(providerData.meta.custom_endpoints).length > 0;

      if (!hasCustomEndpoints) {
        // 收集端点候选（仅在缺少自定义端点时兜底）
        // 1. 从预设配置中获取 endpointCandidates
        // 2. 从当前配置中提取 baseUrl (ANTHROPIC_BASE_URL 或 Codex base_url)
        const urlSet = new Set<string>();

        const addUrl = (rawUrl?: string) => {
          const url = (rawUrl || "").trim().replace(/\/+$/, "");
          if (url && url.startsWith("http")) {
            urlSet.add(url);
          }
        };

        if (values.presetId) {
          if (appId === "claude") {
            const presets = providerPresets;
            const presetIndex = parseInt(
              values.presetId.replace("claude-", ""),
            );
            if (
              !isNaN(presetIndex) &&
              presetIndex >= 0 &&
              presetIndex < presets.length
            ) {
              const preset = presets[presetIndex];
              if (preset?.endpointCandidates) {
                preset.endpointCandidates.forEach(addUrl);
              }
            }
          } else if (appId === "claude-desktop") {
            const presetIndex = parseInt(
              values.presetId.replace("claude-desktop-", ""),
            );
            const preset = claudeDesktopProviderPresets[presetIndex];
            if (preset?.endpointCandidates) {
              preset.endpointCandidates.forEach(addUrl);
            }
            addUrl(preset?.baseUrl);
          } else if (appId === "codex") {
            const presets = codexProviderPresets;
            const presetIndex = parseInt(values.presetId.replace("codex-", ""));
            if (
              !isNaN(presetIndex) &&
              presetIndex >= 0 &&
              presetIndex < presets.length
            ) {
              const preset = presets[presetIndex];
              if (Array.isArray(preset.endpointCandidates)) {
                preset.endpointCandidates.forEach(addUrl);
              }
            }
          } else if (appId === "gemini") {
            const presets = geminiProviderPresets;
            const presetIndex = parseInt(
              values.presetId.replace("gemini-", ""),
            );
            if (
              !isNaN(presetIndex) &&
              presetIndex >= 0 &&
              presetIndex < presets.length
            ) {
              const preset = presets[presetIndex];
              if (Array.isArray(preset.endpointCandidates)) {
                preset.endpointCandidates.forEach(addUrl);
              }
            }
          }
        }

        if (appId === "claude" || appId === "claude-desktop") {
          const env = parsedConfig.env as Record<string, any> | undefined;
          if (env?.ANTHROPIC_BASE_URL) {
            addUrl(env.ANTHROPIC_BASE_URL);
          }
        } else if (appId === "codex") {
          const config = parsedConfig.config as string | undefined;
          if (config) {
            const baseUrlMatch = config.match(
              /base_url\s*=\s*["']([^"']+)["']/,
            );
            if (baseUrlMatch?.[1]) {
              addUrl(baseUrlMatch[1]);
            }
          }
        } else if (appId === "gemini") {
          const env = parsedConfig.env as Record<string, any> | undefined;
          if (env?.GOOGLE_GEMINI_BASE_URL) {
            addUrl(env.GOOGLE_GEMINI_BASE_URL);
          }
        } else if (appId === "opencode") {
          const options = parsedConfig.options as
            | Record<string, any>
            | undefined;
          if (options?.baseURL) {
            addUrl(options.baseURL);
          }
        } else if (appId === "openclaw") {
          addUrl(parsedConfig.baseUrl as string | undefined);
        }

        const urls = Array.from(urlSet);
        if (urls.length > 0) {
          const now = Date.now();
          const customEndpoints: Record<string, CustomEndpoint> = {};
          urls.forEach((url) => {
            customEndpoints[url] = {
              url,
              addedAt: now,
              lastUsed: undefined,
            };
          });

          providerData.meta = {
            ...(providerData.meta ?? {}),
            custom_endpoints: customEndpoints,
          };
        }
      }

      await onSubmit(providerData);
      onOpenChange(false);
    },
    [appId, onSubmit, onOpenChange],
  );

  const submitLabel =
    appId === "claude"
      ? t("provider.addClaudeProvider")
      : appId === "codex"
        ? t("provider.addCodexProvider")
        : appId === "gemini"
          ? t("provider.addGeminiProvider")
          : t("provider.addProviderForApp", {
              defaultValue: "添加 {{appName}} 供应商",
              appName: t(`apps.${appId}`, { defaultValue: appId }),
            });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl max-h-[85vh] min-h-[600px] flex flex-col">
        <DialogHeader>
          <DialogTitle>{submitLabel}</DialogTitle>
          <DialogDescription>{t("provider.addProviderHint")}</DialogDescription>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto px-6 py-4">
          <ProviderForm
            appId={appId}
            submitLabel={t("common.add")}
            onSubmit={handleSubmit}
            onCancel={() => onOpenChange(false)}
            showButtons={false}
          />
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button type="submit" form="provider-form">
            <Plus className="h-4 w-4" />
            {t("common.add")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
