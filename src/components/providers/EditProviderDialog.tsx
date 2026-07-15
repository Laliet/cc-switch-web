import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Save } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import type { Provider } from "@/types";
import {
  ProviderForm,
  type ProviderFormValues,
} from "@/components/providers/forms/ProviderForm";
import { providersApi, vscodeApi, type AppId } from "@/lib/api";

interface EditProviderDialogProps {
  open: boolean;
  provider: Provider | null;
  onOpenChange: (open: boolean) => void;
  onSubmit: (provider: Provider) => Promise<void> | void;
  appId: AppId;
}

export function EditProviderDialog({
  open,
  provider,
  onOpenChange,
  onSubmit,
  appId,
}: EditProviderDialogProps) {
  const { t } = useTranslation();

  // 默认使用传入的 provider.settingsConfig，若当前编辑对象是“当前生效供应商”，则尝试读取实时配置替换初始值
  const [liveSettings, setLiveSettings] = useState<Record<
    string,
    unknown
  > | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      if (!open || !provider) {
        setLiveSettings(null);
        return;
      }
      try {
        const currentId = await providersApi.getCurrent(appId);
        if (currentId && provider.id === currentId) {
          try {
            const live = (await vscodeApi.getLiveProviderSettings(
              appId,
            )) as Record<string, unknown>;
            if (!cancelled && live && typeof live === "object") {
              setLiveSettings(live);
            }
          } catch {
            // 读取实时配置失败则回退到 SSOT（不打断编辑流程）
            if (!cancelled) setLiveSettings(null);
          }
        } else {
          if (!cancelled) setLiveSettings(null);
        }
      } finally {
        // no-op
      }
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, [open, provider, appId]);

  const initialSettingsConfig = useMemo(() => {
    if (appId === "opencode" && liveSettings && provider) {
      const providerMap = liveSettings.provider;
      if (providerMap && typeof providerMap === "object") {
        const currentSettings = (providerMap as Record<string, unknown>)[
          provider.id
        ];
        if (currentSettings && typeof currentSettings === "object") {
          return currentSettings as Record<string, unknown>;
        }
      }

      return (provider.settingsConfig ?? {}) as Record<string, unknown>;
    }

    if (appId === "openclaw" && liveSettings && provider) {
      const providerMap = (
        liveSettings.models as Record<string, unknown> | undefined
      )?.providers;
      if (providerMap && typeof providerMap === "object") {
        const currentSettings = (providerMap as Record<string, unknown>)[
          provider.id
        ];
        if (currentSettings && typeof currentSettings === "object") {
          return currentSettings as Record<string, unknown>;
        }
      }
      return (provider.settingsConfig ?? {}) as Record<string, unknown>;
    }

    return (liveSettings ?? provider?.settingsConfig ?? {}) as Record<
      string,
      unknown
    >;
  }, [appId, liveSettings, provider]);

  const handleSubmit = useCallback(
    async (values: ProviderFormValues) => {
      if (!provider) return;

      const parsedConfig = JSON.parse(values.settingsConfig) as Record<
        string,
        unknown
      >;

      const updatedProvider: Provider = {
        ...provider,
        name: values.name.trim(),
        notes: values.notes?.trim() || undefined,
        websiteUrl: values.websiteUrl?.trim() || undefined,
        settingsConfig: parsedConfig,
        ...(values.presetCategory ? { category: values.presetCategory } : {}),
        // 保留或更新 meta 字段
        ...(values.meta ? { meta: values.meta } : {}),
      };

      await onSubmit(updatedProvider);
      onOpenChange(false);
    },
    [onSubmit, onOpenChange, provider],
  );

  if (!provider) {
    return null;
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl max-h-[85vh] min-h-[600px] flex flex-col">
        <DialogHeader>
          <DialogTitle>{t("provider.editProvider")}</DialogTitle>
          <DialogDescription>
            {t("provider.editProviderHint")}
          </DialogDescription>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto px-6 py-4">
          <ProviderForm
            appId={appId}
            providerId={provider.id}
            submitLabel={t("common.save")}
            onSubmit={handleSubmit}
            onCancel={() => onOpenChange(false)}
            initialData={{
              name: provider.name,
              notes: provider.notes,
              websiteUrl: provider.websiteUrl,
              // 若读取到实时配置则优先使用
              settingsConfig: initialSettingsConfig,
              category: provider.category,
              meta: provider.meta,
            }}
            showButtons={false}
          />
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button type="submit" form="provider-form">
            <Save className="h-4 w-4" />
            {t("common.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
