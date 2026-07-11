import React, { useState } from "react";
import { Play, Wand2, Eye, EyeOff } from "lucide-react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { Provider, UsageScript } from "@/types";
import { usageApi, type AppId } from "@/lib/api";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { TEMPLATE_TYPES } from "@/config/constants";
import {
  CODING_PLAN_PROVIDERS,
  detectBalanceProvider,
  detectCodingPlanProvider,
} from "@/config/codingPlanProviders";

const JsonEditor = React.lazy(() => import("./JsonEditor"));

interface UsageScriptModalProps {
  provider: Provider;
  appId: AppId;
  isOpen: boolean;
  onClose: () => void;
  onSave: (script: UsageScript) => void;
}

// 预设模板键名（用于国际化）
const TEMPLATE_KEYS = {
  CUSTOM: "custom",
  GENERAL: "general",
  NEW_API: "newapi",
  PACKYCODE: "packycode",
  CODE88: "88code",
  PRIVNODE: "privnode",
  TOKEN_PLAN: TEMPLATE_TYPES.TOKEN_PLAN,
  BALANCE: TEMPLATE_TYPES.BALANCE,
} as const;

// 生成预设模板的函数（支持国际化）
const generatePresetTemplates = (
  t: (key: string) => string,
): Record<string, string> => ({
  [TEMPLATE_KEYS.CUSTOM]: `({
  request: {
    url: "",
    method: "GET",
    headers: {}
  },
  extractor: function(response) {
    return {
      remaining: 0,
      unit: "USD"
    };
  }
})`,

  [TEMPLATE_KEYS.GENERAL]: `({
  request: {
    url: "{{baseUrl}}/user/balance",
    method: "GET",
    headers: {
      "Authorization": "Bearer {{apiKey}}",
      "User-Agent": "cc-switch/1.0"
    }
  },
  extractor: function(response) {
    return {
      isValid: response.is_active || true,
      remaining: response.balance,
      unit: "USD"
    };
  }
})`,

  [TEMPLATE_KEYS.NEW_API]: `({
  request: {
    url: "{{baseUrl}}/api/user/self",
    method: "GET",
    headers: {
      "Content-Type": "application/json",
      "Authorization": "Bearer {{accessToken}}",
      "New-Api-User": "{{userId}}"
    },
  },
  extractor: function (response) {
    if (response.success && response.data) {
      return {
        planName: response.data.group || "${t("usageScript.defaultPlan")}",
        remaining: response.data.quota / 500000,
        used: response.data.used_quota / 500000,
        total: (response.data.quota + response.data.used_quota) / 500000,
        unit: "USD",
      };
    }
    return {
      isValid: false,
      invalidMessage: response.message || "${t("usageScript.queryFailedMessage")}"
    };
  },
})`,

  // 服务商专用模板
  [TEMPLATE_KEYS.PACKYCODE]: `({
  request: {
    url: "https://www.packyapi.com/api/user/self",
    method: "GET",
    headers: {
      "Authorization": "Bearer {{apiKey}}",
      "Content-Type": "application/json"
    }
  },
  extractor: function (response) {
    if (response.success === false) {
      return { 
        isValid: false, 
        invalidMessage: response.message || "Invalid token" 
      };
    }
    const info = response.data || response;
    const remaining = info.balance ?? info.quota ?? info.credit ?? null;
    const used = info.used_quota ?? info.used ?? info.usage ?? null;
    return {
      planName: info.group || info.role || "PackyCode",
      remaining: remaining,
      used: used,
      total: remaining && used ? remaining + used : null,
      unit: info.unit || "credits",
      percentage: remaining && used ? (used / (remaining + used)) * 100 : null
    };
  }
})`,

  [TEMPLATE_KEYS.CODE88]: `({
  request: {
    url: "{{baseUrl}}/v1/me",
    method: "GET",
    headers: {
      "Authorization": "Bearer {{apiKey}}",
      "Content-Type": "application/json"
    }
  },
  extractor: function (response) {
    if (response.error) {
      return { 
        isValid: false, 
        invalidMessage: response.error.message || "Invalid API key" 
      };
    }
    const info = response.data || response;
    return {
      planName: info.plan || info.tier || "88code",
      remaining: info.balance ?? info.credits ?? info.quota ?? null,
      used: info.used ?? info.used_quota ?? info.usage ?? null,
      unit: info.unit || "credits"
    };
  }
})`,

  [TEMPLATE_KEYS.PRIVNODE]: `({
  request: {
    url: "https://privnode.com/api/user/self",
    method: "GET",
    headers: {
      "Authorization": "Bearer {{apiKey}}",
      "Content-Type": "application/json"
    }
  },
  extractor: function (response) {
    if (response.success === false) {
      return { 
        isValid: false, 
        invalidMessage: response.message || "Invalid token" 
      };
    }
    const info = response.data || response;
    const remaining = info.balance ?? info.quota ?? info.credit ?? null;
    const used = info.used_quota ?? info.used ?? info.usage ?? null;
    return {
      planName: info.group || info.role || "Privnode",
      remaining: remaining,
      used: used,
      total: remaining && used ? remaining + used : null,
      unit: info.unit || "credits",
      percentage: remaining && used ? (used / (remaining + used)) * 100 : null
    };
  }
})`,
  [TEMPLATE_KEYS.TOKEN_PLAN]: "",
  [TEMPLATE_KEYS.BALANCE]: "",
});

