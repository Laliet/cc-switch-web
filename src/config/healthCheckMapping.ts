/**
 * CC-Switch 供应商名称到 Relay-Pulse 供应商名称的映射
 *
 * Relay-Pulse 监控的供应商列表（截至 2025-11）：
 * - 88code (cc, cx)
 * - duckcoding (cc, cx)
 * - privnode (cc, cx)
 * - PackyCode (cc, cx)
 * - FoxCode (cc, cx)
 * - GalaxyCode (cc, cx)
 * - YesCode (cc, cx)
 * - augmunt (cc, cx)
 * - ikuncode (cc, cx)
 * - uucode (cc, cx)
 * - xyai (cc, cx)
 * - cubence (cc)
 * - elysiver (cc) - 公益站
 * - runanytime (cc) - 公益站
 * - right.codes (cx)
 * - xychatai (cx)
 */

import type { Provider } from "@/types";

/**
 * CC-Switch 预设名称 → Relay-Pulse 供应商名称映射
 * 键：CC-Switch 中的供应商预设名称（大小写不敏感匹配）
 * 值：Relay-Pulse 中的 provider 名称（小写）
 */
export const PROVIDER_NAME_MAPPING: Record<string, string> = {
  // 第三方 API 服务商（Relay-Pulse 监控）
  "88code": "88code",
  aicodemirror: "aicodemirror",
  aimz: "aimz",
  anyrouter: "anyrouter",
  ccfly: "ccfly",
  claudecn: "claudecn",
  claudepro: "claudepro",
  codecli: "codecli",
  dmxapi: "dmxapi",
  aihubmix: "aihubmix",
  easychat: "easychat",
  fastcode: "fastcode",
  huggingcode: "huggingcode",
  jikeai: "jikeai",
  linkapi: "linkapi",
  mikucode: "mikucode",
  occ: "occ",
  sssaicode: "sssaicode",
  duckcoding: "duckcoding",
  "duck coding": "duckcoding",
  privnode: "privnode",
  packycode: "packycode",
  "packy code": "packycode",
  foxcode: "foxcode",
  "fox code": "foxcode",
  galaxycode: "galaxycode",
  "galaxy code": "galaxycode",
  yescode: "yescode",
  "yes code": "yescode",
  augmunt: "augmunt",
  ikuncode: "ikuncode",
  "ikun code": "ikuncode",
  uucode: "uucode",
  "uu code": "uucode",
  xyai: "xyai",
  "xy ai": "xyai",
  cubence: "cubence",
  elysiver: "elysiver",
  runanytime: "runanytime",
  "run anytime": "runanytime",
  "right.codes": "right.codes",
  rightcodes: "right.codes",
  right: "right.codes",
  xychatai: "xychatai",
};

/**
 * API Base URL 到 Relay-Pulse 供应商名称的映射
 * 用于从用户配置中提取的 Base URL 反向查找供应商
 */
export const URL_TO_PROVIDER_MAPPING: Record<string, string> = {
  // 88code
  "88code.ai": "88code",

  // AICodeMirror
  "aicodemirror.com": "aicodemirror",
  "api.aicodemirror.com": "aicodemirror",

  // AIMZ
  "mzlone.top": "aimz",

  // Anyrouter
  "anyrouter.top": "anyrouter",

  // CCFly
  "ccfly.online": "ccfly",
  "apic.cikew.site": "ccfly",

  // ClaudeCN
  "claudecn.top": "claudecn",

  // ClaudePro
  "pro.aipor.cc": "claudepro",

  // CodeCli
  "code-cli.cn": "codecli",

  // DMXAPI
  "dmxapi.cn": "dmxapi",

  // AiHubMix
  "aihubmix.com": "aihubmix",
  "api.aihubmix.com": "aihubmix",

  // EasyChat
  "easychat.site": "easychat",
  "server.easychat.site": "easychat",

  // FastCode
  "api.timebackward.com": "fastcode",

  // HuggingCode
  "huggingcode.cc": "huggingcode",

  // JikeAI
  "magic666.top": "jikeai",

  // LinkAPI
  "linkapi.ai": "linkapi",
  "api.linkapi.ai": "linkapi",

  // MikuCode
  "mikucode.xyz": "mikucode",

  // OCC
  "openclaudecode.cn": "occ",

  // SSSAiCode
  "sssaicode.com": "sssaicode",
  "claude3.sssaicode.com": "sssaicode",
  "codex2.sssaicode.com": "sssaicode",

  // 88code
  "api.88code.com": "88code",
  "88code.com": "88code",
  "88code.org": "88code",

  // DuckCoding
  "api.duckcoding.com": "duckcoding",
  "duckcoding.com": "duckcoding",

  // Privnode
  "api.privnode.com": "privnode",
  "privnode.com": "privnode",

  // PackyCode
  "api.packyapi.com": "packycode",
  "packyapi.com": "packycode",
  "api-slb.packyapi.com": "packycode",

  // FoxCode
  "api.foxcode.io": "foxcode",
  "foxcode.io": "foxcode",
  "foxcode.rjj.cc": "foxcode",
  "code.newcli.com": "foxcode",

  // GalaxyCode
  "api.galaxycode.io": "galaxycode",
  "galaxycode.io": "galaxycode",
  "nf.video": "galaxycode",
  "relay.nf.video": "galaxycode",

  // YesCode
  "api.yescode.io": "yescode",
  "yescode.io": "yescode",
  "co.yes.vg": "yescode",

  // Augmunt
  "api.augmunt.com": "augmunt",
  "augmunt.com": "augmunt",

  // ikuncode
  "api.ikuncode.com": "ikuncode",
  "ikuncode.com": "ikuncode",
  "api.ikuncode.cc": "ikuncode",
  "ikuncode.cc": "ikuncode",

  // uucode
  "api.uucode.io": "uucode",
  "uucode.io": "uucode",
  "api.uucode.org": "uucode",
  "uucode.org": "uucode",

  // xyai
  "api.xyai.io": "xyai",
  "xyai.io": "xyai",
  "new.xychatai.com": "xyai",

  // cubence
  "api.cubence.com": "cubence",
  "cubence.com": "cubence",
  "api-dmit.cubence.com": "cubence",

  // right.codes
  "api.right.codes": "right.codes",
  "right.codes": "right.codes",

  // xychatai
  "api.xychatai.com": "xychatai",
  "xychatai.com": "xychatai",

  // duckcoding
  "jp.duckcoding.com": "duckcoding",
};

