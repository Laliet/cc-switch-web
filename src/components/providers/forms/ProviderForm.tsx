import { useEffect, useMemo, useState, useCallback } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Form, FormField, FormItem, FormMessage } from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { providerSchema, type ProviderFormData } from "@/lib/schemas/provider";
import type { AppId } from "@/lib/api";
import type {
  ClaudeDesktopMode,
  ClaudeDesktopModelRoute,
  ProviderCategory,
  ProviderMeta,
} from "@/types";
import {
  providerPresets,
  type ProviderPreset,
} from "@/config/claudeProviderPresets";
import {
  codexProviderPresets,
  type CodexProviderPreset,
} from "@/config/codexProviderPresets";
import {
  geminiProviderPresets,
  type GeminiProviderPreset,
} from "@/config/geminiProviderPresets";
import {
  opencodeProviderPresets,
  type OpenCodeProviderPreset,
} from "@/config/opencodeProviderPresets";
import {
  CLAUDE_DESKTOP_ROLE_ROUTE_IDS,
  claudeDesktopProviderPresets,
  type ClaudeDesktopApiFormat,
  type ClaudeDesktopProviderPreset,
} from "@/config/claudeDesktopProviderPresets";
import { applyTemplateValues } from "@/utils/providerConfigUtils";
import { mergeProviderMeta } from "@/utils/providerMetaUtils";
import { getCodexCustomTemplate } from "@/config/codexTemplates";
import CodexConfigEditor from "./CodexConfigEditor";
import { CommonConfigEditor } from "./CommonConfigEditor";
import GeminiConfigEditor from "./GeminiConfigEditor";
import { ProviderPresetSelector } from "./ProviderPresetSelector";
import { BasicFormFields } from "./BasicFormFields";
import { ClaudeFormFields } from "./ClaudeFormFields";
import { CodexFormFields } from "./CodexFormFields";
import { GeminiFormFields } from "./GeminiFormFields";
import { OpenCodeFormFields } from "./OpenCodeFormFields";
import { OmoFormFields } from "./OmoFormFields";
import {
  OPENCODE_DEFAULT_CONFIG,
  parseOpencodeConfig,
} from "./helpers/opencodeFormUtils";
import { parseOmoOtherFieldsObject } from "@/types/omo";
import {
  useProviderCategory,
  useApiKeyState,
  useBaseUrlState,
  useModelState,
  useCodexConfigState,
  useApiKeyLink,
  useTemplateValues,
  useCommonConfigSnippet,
  useCodexCommonConfig,
  useSpeedTestEndpoints,
  useCodexTomlValidation,
  useGeminiConfigState,
  useGeminiCommonConfig,
} from "./hooks";
import { useOmoDraftState } from "./hooks/useOmoDraftState";
import { useOmoModelSource } from "./hooks/useOmoModelSource";
import { useOpencodeConfigState } from "./hooks/useOpencodeConfigState";

const CLAUDE_DEFAULT_CONFIG = JSON.stringify({ env: {} }, null, 2);
const CLAUDE_DESKTOP_DEFAULT_CONFIG = JSON.stringify(
  {
    env: {
      ANTHROPIC_BASE_URL: "",
      ANTHROPIC_AUTH_TOKEN: "",
    },
  },
  null,
  2,
);
const CODEX_DEFAULT_CONFIG = JSON.stringify({ auth: {}, config: "" }, null, 2);
const GEMINI_DEFAULT_CONFIG = JSON.stringify(
  {
    env: {
      GOOGLE_GEMINI_BASE_URL: "",
      GEMINI_API_KEY: "",
      GEMINI_MODEL: "gemini-3-pro-preview",
    },
  },
  null,
  2,
);
const OMO_DEFAULT_CONFIG = JSON.stringify(
  {
    agents: {},
    categories: {},
  },
  null,
  2,
);

type PresetEntry = {
  id: string;
  preset:
    | ProviderPreset
    | ClaudeDesktopProviderPreset
    | CodexProviderPreset
    | GeminiProviderPreset
    | OpenCodeProviderPreset;
};

type ClaudeDesktopRouteRole = keyof typeof CLAUDE_DESKTOP_ROLE_ROUTE_IDS;

type ClaudeDesktopRouteRow = {
  role: ClaudeDesktopRouteRole;
  routeId: string;
  model: string;
  labelOverride: string;
  supports1m: boolean;
};

function routeRowsFromMeta(meta?: ProviderMeta): ClaudeDesktopRouteRow[] {
  const routes = meta?.claudeDesktopModelRoutes ?? {};
  return (["sonnet", "opus", "haiku"] as ClaudeDesktopRouteRole[]).map(
    (role) => {
      const routeId = CLAUDE_DESKTOP_ROLE_ROUTE_IDS[role];
      const route = routes[routeId];
      return {
        role,
        routeId,
        model: route?.model ?? "",
        labelOverride: route?.labelOverride ?? "",
        supports1m: route?.supports1m ?? false,
      };
    },
  );
}

function routeMapFromRows(
  rows: ClaudeDesktopRouteRow[],
): Record<string, ClaudeDesktopModelRoute> {
  return rows.reduce<Record<string, ClaudeDesktopModelRoute>>((acc, row) => {
    const model = row.model.trim();
    if (!model) return acc;
    acc[row.routeId] = {
      model,
      ...(row.labelOverride.trim()
        ? { labelOverride: row.labelOverride.trim() }
        : {}),
      ...(row.supports1m ? { supports1m: true } : {}),
    };
    return acc;
  }, {});
}

function buildClaudeDesktopConfig(
  baseUrl: string,
  apiKey: string,
  apiKeyField: string,
) {
  return JSON.stringify(
    {
      env: {
        ANTHROPIC_BASE_URL: baseUrl.trim().replace(/\/+$/, ""),
        [apiKeyField]: apiKey.trim(),
      },
    },
    null,
    2,
  );
}

function apiKeyFromConfig(config: string, apiKeyField: string) {
  try {
    const parsed = JSON.parse(config || "{}");
    const env = parsed?.env;
    if (!env || typeof env !== "object") return "";
    const value =
      (env as Record<string, unknown>)[apiKeyField] ??
      (env as Record<string, unknown>).ANTHROPIC_AUTH_TOKEN ??
      (env as Record<string, unknown>).ANTHROPIC_API_KEY;
    return typeof value === "string" ? value : "";
  } catch {
    return "";
  }
}

