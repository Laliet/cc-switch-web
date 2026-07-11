import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Copy,
  Eye,
  Loader2,
  Plus,
  RotateCcw,
  Save,
  SaveAll,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Checkbox } from "@/components/ui/checkbox";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { providersApi } from "@/lib/api";
import type { ProxyAppId, UniversalProvider } from "@/types";
import {
  createUniversalProviderFromPreset,
  universalProviderPresets,
} from "@/config/universalProviderPresets";

const UNIVERSAL_APPS: Exclude<ProxyAppId, "opencode">[] = [
  "claude",
  "codex",
  "gemini",
];

const emptyProvider = (): UniversalProvider => ({
  id: "",
  name: "",
  providerType: "openai-compatible",
  apps: {
    claude: true,
    codex: true,
    gemini: true,
  },
  baseUrl: "",
  apiKey: "",
  models: {},
});

interface UniversalProvidersSectionProps {
  onProvidersChanged?: () => void | Promise<void>;
}

export function UniversalProvidersSection({
  onProvidersChanged,
}: UniversalProvidersSectionProps) {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<Record<string, UniversalProvider>>(
    {},
  );
  const [selectedId, setSelectedId] = useState<string>("");
  const [draft, setDraft] = useState<UniversalProvider>(() => emptyProvider());
  const [loading, setLoading] = useState(false);
  const [busyAction, setBusyAction] = useState<
    "save" | "save-sync" | "sync" | "copy" | "preview" | "delete" | null
  >(null);
  const [confirmSync, setConfirmSync] = useState(false);
  const [preview, setPreview] = useState<Record<string, unknown> | null>(null);

  const providerRows = useMemo(
    () =>
      Object.values(providers).sort((a, b) =>
        a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
      ),
    [providers],
  );

  const loadProviders = useCallback(async () => {
    setLoading(true);
    try {
      const rows = await providersApi.getUniversalAll();
      setProviders(rows);
    } catch (error) {
      console.warn("Failed to load universal providers", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders]);

  const selectProvider = (provider: UniversalProvider | null) => {
    if (!provider) {
      setSelectedId("");
      setDraft(emptyProvider());
      return;
    }
    setSelectedId(provider.id);
    setDraft(provider);
  };

  const updateDraft = (updates: Partial<UniversalProvider>) => {
    setDraft((current) => ({ ...current, ...updates }));
    setPreview(null);
  };

  const updateApp = (
    app: Exclude<ProxyAppId, "opencode">,
    checked: boolean,
  ) => {
    setPreview(null);
    setDraft((current) => ({
      ...current,
      apps: {
        ...current.apps,
        [app]: checked,
      },
    }));
  };

  const updateModel = (
    app: Exclude<ProxyAppId, "opencode">,
    field: string,
    value: string,
  ) => {
    setPreview(null);
    setDraft((current) => ({
      ...current,
      models: {
        ...current.models,
        [app]: {
          ...(current.models[app] ?? {}),
          [field]: value.trim() || undefined,
        },
      },
    }));
  };

  const validateDraft = () => {
    if (!draft.id.trim()) {
      toast.error(
        t("settings.universal.validation.idRequired", {
          defaultValue: "请输入 Universal Provider ID",
        }),
      );
      return false;
    }
    if (!/^[a-zA-Z0-9_-]+$/.test(draft.id.trim())) {
      toast.error(
        t("settings.universal.validation.idInvalid", {
          defaultValue: "ID 只能包含字母、数字、下划线和短横线",
        }),
      );
      return false;
    }
    if (!draft.name.trim() || !draft.baseUrl.trim() || !draft.apiKey.trim()) {
      toast.error(
        t("settings.universal.validation.required", {
          defaultValue: "名称、Base URL 和 API Key 不能为空",
        }),
      );
      return false;
    }
    if (!UNIVERSAL_APPS.some((app) => draft.apps[app])) {
      toast.error(
        t("settings.universal.validation.appRequired", {
          defaultValue: "至少选择一个同步目标应用",
        }),
      );
      return false;
    }
    return true;
  };

  const normalizedDraft = (): UniversalProvider => ({
    ...draft,
    id: draft.id.trim(),
    name: draft.name.trim(),
    providerType: draft.providerType.trim() || "openai-compatible",
    baseUrl: draft.baseUrl.trim(),
    apiKey: draft.apiKey.trim(),
    websiteUrl: draft.websiteUrl?.trim() || undefined,
    notes: draft.notes?.trim() || undefined,
  });

  const handleSave = async (saveAndSync = false) => {
    if (!validateDraft()) return;
    const isNew = !selectedId;
    setBusyAction(saveAndSync ? "save-sync" : "save");
    const provider = normalizedDraft();
    try {
      await providersApi.upsertUniversal(provider);
      if (isNew || saveAndSync) {
        await providersApi.syncUniversal(provider.id);
        await onProvidersChanged?.();
      }
      setSelectedId(provider.id);
      setDraft(provider);
      await loadProviders();
      toast.success(
        t("settings.universal.saved", {
          defaultValue:
            isNew || saveAndSync
              ? "Universal Provider 已保存并同步"
              : "Universal Provider 已保存",
        }),
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.universal.saveFailed", {
          defaultValue: "保存 Universal Provider 失败",
        }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const handleDuplicate = async () => {
    if (!draft.id) return;
    setBusyAction("copy");
    try {
      const suffix = crypto.randomUUID().slice(0, 8);
      const duplicated: UniversalProvider = {
        ...structuredClone(normalizedDraft()),
        id: `${draft.id}-copy-${suffix}`,
        name: `${draft.name} Copy`,
        createdAt: Date.now(),
      };
      await providersApi.upsertUniversal(duplicated);
      await providersApi.syncUniversal(duplicated.id);
      await loadProviders();
      await onProvidersChanged?.();
      selectProvider(duplicated);
      toast.success("Universal Provider 已复制并同步");
    } catch (error) {
      toast.error("复制 Universal Provider 失败", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setBusyAction(null);
    }
  };

  const handlePreview = async () => {
    if (!validateDraft()) return;
    setBusyAction("preview");
    try {
      const result = await providersApi.previewUniversal(normalizedDraft());
      const masked = JSON.parse(
        JSON.stringify(result, (key, value) =>
          /api.?key|token|authorization/i.test(key) && typeof value === "string"
            ? "********"
            : value,
        ),
      ) as Record<string, unknown>;
      setPreview(masked);
    } catch (error) {
      toast.error("生成配置预览失败", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setBusyAction(null);
    }
  };

  const handleSync = async () => {
    if (!draft.id.trim()) return;
    setBusyAction("sync");
    try {
      await providersApi.syncUniversal(draft.id.trim());
      await onProvidersChanged?.();
      toast.success(
        t("settings.universal.synced", {
          defaultValue: "已同步到应用 Provider",
        }),
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.universal.syncFailed", {
          defaultValue: "同步 Universal Provider 失败",
        }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const handleDelete = async () => {
    if (!selectedId) return;
    setBusyAction("delete");
    try {
      await providersApi.deleteUniversal(selectedId);
      selectProvider(null);
      await loadProviders();
      await onProvidersChanged?.();
      toast.success(
        t("settings.universal.deleted", {
          defaultValue: "Universal Provider 已删除",
        }),
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.universal.deleteFailed", {
          defaultValue: "删除 Universal Provider 失败",
        }),
        { description: message },
      );
    } finally {
      setBusyAction(null);
    }
  };

  const isBusy = loading || busyAction !== null;

  return (
    <section className="space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium">
            {t("settings.universal.title", {
              defaultValue: "Universal Provider",
            })}
          </h3>
          <p className="text-xs text-muted-foreground">
            {t("settings.universal.description", {
              defaultValue:
                "维护一次通用 OpenAI-compatible 凭据，并同步生成 Claude、Codex、Gemini Provider。",
            })}
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="gap-1"
          onClick={() => selectProvider(null)}
          disabled={isBusy}
        >
          <Plus className="h-3.5 w-3.5" />
          {t("common.add", { defaultValue: "新增" })}
        </Button>
      </div>

      <div className="flex flex-wrap gap-2">
        {universalProviderPresets.map((preset) => (
          <Button
            key={preset.providerType}
            type="button"
            variant="outline"
            size="sm"
            disabled={isBusy}
            onClick={() => {
              setSelectedId("");
              setDraft(createUniversalProviderFromPreset(preset));
              setPreview(null);
            }}
          >
            {preset.name}
          </Button>
        ))}
      </div>

      <div className="grid gap-3 lg:grid-cols-[220px_1fr]">
        <div className="rounded-md border">
          <div className="border-b px-3 py-2 text-xs font-medium text-muted-foreground">
            {loading
              ? t("common.loading", { defaultValue: "加载中" })
              : t("settings.universal.providers", {
                  defaultValue: "已保存 Provider",
                })}
          </div>
          <div className="max-h-64 overflow-y-auto p-1">
            {providerRows.length === 0 ? (
              <div className="px-2 py-3 text-xs text-muted-foreground">
                {t("settings.universal.empty", {
                  defaultValue: "暂无 Universal Provider",
                })}
              </div>
            ) : (
              providerRows.map((provider) => (
                <button
                  key={provider.id}
                  type="button"
                  className={`w-full rounded px-2 py-2 text-left text-sm ${
                    selectedId === provider.id
                      ? "bg-muted text-foreground"
                      : "text-muted-foreground hover:bg-muted/60"
                  }`}
                  onClick={() => selectProvider(provider)}
                >
                  <div className="truncate font-medium">{provider.name}</div>
                  <div className="truncate text-xs">{provider.id}</div>
                </button>
              ))
            )}
          </div>
        </div>

        <div className="space-y-3 rounded-md border p-3">
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-id">ID</Label>
              <Input
                id="cc-switch-universal-id"
                value={draft.id}
                onChange={(event) => updateDraft({ id: event.target.value })}
                disabled={Boolean(selectedId)}
                placeholder="newapi"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-name">
                {t("settings.universal.name", { defaultValue: "名称" })}
              </Label>
              <Input
                id="cc-switch-universal-name"
                value={draft.name}
                onChange={(event) => updateDraft({ name: event.target.value })}
                placeholder="NewAPI"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-base-url">Base URL</Label>
              <Input
                id="cc-switch-universal-base-url"
                value={draft.baseUrl}
                onChange={(event) =>
                  updateDraft({ baseUrl: event.target.value })
                }
                placeholder="https://api.example.com"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-api-key">API Key</Label>
              <Input
                id="cc-switch-universal-api-key"
                value={draft.apiKey}
                onChange={(event) =>
                  updateDraft({ apiKey: event.target.value })
                }
                type="text"
                autoComplete="off"
                className="secret-text-security"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-provider-type">
                {t("settings.universal.providerType", {
                  defaultValue: "Provider 类型",
                })}
              </Label>
              <Input
                id="cc-switch-universal-provider-type"
                value={draft.providerType}
                onChange={(event) =>
                  updateDraft({ providerType: event.target.value })
                }
                placeholder="openai-compatible"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-website">
                {t("settings.universal.website", { defaultValue: "网站" })}
              </Label>
              <Input
                id="cc-switch-universal-website"
                value={draft.websiteUrl ?? ""}
                onChange={(event) =>
                  updateDraft({ websiteUrl: event.target.value })
                }
                placeholder="https://example.com"
              />
            </div>
          </div>

          <div className="grid gap-2 sm:grid-cols-3">
            {UNIVERSAL_APPS.map((app) => (
              <label
                key={app}
                className="flex items-center gap-2 rounded-md border p-2 text-sm"
              >
                <Checkbox
                  checked={draft.apps[app]}
                  onCheckedChange={(checked) =>
                    updateApp(app, checked === true)
                  }
                />
                <span>{t(`apps.${app}`, { defaultValue: app })}</span>
              </label>
            ))}
          </div>

          <div className="grid gap-3 sm:grid-cols-3">
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-claude-model">
                Claude default model
              </Label>
              <Input
                id="cc-switch-universal-claude-model"
                value={draft.models.claude?.model ?? ""}
                onChange={(event) =>
                  updateModel("claude", "model", event.target.value)
                }
                placeholder="claude-sonnet-4-20250514"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-claude-haiku-model">
                Claude Haiku
              </Label>
              <Input
                id="cc-switch-universal-claude-haiku-model"
                value={draft.models.claude?.haikuModel ?? ""}
                onChange={(event) =>
                  updateModel("claude", "haikuModel", event.target.value)
                }
                placeholder="claude-haiku-4-5-20251001"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-claude-sonnet-model">
                Claude Sonnet
              </Label>
              <Input
                id="cc-switch-universal-claude-sonnet-model"
                value={draft.models.claude?.sonnetModel ?? ""}
                onChange={(event) =>
                  updateModel("claude", "sonnetModel", event.target.value)
                }
                placeholder="claude-sonnet-4-20250514"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-claude-opus-model">
                Claude Opus
              </Label>
              <Input
                id="cc-switch-universal-claude-opus-model"
                value={draft.models.claude?.opusModel ?? ""}
                onChange={(event) =>
                  updateModel("claude", "opusModel", event.target.value)
                }
                placeholder="claude-opus-4-20250514"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-codex-model">
                Codex model
              </Label>
              <Input
                id="cc-switch-universal-codex-model"
                value={draft.models.codex?.model ?? ""}
                onChange={(event) =>
                  updateModel("codex", "model", event.target.value)
                }
                placeholder="gpt-4o"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-codex-effort">
                Codex effort
              </Label>
              <Input
                id="cc-switch-universal-codex-effort"
                value={draft.models.codex?.reasoningEffort ?? ""}
                onChange={(event) =>
                  updateModel("codex", "reasoningEffort", event.target.value)
                }
                placeholder="high"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="cc-switch-universal-gemini-model">
                Gemini model
              </Label>
              <Input
                id="cc-switch-universal-gemini-model"
                value={draft.models.gemini?.model ?? ""}
                onChange={(event) =>
                  updateModel("gemini", "model", event.target.value)
                }
                placeholder="gemini-2.5-pro"
              />
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              onClick={() => void handleSave(false)}
              disabled={isBusy}
              className="gap-2"
            >
              {busyAction === "save" ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Save className="h-4 w-4" />
              )}
              {t("common.save", { defaultValue: "保存" })}
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => void handleSave(true)}
              disabled={isBusy}
              className="gap-2"
            >
              {busyAction === "save-sync" ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <SaveAll className="h-4 w-4" />
              )}
              {t("settings.universal.saveAndSync", {
                defaultValue: "保存并同步",
              })}
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => setConfirmSync(true)}
              disabled={isBusy || !selectedId}
              className="gap-2"
            >
              {busyAction === "sync" ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <RotateCcw className="h-4 w-4" />
              )}
              {t("settings.universal.sync", {
                defaultValue: "同步到应用",
              })}
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => void handleDuplicate()}
              disabled={isBusy || !selectedId}
              className="gap-2"
              title={t("settings.universal.copy", {
                defaultValue: "复制 Provider",
              })}
            >
              {busyAction === "copy" ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Copy className="h-4 w-4" />
              )}
              {t("settings.universal.copy", { defaultValue: "复制" })}
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => void handlePreview()}
              disabled={isBusy}
              className="gap-2"
            >
              {busyAction === "preview" ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Eye className="h-4 w-4" />
              )}
              {t("settings.universal.preview", { defaultValue: "配置预览" })}
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => void handleDelete()}
              disabled={isBusy || !selectedId}
              className="gap-2 text-destructive hover:text-destructive"
            >
              {busyAction === "delete" ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Trash2 className="h-4 w-4" />
              )}
              {t("common.delete", { defaultValue: "删除" })}
            </Button>
          </div>

          {preview ? (
            <pre className="max-h-72 overflow-auto rounded-md border bg-muted/40 p-3 text-xs">
              {JSON.stringify(preview, null, 2)}
            </pre>
          ) : null}
        </div>
      </div>
      <ConfirmDialog
        isOpen={confirmSync}
        title={t("settings.universal.syncConfirmTitle", {
          defaultValue: "同步 Universal Provider",
        })}
        message={t("settings.universal.syncConfirmMessage", {
          defaultValue:
            "将按当前保存配置更新 Claude、Codex 和 Gemini 中对应的 Provider。",
        })}
        confirmText={t("settings.universal.sync", {
          defaultValue: "同步到应用",
        })}
        onConfirm={() => {
          setConfirmSync(false);
          void handleSync();
        }}
        onCancel={() => setConfirmSync(false)}
      />
    </section>
  );
}
