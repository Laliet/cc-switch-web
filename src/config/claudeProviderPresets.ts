/**
 * 预设供应商配置模板
 */
import { ProviderCategory } from "../types";

export interface TemplateValueConfig {
  label: string;
  placeholder: string;
  defaultValue?: string;
  editorValue: string;
}

/**
 * 预设供应商的视觉主题配置
 */
export interface PresetTheme {
  /** 图标类型：'claude' | 'codex' | 'gemini' | 'generic' */
  icon?: "claude" | "codex" | "gemini" | "generic";
  /** 背景色（选中状态），支持 Tailwind 类名或 hex 颜色 */
  backgroundColor?: string;
  /** 文字色（选中状态），支持 Tailwind 类名或 hex 颜色 */
  textColor?: string;
}

export interface ProviderPreset {
  name: string;
  websiteUrl: string;
  // 新增：第三方/聚合等可单独配置获取 API Key 的链接
  apiKeyUrl?: string;
  settingsConfig: object;
  isOfficial?: boolean; // 标识是否为官方预设
  isPartner?: boolean; // 标识是否为商业合作伙伴
  partnerPromotionKey?: string; // 合作伙伴促销信息的 i18n key
  category?: ProviderCategory; // 新增：分类
  // 新增：指定该预设所使用的 API Key 字段名（默认 ANTHROPIC_AUTH_TOKEN）
  apiKeyField?: "ANTHROPIC_AUTH_TOKEN" | "ANTHROPIC_API_KEY";
  // 新增：模板变量定义，用于动态替换配置中的值
  templateValues?: Record<string, TemplateValueConfig>; // editorValue 存储编辑器中的实时输入值
  // 新增：请求地址候选列表（用于地址管理/测速）
  endpointCandidates?: string[];
  // OAuth 托管认证 provider 类型
  providerType?: "github_copilot" | "codex_oauth";
  requiresOAuth?: boolean;
  // 新增：视觉主题配置
  theme?: PresetTheme;
}

const claudeRoleModelEnv = (
  sonnet = "claude-sonnet-4-6",
  opus = "claude-opus-4-7",
  haiku = "claude-haiku-4-5",
) => ({
  ANTHROPIC_MODEL: sonnet,
  ANTHROPIC_DEFAULT_HAIKU_MODEL: haiku,
  ANTHROPIC_DEFAULT_SONNET_MODEL: sonnet,
  ANTHROPIC_DEFAULT_OPUS_MODEL: opus,
});

const anthropicCompatiblePreset = (
  baseUrl: string,
  modelEnv = claudeRoleModelEnv(),
) => ({
  env: {
    ANTHROPIC_BASE_URL: baseUrl,
    ANTHROPIC_AUTH_TOKEN: "",
    ...modelEnv,
  },
});