function baseUrlFromConfig(config: string) {
  try {
    const parsed = JSON.parse(config || "{}");
    const value = parsed?.env?.ANTHROPIC_BASE_URL;
    return typeof value === "string" ? value : "";
  } catch {
    return "";
  }
}

interface ProviderFormProps {
  appId: AppId;
  providerId?: string;
  submitLabel: string;
  onSubmit: (values: ProviderFormValues) => void;
  onCancel: () => void;
  initialData?: {
    name?: string;
    websiteUrl?: string;
    notes?: string;
    settingsConfig?: Record<string, unknown>;
    category?: ProviderCategory;
    meta?: ProviderMeta;
  };
  showButtons?: boolean;
}

export function ProviderForm({
  appId,
  providerId,
  submitLabel,
  onSubmit,
  onCancel,
  initialData,
  showButtons = true,
}: ProviderFormProps) {
  const { t } = useTranslation();
  const isEditMode = Boolean(initialData);
  const isOmoApp = appId === "omo" || appId === "omo-slim";
  const supportsPresets =
    appId === "claude" ||
    appId === "claude-desktop" ||
    appId === "codex" ||
    appId === "gemini" ||
    appId === "opencode";
  const [claudeDesktopMode, setClaudeDesktopMode] =
    useState<ClaudeDesktopMode>(
      initialData?.meta?.claudeDesktopMode ?? "direct",
    );
  const [claudeDesktopApiFormat, setClaudeDesktopApiFormat] =
    useState<ClaudeDesktopApiFormat>(
      (initialData?.meta?.apiFormat as ClaudeDesktopApiFormat | undefined) ??
        "anthropic",
    );
  const [claudeDesktopApiKeyField, setClaudeDesktopApiKeyField] = useState(
    initialData?.meta?.apiKeyField ?? "ANTHROPIC_AUTH_TOKEN",
  );
  const [claudeDesktopBaseUrl, setClaudeDesktopBaseUrl] = useState("");
  const [claudeDesktopApiKey, setClaudeDesktopApiKey] = useState("");
  const [claudeDesktopRoutes, setClaudeDesktopRoutes] = useState<
    ClaudeDesktopRouteRow[]
  >(() => routeRowsFromMeta(initialData?.meta));

  const [selectedPresetId, setSelectedPresetId] = useState<string | null>(
    initialData ? null : "custom",
  );
  const [activePreset, setActivePreset] = useState<{
    id: string;
    category?: ProviderCategory;
    isPartner?: boolean;
    partnerPromotionKey?: string;
  } | null>(null);
  const [isEndpointModalOpen, setIsEndpointModalOpen] = useState(false);
  const [isCodexEndpointModalOpen, setIsCodexEndpointModalOpen] =
    useState(false);

  // 新建供应商：收集端点测速弹窗中的"自定义端点"，提交时一次性落盘到 meta.custom_endpoints
  // 编辑供应商：端点已通过 API 直接保存，不再需要此状态
  const [draftCustomEndpoints, setDraftCustomEndpoints] = useState<string[]>(
    () => {
      // 仅在新建模式下使用
      if (initialData) return [];
      return [];
    },
  );

  // 使用 category hook
  const { category } = useProviderCategory({
    appId,
    selectedPresetId,
    isEditMode,
    initialCategory: initialData?.category,
  });

  useEffect(() => {
    setSelectedPresetId(initialData ? null : "custom");
    setActivePreset(null);
    if (appId === "claude-desktop") {
      const nextMode = initialData?.meta?.claudeDesktopMode ?? "direct";
      const nextFormat =
        (initialData?.meta?.apiFormat as ClaudeDesktopApiFormat | undefined) ??
        "anthropic";
      const nextField =
        initialData?.meta?.apiKeyField ?? "ANTHROPIC_AUTH_TOKEN";
      const nextConfig = initialData?.settingsConfig
        ? JSON.stringify(initialData.settingsConfig)
        : CLAUDE_DESKTOP_DEFAULT_CONFIG;
      setClaudeDesktopMode(nextMode);
      setClaudeDesktopApiFormat(nextFormat);
      setClaudeDesktopApiKeyField(nextField);
      setClaudeDesktopBaseUrl(baseUrlFromConfig(nextConfig));
      setClaudeDesktopApiKey(apiKeyFromConfig(nextConfig, nextField));
      setClaudeDesktopRoutes(routeRowsFromMeta(initialData?.meta));
    }

    // 编辑模式不需要恢复 draftCustomEndpoints，端点已通过 API 管理
    if (!initialData) {
      setDraftCustomEndpoints([]);
    }
  }, [appId, initialData]);

  const defaultValues: ProviderFormData = useMemo(
    () => ({
      name: initialData?.name ?? "",
      websiteUrl: initialData?.websiteUrl ?? "",
      notes: initialData?.notes ?? "",
      settingsConfig: initialData?.settingsConfig
        ? JSON.stringify(initialData.settingsConfig, null, 2)
        : appId === "codex"
          ? CODEX_DEFAULT_CONFIG
          : appId === "claude-desktop"
            ? CLAUDE_DESKTOP_DEFAULT_CONFIG
            : appId === "gemini"
              ? GEMINI_DEFAULT_CONFIG
              : appId === "opencode"
                ? OPENCODE_DEFAULT_CONFIG
                : appId === "omo" || appId === "omo-slim"
                  ? OMO_DEFAULT_CONFIG
                  : CLAUDE_DEFAULT_CONFIG,
    }),
    [initialData, appId],
  );

  const form = useForm<ProviderFormData>({
    resolver: zodResolver(providerSchema),
    defaultValues,
    mode: "onSubmit",
  });
  const settingsConfigValue = form.watch("settingsConfig");

  // 使用 API Key hook
  const {
    apiKey,
    handleApiKeyChange,
    showApiKey: shouldShowApiKey,
  } = useApiKeyState({
    initialConfig: settingsConfigValue,
    onConfigChange: (config) => form.setValue("settingsConfig", config),
    selectedPresetId,
    category,
    appType: appId,
  });
  const shouldShowApiKeyField = useMemo(
    () => shouldShowApiKey(settingsConfigValue, isEditMode),
    [shouldShowApiKey, settingsConfigValue, isEditMode],
  );

  // 使用 Base URL hook (Claude, Codex, Gemini)
  const { baseUrl, handleClaudeBaseUrlChange } = useBaseUrlState({
    appType: appId,
    category,
    settingsConfig: settingsConfigValue,
    codexConfig: "",
    onSettingsConfigChange: (config) => form.setValue("settingsConfig", config),
    onCodexConfigChange: () => {
      /* noop */
    },
  });

  // 使用 Model hook（新：主模型 + Haiku/Sonnet/Opus 默认模型）
  const {
    claudeModel,
    defaultHaikuModel,
    defaultSonnetModel,
    defaultOpusModel,
    handleModelChange,
  } = useModelState({
    settingsConfig: settingsConfigValue,
    onConfigChange: (config) => form.setValue("settingsConfig", config),
  });

  // 使用 Codex 配置 hook (仅 Codex 模式)
  const {
    codexAuth,
    codexConfig,
    codexApiKey,
    codexBaseUrl,
    codexModelName,
    codexAuthError,
    setCodexAuth,
    handleCodexApiKeyChange,
    handleCodexBaseUrlChange,
    handleCodexModelNameChange,
    handleCodexConfigChange: originalHandleCodexConfigChange,
    resetCodexConfig,
  } = useCodexConfigState({ initialData });

  // 使用 Codex TOML 校验 hook (仅 Codex 模式)
  const { configError: codexConfigError, debouncedValidate } =
    useCodexTomlValidation();

  // 包装 handleCodexConfigChange，添加实时校验
  const handleCodexConfigChange = useCallback(
    (value: string) => {
      originalHandleCodexConfigChange(value);
      debouncedValidate(value);
    },
    [originalHandleCodexConfigChange, debouncedValidate],
  );

  // Codex 新建模式：初始化时自动填充模板
  useEffect(() => {
    if (appId === "codex" && !initialData && selectedPresetId === "custom") {
      const template = getCodexCustomTemplate();
      resetCodexConfig(template.auth, template.config);
    }
  }, [appId, initialData, selectedPresetId, resetCodexConfig]);

  useEffect(() => {
    form.reset(defaultValues);
  }, [defaultValues, form]);

  const presetCategoryLabels: Record<string, string> = useMemo(
    () => ({
      official: t("providerForm.categoryOfficial", {
        defaultValue: "官方",
      }),
      cn_official: t("providerForm.categoryCnOfficial", {
        defaultValue: "国内官方",
      }),
      aggregator: t("providerForm.categoryAggregation", {
        defaultValue: "聚合服务",
      }),
      third_party: t("providerForm.categoryThirdParty", {
        defaultValue: "第三方",
      }),
      cloud_provider: t("providerForm.categoryCloudProvider", {
        defaultValue: "云服务",
      }),
      omo: "OMO",
      "omo-slim": "OMO Slim",
    }),
    [t],
  );

  const presetEntries = useMemo(() => {
    if (appId === "codex") {
      return codexProviderPresets.map<PresetEntry>((preset, index) => ({
        id: `codex-${index}`,
        preset,
      }));
    }
    if (appId === "gemini") {
      return geminiProviderPresets.map<PresetEntry>((preset, index) => ({
        id: `gemini-${index}`,
        preset,
      }));
    }
    if (appId === "claude") {
      return providerPresets.map<PresetEntry>((preset, index) => ({
        id: `claude-${index}`,
        preset,
      }));
    }
    if (appId === "claude-desktop") {
      return claudeDesktopProviderPresets.map<PresetEntry>(
        (preset, index) => ({
          id: `claude-desktop-${index}`,
          preset,
        }),
      );
    }
    if (appId === "opencode") {
      return opencodeProviderPresets.map<PresetEntry>((preset, index) => ({
        id: `opencode-${index}`,
        preset,
      }));
    }
    return [];
  }, [appId]);

  const templatePresetEntries = useMemo<
    Array<{ id: string; preset: ProviderPreset | CodexProviderPreset }>
  >(() => {
    if (appId !== "claude") {
      return [];
    }
    return providerPresets.map((preset, index) => ({
      id: `claude-${index}`,
      preset,
    }));
  }, [appId]);

  // 使用模板变量 hook (仅 Claude 模式)
  const {
    templateValues,
    templateValueEntries,
    selectedPreset: templatePreset,
    handleTemplateValueChange,
    validateTemplateValues,
  } = useTemplateValues({
    selectedPresetId: appId === "claude" ? selectedPresetId : null,
    presetEntries: templatePresetEntries,
    settingsConfig: settingsConfigValue,
    onConfigChange: (config) => form.setValue("settingsConfig", config),
  });

  // 使用通用配置片段 hook (仅 Claude 模式)
  const {
    useCommonConfig,
    commonConfigSnippet,
    commonConfigError,
    handleCommonConfigToggle,
    handleCommonConfigSnippetChange,
  } = useCommonConfigSnippet({
    enabled: appId === "claude",
    settingsConfig: settingsConfigValue,
    onConfigChange: (config) => form.setValue("settingsConfig", config),
    initialData: appId === "claude" ? initialData : undefined,
  });

  // 使用 Codex 通用配置片段 hook (仅 Codex 模式)
  const {
    useCommonConfig: useCodexCommonConfigFlag,
    commonConfigSnippet: codexCommonConfigSnippet,
    commonConfigError: codexCommonConfigError,
    handleCommonConfigToggle: handleCodexCommonConfigToggle,
    handleCommonConfigSnippetChange: handleCodexCommonConfigSnippetChange,
  } = useCodexCommonConfig({
    codexConfig,
    onConfigChange: handleCodexConfigChange,
    initialData: appId === "codex" ? initialData : undefined,
  });

  // 使用 Gemini 配置 hook (仅 Gemini 模式)
  const {
    geminiEnv,
    geminiConfig,
    geminiApiKey,
    geminiBaseUrl,
    geminiModel,
    envError,
    configError: geminiConfigError,
    handleGeminiApiKeyChange: originalHandleGeminiApiKeyChange,
    handleGeminiBaseUrlChange: originalHandleGeminiBaseUrlChange,
    handleGeminiEnvChange,
    handleGeminiConfigChange,
    resetGeminiConfig,
    envStringToObj,
    envObjToString,
  } = useGeminiConfigState({
    initialData: appId === "gemini" ? initialData : undefined,
  });

  // 包装 Gemini handlers 以同步 settingsConfig
  const handleGeminiApiKeyChange = useCallback(
    (key: string) => {
      originalHandleGeminiApiKeyChange(key);
      // 同步更新 settingsConfig
      try {
        const config = JSON.parse(form.watch("settingsConfig") || "{}");
        if (!config.env) config.env = {};
        config.env.GEMINI_API_KEY = key.trim();
        form.setValue("settingsConfig", JSON.stringify(config, null, 2));
      } catch {
        // ignore
      }
    },
    [originalHandleGeminiApiKeyChange, form],
  );

  const handleGeminiBaseUrlChange = useCallback(
    (url: string) => {
      originalHandleGeminiBaseUrlChange(url);
      // 同步更新 settingsConfig
      try {
        const config = JSON.parse(form.watch("settingsConfig") || "{}");
        if (!config.env) config.env = {};
        config.env.GOOGLE_GEMINI_BASE_URL = url.trim().replace(/\/+$/, "");
        form.setValue("settingsConfig", JSON.stringify(config, null, 2));
      } catch {
        // ignore
      }
    },
    [originalHandleGeminiBaseUrlChange, form],
  );

  // 使用 Gemini 通用配置 hook (仅 Gemini 模式)
  const {
    useCommonConfig: useGeminiCommonConfigFlag,
    commonConfigSnippet: geminiCommonConfigSnippet,
    commonConfigError: geminiCommonConfigError,
    handleCommonConfigToggle: handleGeminiCommonConfigToggle,
    handleCommonConfigSnippetChange: handleGeminiCommonConfigSnippetChange,
  } = useGeminiCommonConfig({
    configValue: geminiConfig,
    onConfigChange: handleGeminiConfigChange,
    initialData: appId === "gemini" ? initialData : undefined,
  });

  const [isCommonConfigModalOpen, setIsCommonConfigModalOpen] = useState(false);

  const opencodeState = useOpencodeConfigState({
    initialData: appId === "opencode" ? initialData : undefined,
    onSettingsConfigChange: (value) => form.setValue("settingsConfig", value),
    getSettingsConfig: () => form.watch("settingsConfig"),
  });

  const { omoModelOptions, omoModelVariantsMap, omoPresetMetaMap } =
    useOmoModelSource({
      isOmoCategory: isOmoApp,
      providerId,
    });

  const omoDraft = useOmoDraftState({
    initialOmoSettings: isOmoApp ? initialData?.settingsConfig : undefined,
    isEditMode,
    appId,
    category: appId,
  });

  const handleSubmit = (values: ProviderFormData) => {
    // 验证模板变量（仅 Claude 模式）
    if (appId === "claude" && templateValueEntries.length > 0) {
      const validation = validateTemplateValues();
      if (!validation.isValid && validation.missingField) {
        form.setError("settingsConfig", {
          type: "manual",
          message: t("providerForm.fillParameter", {
            label: validation.missingField.label,
            defaultValue: `请填写 ${validation.missingField.label}`,
          }),
        });
        return;
      }
    }

    let settingsConfig: string;

    // Codex: 组合 auth 和 config
    if (appId === "codex") {
      try {
        const authJson = JSON.parse(codexAuth);
        const configObj = {
          auth: authJson,
          config: codexConfig ?? "",
        };
        settingsConfig = JSON.stringify(configObj);
      } catch (err) {
        // 如果解析失败，使用表单中的配置
        settingsConfig = values.settingsConfig.trim();
      }
    } else if (appId === "claude-desktop") {
      settingsConfig = buildClaudeDesktopConfig(
        claudeDesktopBaseUrl,
        claudeDesktopApiKey,
        claudeDesktopApiKeyField,
      );
    } else if (appId === "gemini") {
      // Gemini: 组合 env 和 config
      try {
        const envObj = envStringToObj(geminiEnv);
        const configObj = geminiConfig.trim() ? JSON.parse(geminiConfig) : {};
        const combined = {
          env: envObj,
          config: configObj,
        };
        settingsConfig = JSON.stringify(combined);
      } catch (err) {
        // 如果解析失败，使用表单中的配置
        settingsConfig = values.settingsConfig.trim();
      }
    } else if (appId === "opencode") {
      settingsConfig = values.settingsConfig.trim();
    } else if (isOmoApp) {
      if (omoDraft.omoOtherFieldsStr.trim()) {
        try {
          const otherFields = parseOmoOtherFieldsObject(
            omoDraft.omoOtherFieldsStr,
          );
          if (!otherFields) {
            form.setError("settingsConfig", {
              type: "manual",
              message: t("omo.jsonMustBeObject", {
                field: t("omo.otherFields", {
                  defaultValue: "Other Config",
                }),
                defaultValue: "{{field}} must be a JSON object",
              }),
            });
            return;
          }
        } catch {
          form.setError("settingsConfig", {
            type: "manual",
            message: t("omo.invalidJson", {
              defaultValue: "Other Fields contains invalid JSON",
            }),
          });
          return;
        }
      }

      const omoConfig: Record<string, unknown> = {};
      if (Object.keys(omoDraft.omoAgents).length > 0) {
        omoConfig.agents = omoDraft.omoAgents;
      }
      if (appId === "omo" && Object.keys(omoDraft.omoCategories).length > 0) {
        omoConfig.categories = omoDraft.omoCategories;
      }
      if (omoDraft.omoOtherFieldsStr.trim()) {
        const otherFields = parseOmoOtherFieldsObject(
          omoDraft.omoOtherFieldsStr,
        );
        if (otherFields) {
          Object.assign(omoConfig, otherFields);
        }
      }
      settingsConfig = JSON.stringify(omoConfig);
    } else {
      // Claude: 使用表单配置
      settingsConfig = values.settingsConfig.trim();
    }

    const payload: ProviderFormValues = {
      ...values,
      name: values.name.trim(),
      websiteUrl: values.websiteUrl?.trim() ?? "",
      settingsConfig,
    };

    if (activePreset) {
      payload.presetId = activePreset.id;
      if (activePreset.category) {
        payload.presetCategory = activePreset.category;
      }
      // 继承合作伙伴标识
      if (activePreset.isPartner) {
        payload.isPartner = activePreset.isPartner;
      }
    }
    if (isOmoApp && !payload.presetCategory) {
      payload.presetCategory = appId;
    }
    if (appId === "claude-desktop") {
      payload.meta = {
        ...(initialData?.meta ?? {}),
        ...(payload.meta ?? {}),
        claudeDesktopMode,
        claudeDesktopModelRoutes: routeMapFromRows(claudeDesktopRoutes),
        apiFormat: claudeDesktopApiFormat,
        apiKeyField: claudeDesktopApiKeyField,
        ...(activePreset?.isPartner ? { isPartner: true } : {}),
        ...(activePreset?.partnerPromotionKey
          ? { partnerPromotionKey: activePreset.partnerPromotionKey }
          : {}),
      };
    }

    // 处理 meta 字段：仅在新建模式下从 draftCustomEndpoints 生成 custom_endpoints
    // 编辑模式：端点已通过 API 直接保存，不在此处理
    if (!isEditMode && draftCustomEndpoints.length > 0) {
      const customEndpointsToSave: Record<
        string,
        import("@/types").CustomEndpoint
      > = draftCustomEndpoints.reduce(
        (acc, url) => {
          const now = Date.now();
          acc[url] = { url, addedAt: now, lastUsed: undefined };
          return acc;
        },
        {} as Record<string, import("@/types").CustomEndpoint>,
      );

      // 检测是否需要清空端点（重要：区分"用户清空端点"和"用户没有修改端点"）
      const hadEndpoints =
        initialData?.meta?.custom_endpoints &&
        Object.keys(initialData.meta.custom_endpoints).length > 0;
      const needsClearEndpoints =
        hadEndpoints && draftCustomEndpoints.length === 0;

      // 如果用户明确清空了端点，传递空对象（而不是 null）让后端知道要删除
      let mergedMeta = needsClearEndpoints
        ? mergeProviderMeta(initialData?.meta, {})
        : mergeProviderMeta(initialData?.meta, customEndpointsToSave);

      // 添加合作伙伴标识与促销 key
      if (activePreset?.isPartner) {
        mergedMeta = {
          ...(mergedMeta ?? {}),
          isPartner: true,
        };
      }

      if (activePreset?.partnerPromotionKey) {
        mergedMeta = {
          ...(mergedMeta ?? {}),
          partnerPromotionKey: activePreset.partnerPromotionKey,
        };
      }

      if (mergedMeta !== undefined) {
        payload.meta = mergedMeta;
      }
    }

    onSubmit(payload);
  };

  const groupedPresets = useMemo(() => {
    return presetEntries.reduce<Record<string, PresetEntry[]>>((acc, entry) => {
      const category = entry.preset.category ?? "others";
      if (!acc[category]) {
        acc[category] = [];
      }
      acc[category].push(entry);
      return acc;
    }, {});
  }, [presetEntries]);

  const categoryKeys = useMemo(() => {
    return Object.keys(groupedPresets).filter(
      (key) => key !== "custom" && groupedPresets[key]?.length,
    );
  }, [groupedPresets]);

  // 判断是否显示端点测速（仅官方类别不显示）
  const shouldShowSpeedTest = category !== "official";

  // 使用 API Key 链接 hook (Claude)
  const {
    shouldShowApiKeyLink: shouldShowClaudeApiKeyLink,
    websiteUrl: claudeWebsiteUrl,
    isPartner: isClaudePartner,
    partnerPromotionKey: claudePartnerPromotionKey,
  } = useApiKeyLink({
    appId: "claude",
    category,
    selectedPresetId,
    presetEntries,
    formWebsiteUrl: form.watch("websiteUrl") || "",
  });

  // 使用 API Key 链接 hook (Codex)
  const {
    shouldShowApiKeyLink: shouldShowCodexApiKeyLink,
    websiteUrl: codexWebsiteUrl,
    isPartner: isCodexPartner,
    partnerPromotionKey: codexPartnerPromotionKey,
  } = useApiKeyLink({
    appId: "codex",
    category,
    selectedPresetId,
    presetEntries,
    formWebsiteUrl: form.watch("websiteUrl") || "",
  });

  // 使用 API Key 链接 hook (Gemini)
  const {
    shouldShowApiKeyLink: shouldShowGeminiApiKeyLink,
    websiteUrl: geminiWebsiteUrl,
    isPartner: isGeminiPartner,
    partnerPromotionKey: geminiPartnerPromotionKey,
  } = useApiKeyLink({
    appId: "gemini",
    category,
    selectedPresetId,
    presetEntries,
    formWebsiteUrl: form.watch("websiteUrl") || "",
  });

  const {
    shouldShowApiKeyLink: shouldShowOpencodeApiKeyLink,
    websiteUrl: opencodeWebsiteUrl,
    isPartner: isOpencodePartner,
    partnerPromotionKey: opencodePartnerPromotionKey,
  } = useApiKeyLink({
    appId: "opencode",
    category,
    selectedPresetId,
    presetEntries,
    formWebsiteUrl: form.watch("websiteUrl") || "",
  });

  // 使用端点测速候选 hook
  const speedTestEndpoints = useSpeedTestEndpoints({
    appId,
    selectedPresetId,
    presetEntries,
    baseUrl,
    codexBaseUrl,
    initialData,
  });

  const handlePresetChange = (value: string) => {
    setSelectedPresetId(value);
    if (value === "custom") {
      setActivePreset(null);
      form.reset(defaultValues);

      // Codex 自定义模式：加载模板
      if (appId === "codex") {
        const template = getCodexCustomTemplate();
        resetCodexConfig(template.auth, template.config);
      }
      // Gemini 自定义模式：重置为空配置
      if (appId === "gemini") {
        resetGeminiConfig({}, {});
      }
      if (appId === "opencode") {
        opencodeState.reset();
      }
      return;
    }

    const entry = presetEntries.find((item) => item.id === value);
    if (!entry) {
      return;
    }

    setActivePreset({
      id: value,
      category: entry.preset.category,
      isPartner: entry.preset.isPartner,
      partnerPromotionKey: entry.preset.partnerPromotionKey,
    });

    if (appId === "codex") {
      const preset = entry.preset as CodexProviderPreset;
      const auth = preset.auth ?? {};
      const config = preset.config ?? "";

      // 重置 Codex 配置
      resetCodexConfig(auth, config);

      // 更新表单其他字段
      form.reset({
        name: preset.name,
        websiteUrl: preset.websiteUrl ?? "",
        settingsConfig: JSON.stringify({ auth, config }, null, 2),
      });
      return;
    }

    if (appId === "gemini") {
      const preset = entry.preset as GeminiProviderPreset;
      const env = (preset.settingsConfig as any)?.env ?? {};
      const config = (preset.settingsConfig as any)?.config ?? {};

      // 重置 Gemini 配置
      resetGeminiConfig(env, config);

      // 更新表单其他字段
      form.reset({
        name: preset.name,
        websiteUrl: preset.websiteUrl ?? "",
        settingsConfig: JSON.stringify(preset.settingsConfig, null, 2),
      });
      return;
    }

    if (appId === "opencode") {
      const preset = entry.preset as OpenCodeProviderPreset;
      if (preset.category === "omo" || preset.category === "omo-slim") {
        form.reset({
          name: preset.category === "omo" ? "OMO" : "OMO Slim",
          websiteUrl: preset.websiteUrl ?? "",
          notes: "",
          settingsConfig: JSON.stringify({}, null, 2),
        });
        return;
      }

      const config = parseOpencodeConfig(preset.settingsConfig);
      opencodeState.reset(config);
      form.reset({
        name: preset.nameKey ? t(preset.nameKey) : preset.name,
        websiteUrl: preset.websiteUrl ?? "",
        notes: "",
        settingsConfig: JSON.stringify(config, null, 2),
      });
      return;
    }

    if (appId === "claude-desktop") {
      const preset = entry.preset as ClaudeDesktopProviderPreset;
      const apiKeyField = preset.apiKeyField ?? "ANTHROPIC_AUTH_TOKEN";
      const rows = routeRowsFromMeta({
        claudeDesktopModelRoutes: Object.fromEntries(
          (preset.modelRoutes ?? []).map((route) => [
            route.routeId,
            {
              model: route.upstreamModel,
              ...(route.labelOverride
                ? { labelOverride: route.labelOverride }
                : {}),
              ...(route.supports1m ? { supports1m: true } : {}),
            },
          ]),
        ),
      });

      setClaudeDesktopMode(preset.mode);
      setClaudeDesktopApiFormat(preset.apiFormat ?? "anthropic");
      setClaudeDesktopApiKeyField(apiKeyField);
      setClaudeDesktopBaseUrl(preset.baseUrl);
      setClaudeDesktopApiKey("");
      setClaudeDesktopRoutes(rows);
      form.reset({
        name: preset.name,
        websiteUrl: preset.websiteUrl ?? "",
        notes: "",
        settingsConfig: buildClaudeDesktopConfig(
          preset.baseUrl,
          "",
          apiKeyField,
        ),
      });
      return;
    }

    const preset = entry.preset as ProviderPreset;
    const config = applyTemplateValues(
      preset.settingsConfig,
      preset.templateValues,
    );

    form.reset({
      name: preset.name,
      websiteUrl: preset.websiteUrl ?? "",
      settingsConfig: JSON.stringify(config, null, 2),
    });
  };

  return (
    <Form {...form}>
      <form
        id="provider-form"
        onSubmit={form.handleSubmit(handleSubmit)}
        className="space-y-6"
      >
        {/* 预设供应商选择（仅新增模式显示） */}
        {!initialData && supportsPresets ? (
          <ProviderPresetSelector
            selectedPresetId={selectedPresetId}
            groupedPresets={groupedPresets}
            categoryKeys={categoryKeys}
            presetCategoryLabels={presetCategoryLabels}
            onPresetChange={handlePresetChange}
            category={category}
          />
        ) : null}

        {/* 基础字段 */}
        <BasicFormFields form={form} />

        {/* Claude 专属字段 */}
        {appId === "claude" && (
          <ClaudeFormFields
            providerId={providerId}
            shouldShowApiKey={shouldShowApiKeyField}
            apiKey={apiKey}
            onApiKeyChange={handleApiKeyChange}
            category={category}
            shouldShowApiKeyLink={shouldShowClaudeApiKeyLink}
            websiteUrl={claudeWebsiteUrl}
            isPartner={isClaudePartner}
            partnerPromotionKey={claudePartnerPromotionKey}
            templateValueEntries={templateValueEntries}
            templateValues={templateValues}
            templatePresetName={templatePreset?.name || ""}
            onTemplateValueChange={handleTemplateValueChange}
            shouldShowSpeedTest={shouldShowSpeedTest}
            baseUrl={baseUrl}
            onBaseUrlChange={handleClaudeBaseUrlChange}
            isEndpointModalOpen={isEndpointModalOpen}
            onEndpointModalToggle={setIsEndpointModalOpen}
            onCustomEndpointsChange={
              isEditMode ? undefined : setDraftCustomEndpoints
            }
            shouldShowModelSelector={category !== "official"}
            claudeModel={claudeModel}
            defaultHaikuModel={defaultHaikuModel}
            defaultSonnetModel={defaultSonnetModel}
            defaultOpusModel={defaultOpusModel}
            onModelChange={handleModelChange}
            speedTestEndpoints={speedTestEndpoints}
          />
        )}

        {appId === "claude-desktop" && (
          <div className="space-y-5">
            <div className="grid gap-4 md:grid-cols-3">
              <div className="space-y-2">
                <Label>{t("providerForm.claudeDesktopMode", { defaultValue: "写入模式" })}</Label>
                <Select
                  value={claudeDesktopMode}
                  onValueChange={(value) =>
                    setClaudeDesktopMode(value as ClaudeDesktopMode)
                  }
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="direct">Direct</SelectItem>
                    <SelectItem value="proxy">Local Routing</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2">
                <Label>{t("providerForm.apiFormat", { defaultValue: "API 格式" })}</Label>
                <Select
                  value={claudeDesktopApiFormat}
                  onValueChange={(value) =>
                    setClaudeDesktopApiFormat(value as ClaudeDesktopApiFormat)
                  }
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="anthropic">Anthropic</SelectItem>
                    <SelectItem value="openai_chat">OpenAI Chat</SelectItem>
                    <SelectItem value="openai_responses">
                      OpenAI Responses
                    </SelectItem>
                    <SelectItem value="gemini_native">Gemini Native</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2">
                <Label>{t("providerForm.apiKeyField", { defaultValue: "Key 字段" })}</Label>
                <Select
                  value={claudeDesktopApiKeyField}
                  onValueChange={(value) => {
                    setClaudeDesktopApiKeyField(value);
                    form.setValue(
                      "settingsConfig",
                      buildClaudeDesktopConfig(
                        claudeDesktopBaseUrl,
                        claudeDesktopApiKey,
                        value,
                      ),
                    );
                  }}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="ANTHROPIC_AUTH_TOKEN">
                      ANTHROPIC_AUTH_TOKEN
                    </SelectItem>
                    <SelectItem value="ANTHROPIC_API_KEY">
                      ANTHROPIC_API_KEY
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="claude-desktop-base-url">
                {t("providerForm.apiEndpoint", { defaultValue: "API Endpoint" })}
              </Label>
              <Input
                id="claude-desktop-base-url"
                value={claudeDesktopBaseUrl}
                onChange={(event) => {
                  const value = event.target.value;
                  setClaudeDesktopBaseUrl(value);
                  form.setValue(
                    "settingsConfig",
                    buildClaudeDesktopConfig(
                      value,
                      claudeDesktopApiKey,
                      claudeDesktopApiKeyField,
                    ),
                  );
                }}
                placeholder="https://api.example.com"
                autoComplete="off"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="claude-desktop-api-key">API Key</Label>
              <Input
                id="claude-desktop-api-key"
                value={claudeDesktopApiKey}
                onChange={(event) => {
                  const value = event.target.value;
                  setClaudeDesktopApiKey(value);
                  form.setValue(
                    "settingsConfig",
                    buildClaudeDesktopConfig(
                      claudeDesktopBaseUrl,
                      value,
                      claudeDesktopApiKeyField,
                    ),
                  );
                }}
                placeholder="sk-..."
                autoComplete="off"
              />
            </div>

            <div className="space-y-3">
              <Label>
                {t("providerForm.claudeDesktopRoutes", {
                  defaultValue: "模型角色映射",
                })}
              </Label>
              <div className="space-y-3">
                {claudeDesktopRoutes.map((row, index) => (
                  <div
                    key={row.routeId}
                    className="grid gap-3 rounded-md border border-border-default p-3 md:grid-cols-[110px_1fr_1fr_90px]"
                  >
                    <div className="text-sm font-medium capitalize leading-9">
                      {row.role}
                    </div>
                    <Input
                      value={row.model}
                      onChange={(event) => {
                        const value = event.target.value;
                        setClaudeDesktopRoutes((rows) =>
                          rows.map((item, itemIndex) =>
                            itemIndex === index
                              ? { ...item, model: value }
                              : item,
                          ),
                        );
                      }}
                      placeholder="upstream-model"
                      autoComplete="off"
                    />
                    <Input
                      value={row.labelOverride}
                      onChange={(event) => {
                        const value = event.target.value;
                        setClaudeDesktopRoutes((rows) =>
                          rows.map((item, itemIndex) =>
                            itemIndex === index
                              ? { ...item, labelOverride: value }
                              : item,
                          ),
                        );
                      }}
                      placeholder="label"
                      autoComplete="off"
                    />
                    <div className="flex items-center justify-end gap-2">
                      <span className="text-xs text-muted-foreground">1M</span>
                      <Switch
                        checked={row.supports1m}
                        onCheckedChange={(checked) => {
                          setClaudeDesktopRoutes((rows) =>
                            rows.map((item, itemIndex) =>
                              itemIndex === index
                                ? { ...item, supports1m: checked }
                                : item,
                            ),
                          );
                        }}
                      />
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}

        {/* Codex 专属字段 */}
        {appId === "codex" && (
          <CodexFormFields
            providerId={providerId}
            codexApiKey={codexApiKey}
            onApiKeyChange={handleCodexApiKeyChange}
            category={category}
            shouldShowApiKeyLink={shouldShowCodexApiKeyLink}
            websiteUrl={codexWebsiteUrl}
            isPartner={isCodexPartner}
            partnerPromotionKey={codexPartnerPromotionKey}
            shouldShowSpeedTest={shouldShowSpeedTest}
            codexBaseUrl={codexBaseUrl}
            onBaseUrlChange={handleCodexBaseUrlChange}
            isEndpointModalOpen={isCodexEndpointModalOpen}
            onEndpointModalToggle={setIsCodexEndpointModalOpen}
            onCustomEndpointsChange={
              isEditMode ? undefined : setDraftCustomEndpoints
            }
            shouldShowModelField={category !== "official"}
            modelName={codexModelName}
            onModelNameChange={handleCodexModelNameChange}
            speedTestEndpoints={speedTestEndpoints}
          />
        )}

        {/* Gemini 专属字段 */}
        {appId === "gemini" && (
          <GeminiFormFields
            providerId={providerId}
            shouldShowApiKey={shouldShowApiKeyField}
            apiKey={geminiApiKey}
            onApiKeyChange={handleGeminiApiKeyChange}
            category={category}
            shouldShowApiKeyLink={shouldShowGeminiApiKeyLink}
            websiteUrl={geminiWebsiteUrl}
            isPartner={isGeminiPartner}
            partnerPromotionKey={geminiPartnerPromotionKey}
            shouldShowSpeedTest={shouldShowSpeedTest}
            baseUrl={geminiBaseUrl}
            onBaseUrlChange={handleGeminiBaseUrlChange}
            isEndpointModalOpen={isEndpointModalOpen}
            onEndpointModalToggle={setIsEndpointModalOpen}
            onCustomEndpointsChange={setDraftCustomEndpoints}
            shouldShowModelField={true}
            model={geminiModel}
            onModelChange={(model) => {
              // 同时更新 form.settingsConfig 和 geminiEnv
              const config = JSON.parse(form.watch("settingsConfig") || "{}");
              if (!config.env) config.env = {};
              config.env.GEMINI_MODEL = model;
              form.setValue("settingsConfig", JSON.stringify(config, null, 2));

              // 同步更新 geminiEnv，确保提交时不丢失
              const envObj = envStringToObj(geminiEnv);
              envObj.GEMINI_MODEL = model.trim();
              const newEnv = envObjToString(envObj);
              handleGeminiEnvChange(newEnv);
            }}
            speedTestEndpoints={speedTestEndpoints}
          />
        )}

        {/* OpenCode 专属字段 */}
        {appId === "opencode" && (
          <OpenCodeFormFields
            npm={opencodeState.npm}
            onNpmChange={opencodeState.handleNpmChange}
            apiKey={opencodeState.apiKey}
            onApiKeyChange={opencodeState.handleApiKeyChange}
            category={category}
            shouldShowApiKeyLink={shouldShowOpencodeApiKeyLink}
            websiteUrl={opencodeWebsiteUrl}
            isPartner={isOpencodePartner}
            partnerPromotionKey={opencodePartnerPromotionKey}
            baseUrl={opencodeState.baseUrl}
            onBaseUrlChange={opencodeState.handleBaseUrlChange}
            isFullUrl={opencodeState.isFullUrl}
            onIsFullUrlChange={opencodeState.handleIsFullUrlChange}
            modelsUrl={opencodeState.modelsUrl}
            onModelsUrlChange={opencodeState.handleModelsUrlChange}
            models={opencodeState.models}
            onModelsChange={opencodeState.handleModelsChange}
            extraOptions={opencodeState.extraOptions}
            onExtraOptionsChange={opencodeState.handleExtraOptionsChange}
          />
        )}

        {isOmoApp && (
          <OmoFormFields
            modelOptions={omoModelOptions}
            modelVariantsMap={omoModelVariantsMap}
            presetMetaMap={omoPresetMetaMap}
            agents={omoDraft.omoAgents}
            onAgentsChange={omoDraft.setOmoAgents}
            categories={appId === "omo" ? omoDraft.omoCategories : undefined}
            onCategoriesChange={
              appId === "omo" ? omoDraft.setOmoCategories : undefined
            }
            otherFieldsStr={omoDraft.omoOtherFieldsStr}
            onOtherFieldsStrChange={omoDraft.setOmoOtherFieldsStr}
            isSlim={appId === "omo-slim"}
          />
        )}

        {/* 配置编辑器：Codex、Claude、Gemini 分别使用不同的编辑器 */}
        {appId === "codex" ? (
          <>
            <CodexConfigEditor
              authValue={codexAuth}
              configValue={codexConfig}
              onAuthChange={setCodexAuth}
              onConfigChange={handleCodexConfigChange}
              useCommonConfig={useCodexCommonConfigFlag}
              onCommonConfigToggle={handleCodexCommonConfigToggle}
              commonConfigSnippet={codexCommonConfigSnippet}
              onCommonConfigSnippetChange={handleCodexCommonConfigSnippetChange}
              commonConfigError={codexCommonConfigError}
              authError={codexAuthError}
              configError={codexConfigError}
            />
            {/* 配置验证错误显示 */}
            <FormField
              control={form.control}
              name="settingsConfig"
              render={() => (
                <FormItem className="space-y-0">
                  <FormMessage />
                </FormItem>
              )}
            />
          </>
        ) : appId === "gemini" ? (
          <>
            <GeminiConfigEditor
              envValue={geminiEnv}
              configValue={geminiConfig}
              onEnvChange={handleGeminiEnvChange}
              onConfigChange={handleGeminiConfigChange}
              useCommonConfig={useGeminiCommonConfigFlag}
              onCommonConfigToggle={handleGeminiCommonConfigToggle}
              commonConfigSnippet={geminiCommonConfigSnippet}
              onCommonConfigSnippetChange={
                handleGeminiCommonConfigSnippetChange
              }
              commonConfigError={geminiCommonConfigError}
              envError={envError}
              configError={geminiConfigError}
            />
            {/* 配置验证错误显示 */}
            <FormField
              control={form.control}
              name="settingsConfig"
              render={() => (
                <FormItem className="space-y-0">
                  <FormMessage />
                </FormItem>
              )}
            />
          </>
        ) : isOmoApp ? (
          <FormField
            control={form.control}
            name="settingsConfig"
            render={() => (
              <FormItem className="space-y-0">
                <FormMessage />
              </FormItem>
            )}
          />
        ) : (
          <>
            <CommonConfigEditor
              value={settingsConfigValue}
              onChange={(value) => form.setValue("settingsConfig", value)}
              useCommonConfig={useCommonConfig}
              onCommonConfigToggle={handleCommonConfigToggle}
              commonConfigSnippet={commonConfigSnippet}
              onCommonConfigSnippetChange={handleCommonConfigSnippetChange}
              commonConfigError={commonConfigError}
              onEditClick={() => setIsCommonConfigModalOpen(true)}
              isModalOpen={isCommonConfigModalOpen}
              onModalClose={() => setIsCommonConfigModalOpen(false)}
              showCommonConfigControls={appId === "claude"}
              appId={appId}
            />
            {/* 配置验证错误显示 */}
            <FormField
              control={form.control}
              name="settingsConfig"
              render={() => (
                <FormItem className="space-y-0">
                  <FormMessage />
                </FormItem>
              )}
            />
          </>
        )}

        {showButtons && (
          <div className="flex justify-end gap-2">
            <Button variant="outline" type="button" onClick={onCancel}>
              {t("common.cancel")}
            </Button>
            <Button type="submit">{submitLabel}</Button>
          </div>
        )}
      </form>
    </Form>
  );
}

export type ProviderFormValues = ProviderFormData & {
  presetId?: string;
  presetCategory?: ProviderCategory;
  isPartner?: boolean;
  meta?: ProviderMeta;
};