// 模板名称国际化键映射
const TEMPLATE_NAME_KEYS: Record<string, string> = {
  [TEMPLATE_KEYS.CUSTOM]: "usageScript.templates.custom",
  [TEMPLATE_KEYS.GENERAL]: "usageScript.templates.general",
  [TEMPLATE_KEYS.NEW_API]: "usageScript.templates.newapi",
  [TEMPLATE_KEYS.PACKYCODE]: "usageScript.templates.packycode",
  [TEMPLATE_KEYS.CODE88]: "usageScript.templates.88code",
  [TEMPLATE_KEYS.PRIVNODE]: "usageScript.templates.privnode",
  [TEMPLATE_KEYS.TOKEN_PLAN]: "usageScript.templates.tokenPlan",
  [TEMPLATE_KEYS.BALANCE]: "usageScript.templates.balance",
};

function getProviderCredentials(provider: Provider) {
  const env = provider.settingsConfig?.env ?? {};
  return {
    apiKey:
      env.ANTHROPIC_AUTH_TOKEN ??
      env.ANTHROPIC_API_KEY ??
      env.OPENAI_API_KEY ??
      env.OPENROUTER_API_KEY ??
      env.GOOGLE_API_KEY ??
      "",
    baseUrl:
      env.ANTHROPIC_BASE_URL ??
      env.OPENAI_BASE_URL ??
      env.OPENROUTER_BASE_URL ??
      env.GOOGLE_GEMINI_BASE_URL ??
      "",
  };
}

const API_KEY_TEMPLATES = new Set<string>([
  TEMPLATE_KEYS.GENERAL,
  TEMPLATE_KEYS.PACKYCODE,
  TEMPLATE_KEYS.CODE88,
  TEMPLATE_KEYS.PRIVNODE,
]);

const BASE_URL_TEMPLATES = new Set<string>([
  TEMPLATE_KEYS.GENERAL,
  TEMPLATE_KEYS.CODE88,
]);