/**
 * 从供应商名称获取 Relay-Pulse 映射名称
 * @param providerName CC-Switch 供应商名称
 * @returns Relay-Pulse 供应商名称，未找到返回 undefined
 */
export function getRelayPulseProvider(
  providerName: string,
): string | undefined {
  const normalized = providerName.toLowerCase().trim();
  return PROVIDER_NAME_MAPPING[normalized];
}

/**
 * 从 URL 提取域名并获取 Relay-Pulse 映射名称
 * @param url API Base URL
 * @returns Relay-Pulse 供应商名称，未找到返回 undefined
 */
export function getRelayPulseProviderFromUrl(url: string): string | undefined {
  try {
    const normalizedUrl = url.trim();
    if (!normalizedUrl) return undefined;

    const urlObj = normalizedUrl.includes("://")
      ? new URL(normalizedUrl)
      : new URL(`https://${normalizedUrl}`);
    const host = urlObj.hostname.toLowerCase();

    // 直接匹配
    if (URL_TO_PROVIDER_MAPPING[host]) {
      return URL_TO_PROVIDER_MAPPING[host];
    }

    // 尝试去掉 www. 前缀
    const withoutWww = host.replace(/^www\./, "");
    if (URL_TO_PROVIDER_MAPPING[withoutWww]) {
      return URL_TO_PROVIDER_MAPPING[withoutWww];
    }

    // 尝试去掉 api. 前缀
    const withoutApi = host.replace(/^api\./, "");
    if (URL_TO_PROVIDER_MAPPING[withoutApi]) {
      return URL_TO_PROVIDER_MAPPING[withoutApi];
    }

    return undefined;
  } catch {
    return undefined;
  }
}

/**
 * 从 Provider 对象提取 Relay-Pulse 映射名称
 * 优先使用名称匹配，其次使用 URL 匹配
 * @param provider CC-Switch Provider 对象
 * @returns Relay-Pulse 供应商名称，未找到返回 undefined
 */
export function getRelayPulseProviderFromProvider(
  provider: Provider,
): string | undefined {
  // 1. 首先尝试名称匹配
  const byName = getRelayPulseProvider(provider.name);
  if (byName) return byName;

  // 2. 尝试从 settingsConfig 中提取 Base URL
  const config = provider.settingsConfig;

  // Claude 配置：env.ANTHROPIC_BASE_URL
  if (config?.env?.ANTHROPIC_BASE_URL) {
    const byUrl = getRelayPulseProviderFromUrl(config.env.ANTHROPIC_BASE_URL);
    if (byUrl) return byUrl;
  }

  // Codex 配置：从 TOML config 中提取 base_url
  if (typeof config?.config === "string") {
    const match = config.config.match(/base_url\s*=\s*["']([^"']+)["']/);
    if (match?.[1]) {
      const byUrl = getRelayPulseProviderFromUrl(match[1]);
      if (byUrl) return byUrl;
    }
  }

  // Gemini 配置：env.GOOGLE_GEMINI_BASE_URL
  if (config?.env?.GOOGLE_GEMINI_BASE_URL) {
    const byUrl = getRelayPulseProviderFromUrl(
      config.env.GOOGLE_GEMINI_BASE_URL,
    );
    if (byUrl) return byUrl;
  }

  // OpenCode 配置：options.baseURL
  if (typeof config?.options?.baseURL === "string") {
    const byUrl = getRelayPulseProviderFromUrl(config.options.baseURL);
    if (byUrl) return byUrl;
  }

  // 3. 尝试从 websiteUrl 匹配
  if (provider.websiteUrl) {
    const byWebsite = getRelayPulseProviderFromUrl(provider.websiteUrl);
    if (byWebsite) return byWebsite;
  }

  return undefined;
}

/**
 * 检查供应商是否被 Relay-Pulse 监控
 * @param provider CC-Switch Provider 对象
 * @returns 是否被监控
 */
export function isProviderMonitored(provider: Provider): boolean {
  return getRelayPulseProviderFromProvider(provider) !== undefined;
}