export const providerPresets: ProviderPreset[] = [
  {
    name: "Claude Official",
    websiteUrl: "https://www.anthropic.com/claude-code",
    settingsConfig: {
      env: {},
    },
    isOfficial: true, // 明确标识为官方预设
    category: "official",
    theme: {
      icon: "claude",
      backgroundColor: "#D97757",
      textColor: "#FFFFFF",
    },
  },
  {
    name: "GitHub Copilot",
    websiteUrl: "https://github.com/features/copilot",
    settingsConfig: anthropicCompatiblePreset(
      "https://api.githubcopilot.com",
      claudeRoleModelEnv(
        "claude-sonnet-4.6",
        "claude-sonnet-4.6",
        "claude-haiku-4.5",
      ),
    ),
    endpointCandidates: ["https://api.githubcopilot.com"],
    category: "third_party",
    providerType: "github_copilot",
    requiresOAuth: true,
    theme: {
      icon: "generic",
      backgroundColor: "#111827",
      textColor: "#FFFFFF",
    },
  },
  {
    name: "Codex OAuth",
    websiteUrl: "https://chatgpt.com/codex",
    settingsConfig: anthropicCompatiblePreset(
      "https://chatgpt.com/backend-api/codex",
      claudeRoleModelEnv("gpt-5-codex", "gpt-5-codex", "gpt-5-codex"),
    ),
    endpointCandidates: ["https://chatgpt.com/backend-api/codex"],
    category: "third_party",
    providerType: "codex_oauth",
    requiresOAuth: true,
    theme: {
      icon: "codex",
      backgroundColor: "#111827",
      textColor: "#FFFFFF",
    },
  },
  {
    name: "PatewayAI",
    websiteUrl: "https://api.pateway.ai",
    apiKeyUrl: "https://api.pateway.ai",
    settingsConfig: anthropicCompatiblePreset("https://api.pateway.ai"),
    endpointCandidates: ["https://api.pateway.ai"],
    category: "third_party",
    isPartner: true,
    partnerPromotionKey: "patewayai",
  },
  {
    name: "火山Agentplan",
    websiteUrl:
      "https://www.volcengine.com/activity/agentplan?utm_campaign=hw&utm_content=ccswitch&utm_medium=devrel_tool_web&utm_source=OWO&utm_term=ccswitch",
    apiKeyUrl:
      "https://www.volcengine.com/activity/agentplan?utm_campaign=hw&utm_content=ccswitch&utm_medium=devrel_tool_web&utm_source=OWO&utm_term=ccswitch",
    settingsConfig: anthropicCompatiblePreset(
      "https://ark.cn-beijing.volces.com/api/coding",
      claudeRoleModelEnv(
        "ark-code-latest",
        "ark-code-latest",
        "ark-code-latest",
      ),
    ),
    endpointCandidates: ["https://ark.cn-beijing.volces.com/api/coding"],
    category: "cn_official",
    isPartner: true,
    partnerPromotionKey: "volcengine_agentplan",
  },
  {
    name: "BytePlus",
    websiteUrl:
      "https://www.byteplus.com/en/product/modelark?utm_campaign=hw&utm_content=ccswitch&utm_medium=devrel_tool_web&utm_source=OWO&utm_term=ccswitch",
    apiKeyUrl:
      "https://www.byteplus.com/en/product/modelark?utm_campaign=hw&utm_content=ccswitch&utm_medium=devrel_tool_web&utm_source=OWO&utm_term=ccswitch",
    settingsConfig: anthropicCompatiblePreset(
      "https://ark.ap-southeast.bytepluses.com/api/coding",
      claudeRoleModelEnv(
        "ark-code-latest",
        "ark-code-latest",
        "ark-code-latest",
      ),
    ),
    endpointCandidates: ["https://ark.ap-southeast.bytepluses.com/api/coding"],
    category: "cn_official",
    isPartner: true,
    partnerPromotionKey: "byteplus",
  },
  {
    name: "Baidu Qianfan Coding Plan",
    websiteUrl: "https://cloud.baidu.com/product/qianfan_modelbuilder",
    apiKeyUrl: "https://cloud.baidu.com/product/qianfan_modelbuilder",
    settingsConfig: anthropicCompatiblePreset(
      "https://qianfan.baidubce.com/anthropic/coding",
      claudeRoleModelEnv(
        "qianfan-code-latest",
        "qianfan-code-latest",
        "qianfan-code-latest",
      ),
    ),
    endpointCandidates: ["https://qianfan.baidubce.com/anthropic/coding"],
    category: "cn_official",
  },
  {
    name: "ClaudeAPI",
    websiteUrl: "https://claudeapi.com",
    apiKeyUrl: "https://claudeapi.com",
    settingsConfig: anthropicCompatiblePreset("https://gw.claudeapi.com"),
    endpointCandidates: ["https://gw.claudeapi.com"],
    category: "third_party",
    isPartner: true,
    partnerPromotionKey: "claudeapi",
  },
  {
    name: "ClaudeCN",
    websiteUrl: "https://claudecn.top",
    apiKeyUrl: "https://claudecn.top/register?aff=ccswitch",
    settingsConfig: anthropicCompatiblePreset("https://claudecn.top"),
    endpointCandidates: ["https://claudecn.top"],
    category: "third_party",
    isPartner: true,
    partnerPromotionKey: "claudecn",
  },
  {
    name: "RunAPI",
    websiteUrl: "https://runapi.co",
    apiKeyUrl: "https://runapi.co",
    settingsConfig: anthropicCompatiblePreset("https://runapi.co"),
    endpointCandidates: ["https://runapi.co"],
    category: "aggregator",
    isPartner: true,
    partnerPromotionKey: "runapi",
  },
  {
    name: "RelaxyCode",
    websiteUrl: "https://www.relaxycode.com",
    apiKeyUrl: "https://www.relaxycode.com",
    settingsConfig: anthropicCompatiblePreset("https://www.relaxycode.com"),
    endpointCandidates: ["https://www.relaxycode.com"],
    category: "third_party",
  },
  {
    name: "Compshare",
    websiteUrl: "https://www.compshare.cn",
    apiKeyUrl: "https://www.compshare.cn",
    settingsConfig: anthropicCompatiblePreset("https://api.modelverse.cn"),
    endpointCandidates: ["https://api.modelverse.cn"],
    category: "aggregator",
    isPartner: true,
    partnerPromotionKey: "ucloud",
  },
  {
    name: "DeepSeek",
    websiteUrl: "https://platform.deepseek.com",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.deepseek.com/anthropic",
        ANTHROPIC_AUTH_TOKEN: "",
        ANTHROPIC_MODEL: "deepseek-chat",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "deepseek-chat",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "deepseek-chat",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "deepseek-chat",
      },
    },
    category: "cn_official",
  },
  {
    name: "Zhipu GLM",
    websiteUrl: "https://open.bigmodel.cn",
    apiKeyUrl: "https://www.bigmodel.cn/claude-code?ic=RRVJPB5SII",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://open.bigmodel.cn/api/anthropic",
        ANTHROPIC_AUTH_TOKEN: "",
        API_TIMEOUT_MS: "3000000",
        CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: "1",
        ANTHROPIC_MODEL: "glm-4.7",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "glm-4.5-air",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "glm-4.7",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "glm-4.7",
      },
    },
    category: "cn_official",
    isPartner: true, // 合作伙伴
    partnerPromotionKey: "zhipu", // 促销信息 i18n key
  },
  {
    name: "Z.ai GLM",
    websiteUrl: "https://z.ai",
    apiKeyUrl: "https://z.ai/subscribe?ic=8JVLJQFSKB",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.z.ai/api/anthropic",
        ANTHROPIC_AUTH_TOKEN: "",
        ANTHROPIC_MODEL: "glm-4.6",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "glm-4.5-air",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "glm-4.6",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "glm-4.6",
      },
    },
    category: "cn_official",
    isPartner: true, // 合作伙伴
    partnerPromotionKey: "zhipu", // 促销信息 i18n key
  },
  {
    name: "Qwen Coder",
    websiteUrl: "https://bailian.console.aliyun.com",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://dashscope.aliyuncs.com/apps/anthropic",
        ANTHROPIC_AUTH_TOKEN: "",
        ANTHROPIC_MODEL: "qwen-max",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "qwen-max",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "qwen-max",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "qwen-max",
      },
    },
    category: "cn_official",
  },
  {
    name: "Kimi k2",
    websiteUrl: "https://platform.moonshot.cn/console",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.moonshot.cn/anthropic",
        ANTHROPIC_AUTH_TOKEN: "",
        ANTHROPIC_MODEL: "kimi-k2-thinking",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "kimi-k2-thinking",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "kimi-k2-thinking",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "kimi-k2-thinking",
      },
    },
    category: "cn_official",
  },
  {
    name: "Kimi For Coding",
    websiteUrl: "https://www.kimi.com/coding/docs/",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.kimi.com/coding/",
        ANTHROPIC_AUTH_TOKEN: "",
        ANTHROPIC_MODEL: "kimi-for-coding",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "kimi-for-coding",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "kimi-for-coding",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "kimi-for-coding",
      },
    },
    category: "cn_official",
  },
  {
    name: "ModelScope",
    websiteUrl: "https://modelscope.cn",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api-inference.modelscope.cn",
        ANTHROPIC_AUTH_TOKEN: "",
        ANTHROPIC_MODEL: "ZhipuAI/GLM-4.6",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "ZhipuAI/GLM-4.6",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "ZhipuAI/GLM-4.6",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "ZhipuAI/GLM-4.6",
      },
    },
    category: "aggregator",
  },
  {
    name: "KAT-Coder",
    websiteUrl: "https://console.streamlake.ai",
    apiKeyUrl: "https://console.streamlake.ai/console/api-key",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL:
          "https://vanchin.streamlake.ai/api/gateway/v1/endpoints/${ENDPOINT_ID}/claude-code-proxy",
        ANTHROPIC_AUTH_TOKEN: "",
        ANTHROPIC_MODEL: "KAT-Coder-Pro V1",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "KAT-Coder-Air V1",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "KAT-Coder-Pro V1",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "KAT-Coder-Pro V1",
      },
    },
    category: "cn_official",
    templateValues: {
      ENDPOINT_ID: {
        label: "Vanchin Endpoint ID",
        placeholder: "ep-xxx-xxx",
        defaultValue: "",
        editorValue: "",
      },
    },
  },
  {
    name: "Longcat",
    websiteUrl: "https://longcat.chat/platform",
    apiKeyUrl: "https://longcat.chat/platform/api_keys",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.longcat.chat/anthropic",
        ANTHROPIC_AUTH_TOKEN: "",
        ANTHROPIC_MODEL: "LongCat-Flash-Chat",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "LongCat-Flash-Chat",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "LongCat-Flash-Chat",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "LongCat-Flash-Chat",
        CLAUDE_CODE_MAX_OUTPUT_TOKENS: "6000",
        CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: 1,
      },
    },
    category: "cn_official",
  },
  {
    name: "MiniMax",
    websiteUrl: "https://platform.minimaxi.com",
    apiKeyUrl: "https://platform.minimaxi.com/user-center/basic-information",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.minimaxi.com/anthropic",
        ANTHROPIC_AUTH_TOKEN: "",
        API_TIMEOUT_MS: "3000000",
        CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: 1,
        ANTHROPIC_MODEL: "MiniMax-M2.1",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "MiniMax-M2.1",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "MiniMax-M2.1",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "MiniMax-M2.1",
      },
    },
    category: "cn_official",
  },
  {
    name: "DouBaoSeed",
    websiteUrl: "https://www.volcengine.com/product/doubao",
    apiKeyUrl: "https://www.volcengine.com/product/doubao",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://ark.cn-beijing.volces.com/api/coding",
        ANTHROPIC_AUTH_TOKEN: "",
        API_TIMEOUT_MS: "3000000",
        ANTHROPIC_MODEL: "doubao-seed-code-preview-latest",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "doubao-seed-code-preview-latest",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "doubao-seed-code-preview-latest",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "doubao-seed-code-preview-latest",
      },
    },
    category: "cn_official",
  },
  {
    name: "BaiLing",
    websiteUrl: "https://alipaytbox.yuque.com/sxs0ba/ling/get_started",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.tbox.cn/api/anthropic",
        ANTHROPIC_AUTH_TOKEN: "",
        ANTHROPIC_MODEL: "Ling-1T",
        ANTHROPIC_DEFAULT_HAIKU_MODEL: "Ling-1T",
        ANTHROPIC_DEFAULT_SONNET_MODEL: "Ling-1T",
        ANTHROPIC_DEFAULT_OPUS_MODEL: "Ling-1T",
      },
    },
    category: "cn_official",
  },
  {
    name: "AiHubMix",
    websiteUrl: "https://aihubmix.com",
    apiKeyUrl: "https://aihubmix.com",
    // 说明：该供应商使用 ANTHROPIC_API_KEY（而非 ANTHROPIC_AUTH_TOKEN）
    apiKeyField: "ANTHROPIC_API_KEY",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://aihubmix.com",
        ANTHROPIC_API_KEY: "",
      },
    },
    // 请求地址候选（用于地址管理/测速），用户可自行选择/覆盖
    endpointCandidates: ["https://aihubmix.com", "https://api.aihubmix.com"],
    category: "aggregator",
  },
  {
    name: "DMXAPI",
    websiteUrl: "https://www.dmxapi.cn",
    apiKeyUrl: "https://www.dmxapi.cn",
    // 说明：该供应商使用 ANTHROPIC_API_KEY（而非 ANTHROPIC_AUTH_TOKEN）
    apiKeyField: "ANTHROPIC_API_KEY",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://www.dmxapi.cn",
        ANTHROPIC_API_KEY: "",
      },
    },
    // 请求地址候选（用于地址管理/测速），用户可自行选择/覆盖
    endpointCandidates: ["https://www.dmxapi.cn"],
    category: "aggregator",
  },
  {
    name: "OpenRouter",
    websiteUrl: "https://openrouter.ai",
    apiKeyUrl: "https://openrouter.ai/keys",
    apiKeyField: "ANTHROPIC_API_KEY",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://openrouter.ai/api",
        ANTHROPIC_API_KEY: "",
      },
    },
    category: "aggregator",
  },
  {
    name: "PackyCode",
    websiteUrl: "https://www.packyapi.com",
    apiKeyUrl: "https://www.packyapi.com/register?aff=cc-switch",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://www.packyapi.com",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    // 请求地址候选（用于地址管理/测速）
    endpointCandidates: [
      "https://www.packyapi.com",
      "https://api-slb.packyapi.com",
    ],
    category: "third_party",
    isPartner: true, // 合作伙伴
    partnerPromotionKey: "packycode", // 促销信息 i18n key
  },
  {
    name: "88code",
    websiteUrl: "https://www.88code.ai",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://www.88code.ai/api",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://www.88code.ai/api"],
    category: "third_party",
  },
  {
    name: "AICodeMirror",
    websiteUrl: "https://www.aicodemirror.com",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.aicodemirror.com/api/claudecode",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://api.aicodemirror.com/api/claudecode"],
    category: "third_party",
  },
  {
    name: "AIMZ",
    websiteUrl: "https://mzlone.top",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://mzlone.top",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://mzlone.top"],
    category: "third_party",
  },
  {
    name: "Anyrouter",
    websiteUrl: "https://anyrouter.top",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://anyrouter.top",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://anyrouter.top"],
    category: "third_party",
  },
  {
    name: "CCFly",
    websiteUrl: "https://ccfly.online",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://apic.cikew.site/api",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://apic.cikew.site/api"],
    category: "third_party",
  },
  {
    name: "ClaudePro",
    websiteUrl: "https://pro.aipor.cc",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://pro.aipor.cc/api",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://pro.aipor.cc/api"],
    category: "third_party",
  },
  {
    name: "CodeCli",
    websiteUrl: "https://code-cli.cn",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://code-cli.cn/api/claudecode",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://code-cli.cn/api/claudecode"],
    category: "third_party",
  },
  {
    name: "EasyChat",
    websiteUrl: "https://easychat.site",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://server.easychat.site",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://server.easychat.site"],
    category: "third_party",
  },
  {
    name: "FastCode",
    websiteUrl: "https://api.timebackward.com",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.timebackward.com",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://api.timebackward.com"],
    category: "third_party",
  },
  {
    name: "FoxCode",
    websiteUrl: "https://foxcode.rjj.cc",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://code.newcli.com/claude/aws",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://code.newcli.com/claude/aws"],
    category: "third_party",
  },
  {
    name: "GalaxyCode",
    websiteUrl: "https://nf.video",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://relay.nf.video",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://relay.nf.video"],
    category: "third_party",
  },
  {
    name: "HuggingCode",
    websiteUrl: "https://huggingcode.cc",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://huggingcode.cc",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://huggingcode.cc"],
    category: "third_party",
  },
  {
    name: "JikeAI",
    websiteUrl: "https://magic666.top",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://magic666.top",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://magic666.top"],
    category: "third_party",
  },
  {
    name: "LinkAPI",
    websiteUrl: "https://linkapi.ai",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.linkapi.ai",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://api.linkapi.ai"],
    category: "third_party",
  },
  {
    name: "MikuCode",
    websiteUrl: "https://mikucode.xyz",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://mikucode.xyz",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://mikucode.xyz"],
    category: "third_party",
  },
  {
    name: "OCC",
    websiteUrl: "https://www.openclaudecode.cn",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://www.openclaudecode.cn",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://www.openclaudecode.cn"],
    category: "third_party",
  },
  {
    name: "SSSAiCode",
    websiteUrl: "https://www.sssaicode.com",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://claude3.sssaicode.com/api",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://claude3.sssaicode.com/api"],
    category: "third_party",
  },
  {
    name: "YesCode",
    websiteUrl: "https://co.yes.vg",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://co.yes.vg",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://co.yes.vg"],
    category: "third_party",
  },
  {
    name: "cubence",
    websiteUrl: "https://cubence.com",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api-dmit.cubence.com",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://api-dmit.cubence.com"],
    category: "third_party",
  },
  {
    name: "duckcoding",
    websiteUrl: "https://duckcoding.com",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://jp.duckcoding.com",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://jp.duckcoding.com"],
    category: "third_party",
  },
  {
    name: "ikuncode",
    websiteUrl: "https://api.ikuncode.cc",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.ikuncode.cc",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://api.ikuncode.cc"],
    category: "third_party",
  },
  {
    name: "privnode",
    websiteUrl: "https://privnode.com",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://privnode.com",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://privnode.com"],
    category: "third_party",
  },
  {
    name: "uucode",
    websiteUrl: "https://www.uucode.org",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://api.uucode.org",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://api.uucode.org"],
    category: "third_party",
  },
  {
    name: "xyai",
    websiteUrl: "https://new.xychatai.com",
    settingsConfig: {
      env: {
        ANTHROPIC_BASE_URL: "https://new.xychatai.com/claude_api",
        ANTHROPIC_AUTH_TOKEN: "",
      },
    },
    endpointCandidates: ["https://new.xychatai.com/claude_api"],
    category: "third_party",
  },
];