const UsageScriptModal: React.FC<UsageScriptModalProps> = ({
  provider,
  appId,
  isOpen,
  onClose,
  onSave,
}) => {
  const { t } = useTranslation();

  // 生成带国际化的预设模板
  const PRESET_TEMPLATES = generatePresetTemplates(t);
  const providerCredentials = getProviderCredentials(provider);

  const [script, setScript] = useState<UsageScript>(() => {
    const saved = provider.meta?.usage_script;
    if (saved) return saved;
    const codingPlan = detectCodingPlanProvider(providerCredentials.baseUrl);
    const balance = detectBalanceProvider(providerCredentials.baseUrl);
    return {
      enabled: false,
      language: "javascript",
      code:
        codingPlan || balance ? "" : PRESET_TEMPLATES[TEMPLATE_KEYS.GENERAL],
      timeout: 10,
      templateType: codingPlan
        ? TEMPLATE_TYPES.TOKEN_PLAN
        : balance
          ? TEMPLATE_TYPES.BALANCE
          : TEMPLATE_TYPES.GENERAL,
      codingPlanProvider: codingPlan?.id,
    };
  });

  const [testing, setTesting] = useState(false);

  // 🔧 输入时的格式化（宽松）- 只清理格式，不约束范围
  const sanitizeNumberInput = (value: string): string => {
    // 移除所有非数字字符
    let cleaned = value.replace(/[^\d]/g, "");

    // 移除前导零（除非输入的就是 "0"）
    if (cleaned.length > 1 && cleaned.startsWith("0")) {
      cleaned = cleaned.replace(/^0+/, "");
    }

    return cleaned;
  };

  // 🔧 失焦时的验证（严格）- 仅确保有效整数
  const validateTimeout = (value: string): number => {
    // 转换为数字
    const num = Number(value);

    // 检查是否为有效数字
    if (isNaN(num) || value.trim() === "") {
      return 10; // 默认值
    }

    // 检查是否为整数
    if (!Number.isInteger(num)) {
      toast.warning(
        t("usageScript.timeoutMustBeInteger") || "超时时间必须为整数",
      );
    }

    // 检查负数
    if (num < 0) {
      toast.error(
        t("usageScript.timeoutCannotBeNegative") || "超时时间不能为负数",
      );
      return 10;
    }

    return Math.floor(num);
  };

  // 🔧 失焦时的验证（严格）- 自动查询间隔
  const validateAndClampInterval = (value: string): number => {
    // 转换为数字
    const num = Number(value);

    // 检查是否为有效数字
    if (isNaN(num) || value.trim() === "") {
      return 0; // 禁用自动查询
    }

    // 检查是否为整数
    if (!Number.isInteger(num)) {
      toast.warning(
        t("usageScript.intervalMustBeInteger") || "自动查询间隔必须为整数",
      );
    }

    // 检查负数
    if (num < 0) {
      toast.error(
        t("usageScript.intervalCannotBeNegative") || "自动查询间隔不能为负数",
      );
      return 0;
    }

    // 约束到 [0, 1440] 范围（最大24小时）
    const clamped = Math.max(0, Math.min(1440, Math.floor(num)));

    // 如果值被调整，显示提示
    if (clamped !== num && num > 0) {
      toast.info(
        t("usageScript.intervalAdjusted", { value: clamped }) ||
          `自动查询间隔已调整为 ${clamped} 分钟`,
      );
    }

    return clamped;
  };

  // 跟踪当前选择的模板类型（用于控制高级配置的显示）
  // 初始化：如果已有 accessToken 或 userId，说明是 NewAPI 模板
  const [selectedTemplate, setSelectedTemplate] = useState<string | null>(
    () => {
      const existingScript = provider.meta?.usage_script;
      if (existingScript?.templateType) return existingScript.templateType;
      if (existingScript?.accessToken || existingScript?.userId) {
        return TEMPLATE_KEYS.NEW_API;
      }
      if (detectCodingPlanProvider(providerCredentials.baseUrl)) {
        return TEMPLATE_TYPES.TOKEN_PLAN;
      }
      if (detectBalanceProvider(providerCredentials.baseUrl)) {
        return TEMPLATE_TYPES.BALANCE;
      }
      return TEMPLATE_TYPES.GENERAL;
    },
  );

  // 控制 API Key 的显示/隐藏
  const [showApiKey, setShowApiKey] = useState(false);
  const [showAccessToken, setShowAccessToken] = useState(false);

  const handleSave = () => {
    // 验证脚本格式
    const nativeTemplate =
      selectedTemplate === TEMPLATE_TYPES.TOKEN_PLAN ||
      selectedTemplate === TEMPLATE_TYPES.BALANCE;
    if (script.enabled && !nativeTemplate && !script.code.trim()) {
      toast.error(t("usageScript.scriptEmpty"));
      return;
    }

    // 基本的 JS 语法检查（检查是否包含 return 语句）
    if (script.enabled && !nativeTemplate && !script.code.includes("return")) {
      toast.error(t("usageScript.mustHaveReturn"), { duration: 5000 });
      return;
    }

    onSave({
      ...script,
      templateType: selectedTemplate as UsageScript["templateType"],
    });
    onClose();
  };

  const handleTest = async () => {
    setTesting(true);
    try {
      // 使用当前编辑器中的脚本内容进行测试
      const result = await usageApi.testScript(
        provider.id,
        appId,
        script.code,
        script.timeout,
        script.apiKey,
        script.baseUrl,
        script.accessToken,
        script.userId,
        selectedTemplate as UsageScript["templateType"],
      );
      if (result.success && result.data && result.data.length > 0) {
        // 显示所有套餐数据
        const summary = result.data
          .map((plan) => {
            const planInfo = plan.planName ? `[${plan.planName}]` : "";
            return `${planInfo} ${t("usage.remaining")} ${plan.remaining} ${plan.unit}`;
          })
          .join(", ");
        toast.success(`${t("usageScript.testSuccess")}${summary}`, {
          duration: 3000,
        });
      } else {
        toast.error(
          `${t("usageScript.testFailed")}: ${result.error || t("endpointTest.noResult")}`,
          {
            duration: 5000,
          },
        );
      }
    } catch (error: any) {
      toast.error(
        `${t("usageScript.testFailed")}: ${error?.message || t("common.unknown")}`,
        {
          duration: 5000,
        },
      );
    } finally {
      setTesting(false);
    }
  };

  const handleFormat = async () => {
    try {
      const [prettier, parserBabel, pluginEstree] = await Promise.all([
        import("prettier/standalone"),
        import("prettier/parser-babel"),
        import("prettier/plugins/estree"),
      ]);
      const formatted = await prettier.format(script.code, {
        parser: "babel",
        plugins: [parserBabel.default, pluginEstree.default],
        semi: true,
        singleQuote: false,
        tabWidth: 2,
        printWidth: 80,
      });
      setScript({ ...script, code: formatted.trim() });
      toast.success(t("usageScript.formatSuccess"), { duration: 1000 });
    } catch (error: any) {
      toast.error(
        `${t("usageScript.formatFailed")}: ${error?.message || t("jsonEditor.invalidJson")}`,
        {
          duration: 3000,
        },
      );
    }
  };

  const handleUsePreset = (presetName: string) => {
    const preset = PRESET_TEMPLATES[presetName];
    if (preset === undefined) return;

    const nextScript: UsageScript = { ...script, code: preset };

    // 根据模板类型清空不同的字段
    if (
      presetName === TEMPLATE_KEYS.TOKEN_PLAN ||
      presetName === TEMPLATE_KEYS.BALANCE
    ) {
      nextScript.apiKey = undefined;
      nextScript.baseUrl = undefined;
      nextScript.accessToken = undefined;
      nextScript.userId = undefined;
      nextScript.codingPlanProvider =
        presetName === TEMPLATE_KEYS.TOKEN_PLAN
          ? (script.codingPlanProvider ??
            detectCodingPlanProvider(providerCredentials.baseUrl)?.id ??
            "kimi")
          : undefined;
    } else if (presetName === TEMPLATE_KEYS.CUSTOM) {
      // 自定义：清空所有凭证字段
      nextScript.apiKey = undefined;
      nextScript.baseUrl = undefined;
      nextScript.accessToken = undefined;
      nextScript.userId = undefined;
    } else if (presetName === TEMPLATE_KEYS.NEW_API) {
      // NewAPI：清空 apiKey（NewAPI 不使用通用的 apiKey）
      nextScript.apiKey = undefined;
    } else {
      // 其他通用/服务商模板：清理 NewAPI 字段
      nextScript.accessToken = undefined;
      nextScript.userId = undefined;
    }

    setScript(nextScript);
    setSelectedTemplate(presetName); // 记录选择的模板
  };

  // 判断是否应该显示凭证配置区域
  const shouldShowCredentialsConfig =
    (selectedTemplate ? API_KEY_TEMPLATES.has(selectedTemplate) : false) ||
    selectedTemplate === TEMPLATE_KEYS.NEW_API;
  const showApiKeyFields =
    selectedTemplate !== null && API_KEY_TEMPLATES.has(selectedTemplate);
  const showBaseUrlInput =
    selectedTemplate !== null && BASE_URL_TEMPLATES.has(selectedTemplate);

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-3xl max-h-[90vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>
            {t("usageScript.title")} - {provider.name}
          </DialogTitle>
          <DialogDescription>
            {t("usageScript.description", {
              defaultValue: "配置查询余额或用量的脚本和凭证。",
            })}
          </DialogDescription>
        </DialogHeader>

        {/* Content - Scrollable */}
        <div className="flex-1 overflow-y-auto px-6 py-4 space-y-4">
          {/* 启用开关 */}
          <div className="flex items-center justify-between gap-4 rounded-lg border border-border-default p-4">
            <div className="space-y-1">
              <p className="text-sm font-medium leading-none">
                {t("usageScript.enableUsageQuery")}
              </p>
            </div>
            <Switch
              checked={script.enabled}
              onCheckedChange={(checked) =>
                setScript({ ...script, enabled: checked })
              }
              aria-label={t("usageScript.enableUsageQuery")}
            />
          </div>

          {script.enabled && (
            <>
              {/* 预设模板选择 */}
              <div>
                <Label className="mb-2">
                  {t("usageScript.presetTemplate")}
                </Label>
                <div className="flex gap-2">
                  {Object.keys(PRESET_TEMPLATES).map((name) => {
                    const isSelected = selectedTemplate === name;
                    return (
                      <button
                        key={name}
                        onClick={() => handleUsePreset(name)}
                        className={`px-3 py-1.5 text-xs rounded transition-colors ${
                          isSelected
                            ? "bg-blue-500 text-white dark:bg-blue-600"
                            : "bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700"
                        }`}
                      >
                        {t(TEMPLATE_NAME_KEYS[name])}
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* 凭证配置区域：通用和 NewAPI 模板显示 */}
              {shouldShowCredentialsConfig && (
                <div className="space-y-4 p-4 bg-gray-50 dark:bg-gray-800/50 rounded-lg">
                  <h4 className="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {t("usageScript.credentialsConfig")}
                  </h4>

                  {/* 通用/服务商模板：显示 apiKey + baseUrl（按需） */}
                  {showApiKeyFields && (
                    <>
                      <div className="space-y-2">
                        <Label htmlFor="usage-api-key">API Key</Label>
                        <div className="relative">
                          <Input
                            id="usage-api-key"
                            type="text"
                            value={script.apiKey || ""}
                            onChange={(e) =>
                              setScript({ ...script, apiKey: e.target.value })
                            }
                            placeholder="sk-xxxxx"
                            autoComplete="off"
                            className={showApiKey ? "" : "secret-text-security"}
                          />
                          {script.apiKey && (
                            <button
                              type="button"
                              onClick={() => setShowApiKey(!showApiKey)}
                              className="absolute inset-y-0 right-0 flex items-center pr-3 text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 transition-colors"
                              aria-label={
                                showApiKey
                                  ? t("apiKeyInput.hide")
                                  : t("apiKeyInput.show")
                              }
                            >
                              {showApiKey ? (
                                <EyeOff size={16} />
                              ) : (
                                <Eye size={16} />
                              )}
                            </button>
                          )}
                        </div>
                      </div>

                      {showBaseUrlInput && (
                        <div className="space-y-2">
                          <Label htmlFor="usage-base-url">Base URL</Label>
                          <Input
                            id="usage-base-url"
                            type="text"
                            value={script.baseUrl || ""}
                            onChange={(e) =>
                              setScript({ ...script, baseUrl: e.target.value })
                            }
                            placeholder="https://api.example.com"
                            autoComplete="off"
                          />
                        </div>
                      )}
                    </>
                  )}

                  {/* NewAPI 模板：显示 baseUrl + accessToken + userId */}
                  {selectedTemplate === TEMPLATE_KEYS.NEW_API && (
                    <>
                      <div className="space-y-2">
                        <Label htmlFor="usage-newapi-base-url">Base URL</Label>
                        <Input
                          id="usage-newapi-base-url"
                          type="text"
                          value={script.baseUrl || ""}
                          onChange={(e) =>
                            setScript({ ...script, baseUrl: e.target.value })
                          }
                          placeholder="https://api.newapi.com"
                          autoComplete="off"
                        />
                      </div>

                      <div className="space-y-2">
                        <Label htmlFor="usage-access-token">
                          {t("usageScript.accessToken")}
                        </Label>
                        <div className="relative">
                          <Input
                            id="usage-access-token"
                            type="text"
                            value={script.accessToken || ""}
                            onChange={(e) =>
                              setScript({
                                ...script,
                                accessToken: e.target.value,
                              })
                            }
                            placeholder={t(
                              "usageScript.accessTokenPlaceholder",
                            )}
                            autoComplete="off"
                            className={
                              showAccessToken ? "" : "secret-text-security"
                            }
                          />
                          {script.accessToken && (
                            <button
                              type="button"
                              onClick={() =>
                                setShowAccessToken(!showAccessToken)
                              }
                              className="absolute inset-y-0 right-0 flex items-center pr-3 text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 transition-colors"
                              aria-label={
                                showAccessToken
                                  ? t("apiKeyInput.hide")
                                  : t("apiKeyInput.show")
                              }
                            >
                              {showAccessToken ? (
                                <EyeOff size={16} />
                              ) : (
                                <Eye size={16} />
                              )}
                            </button>
                          )}
                        </div>
                      </div>

                      <div className="space-y-2">
                        <Label htmlFor="usage-user-id">
                          {t("usageScript.userId")}
                        </Label>
                        <Input
                          id="usage-user-id"
                          type="text"
                          value={script.userId || ""}
                          onChange={(e) =>
                            setScript({ ...script, userId: e.target.value })
                          }
                          placeholder={t("usageScript.userIdPlaceholder")}
                          autoComplete="off"
                        />
                      </div>
                    </>
                  )}
                </div>
              )}

              {selectedTemplate === TEMPLATE_KEYS.TOKEN_PLAN ? (
                <div className="flex flex-wrap gap-2 border-t border-border-default pt-3">
                  {CODING_PLAN_PROVIDERS.map((provider) => (
                    <Button
                      key={provider.id}
                      type="button"
                      size="sm"
                      variant={
                        script.codingPlanProvider === provider.id
                          ? "default"
                          : "outline"
                      }
                      onClick={() =>
                        setScript({
                          ...script,
                          codingPlanProvider: provider.id,
                        })
                      }
                    >
                      {provider.label}
                    </Button>
                  ))}
                </div>
              ) : null}

              {/* 脚本编辑器 */}
              {selectedTemplate !== TEMPLATE_KEYS.TOKEN_PLAN &&
              selectedTemplate !== TEMPLATE_KEYS.BALANCE ? (
                <div>
                  <Label className="mb-2">{t("usageScript.queryScript")}</Label>
                  <React.Suspense
                    fallback={
                      <textarea
                        className="h-[300px] w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-sm"
                        value={script.code}
                        onChange={(event) =>
                          setScript({ ...script, code: event.target.value })
                        }
                      />
                    }
                  >
                    <JsonEditor
                      value={script.code}
                      onChange={(code) => setScript({ ...script, code })}
                      height="300px"
                      language="javascript"
                    />
                  </React.Suspense>
                  <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                    {t("usageScript.variablesHint", {
                      apiKey: "{{apiKey}}",
                      baseUrl: "{{baseUrl}}",
                    })}
                  </p>
                </div>
              ) : null}

              {/* 配置选项 */}
              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <Label htmlFor="usage-timeout">
                    {t("usageScript.timeoutSeconds")}
                  </Label>
                  <Input
                    id="usage-timeout"
                    type="number"
                    value={script.timeout ?? ""}
                    onChange={(e) => {
                      // 输入时：只清理格式，允许临时为空，避免强制回填默认值
                      const cleaned = sanitizeNumberInput(e.target.value);
                      setScript((prev) => ({
                        ...prev,
                        timeout:
                          cleaned === "" ? undefined : parseInt(cleaned, 10),
                      }));
                    }}
                    onBlur={(e) => {
                      // 失焦时：严格验证并约束范围
                      const validated = validateTimeout(e.target.value);
                      setScript({ ...script, timeout: validated });
                    }}
                  />
                  <p className="text-xs text-muted-foreground">
                    {t("usageScript.timeoutHint") || "范围: 2-30 秒"}
                  </p>
                </div>

                {/* 🆕 自动查询间隔 */}
                <div className="space-y-2">
                  <Label htmlFor="usage-auto-interval">
                    {t("usageScript.autoQueryInterval")}
                  </Label>
                  <Input
                    id="usage-auto-interval"
                    type="number"
                    min={0}
                    max={1440}
                    step={1}
                    value={script.autoQueryInterval ?? ""}
                    onChange={(e) => {
                      // 输入时：只清理格式，允许临时为空
                      const cleaned = sanitizeNumberInput(e.target.value);
                      setScript((prev) => ({
                        ...prev,
                        autoQueryInterval:
                          cleaned === "" ? undefined : parseInt(cleaned, 10),
                      }));
                    }}
                    onBlur={(e) => {
                      // 失焦时：严格验证并约束范围
                      const validated = validateAndClampInterval(
                        e.target.value,
                      );
                      setScript({ ...script, autoQueryInterval: validated });
                    }}
                  />
                  <p className="text-xs text-muted-foreground">
                    {t("usageScript.autoQueryIntervalHint")}
                  </p>
                </div>
              </div>

              {/* 脚本说明 */}
              {selectedTemplate !== TEMPLATE_KEYS.TOKEN_PLAN &&
              selectedTemplate !== TEMPLATE_KEYS.BALANCE ? (
                <div className="p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg text-sm text-gray-700 dark:text-gray-300">
                  <h4 className="font-medium mb-2">
                    {t("usageScript.scriptHelp")}
                  </h4>
                  <div className="space-y-3 text-xs">
                    <div>
                      <strong>{t("usageScript.configFormat")}</strong>
                      <pre className="mt-1 p-2 bg-white/50 dark:bg-black/20 rounded text-[10px] overflow-x-auto">
                        {`({
  request: {
    url: "{{baseUrl}}/api/usage",
    method: "POST",
    headers: {
      "Authorization": "Bearer {{apiKey}}",
      "User-Agent": "cc-switch/1.0"
    },
    body: JSON.stringify({ key: "value" })  // ${t("usageScript.commentOptional")}
  },
  extractor: function(response) {
    // ${t("usageScript.commentResponseIsJson")}
    return {
      isValid: !response.error,
      remaining: response.balance,
      unit: "USD"
    };
  }
})`}
                      </pre>
                    </div>

                    <div>
                      <strong>{t("usageScript.extractorFormat")}</strong>
                      <ul className="mt-1 space-y-0.5 ml-2">
                        <li>{t("usageScript.fieldIsValid")}</li>
                        <li>{t("usageScript.fieldInvalidMessage")}</li>
                        <li>{t("usageScript.fieldRemaining")}</li>
                        <li>{t("usageScript.fieldUnit")}</li>
                        <li>{t("usageScript.fieldPlanName")}</li>
                        <li>{t("usageScript.fieldTotal")}</li>
                        <li>{t("usageScript.fieldUsed")}</li>
                        <li>{t("usageScript.fieldExtra")}</li>
                      </ul>
                    </div>

                    <div className="text-gray-600 dark:text-gray-400">
                      <strong>{t("usageScript.tips")}</strong>
                      <ul className="mt-1 space-y-0.5 ml-2">
                        <li>
                          {t("usageScript.tip1", {
                            apiKey: "{{apiKey}}",
                            baseUrl: "{{baseUrl}}",
                          })}
                        </li>
                        <li>{t("usageScript.tip2")}</li>
                        <li>{t("usageScript.tip3")}</li>
                      </ul>
                    </div>
                  </div>
                </div>
              ) : null}
            </>
          )}
        </div>

        {/* Footer */}
        <DialogFooter className="flex-col sm:flex-row sm:justify-between gap-3 pt-4">
          {/* Left side - Test and Format buttons */}
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={handleTest}
              disabled={!script.enabled || testing}
            >
              <Play size={14} />
              {testing ? t("usageScript.testing") : t("usageScript.testScript")}
            </Button>
            {selectedTemplate !== TEMPLATE_KEYS.TOKEN_PLAN &&
            selectedTemplate !== TEMPLATE_KEYS.BALANCE ? (
              <Button
                variant="outline"
                size="sm"
                onClick={handleFormat}
                disabled={!script.enabled}
                title={t("usageScript.format")}
              >
                <Wand2 size={14} />
                {t("usageScript.format")}
              </Button>
            ) : null}
          </div>

          {/* Right side - Cancel and Save buttons */}
          <div className="flex gap-2">
            <Button variant="ghost" size="sm" onClick={onClose}>
              {t("common.cancel")}
            </Button>
            <Button variant="default" size="sm" onClick={handleSave}>
              {t("usageScript.saveConfig")}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};

export default UsageScriptModal;
