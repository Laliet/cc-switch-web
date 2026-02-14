import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

type HttpMethod = "GET" | "POST" | "PUT" | "DELETE" | "HEAD";
type CommandArgs = Record<string, unknown>;

interface Endpoint {
  url: string;
  method: HttpMethod;
  body?: unknown;
}

const DEFAULT_WEB_API_BASE = "/api";

// Storage keys - exported for use across modules
export const WEB_AUTH_STORAGE_KEY = "cc-switch-web-auth";
export const WEB_CSRF_STORAGE_KEY = "cc-switch-csrf-token";
export const WEB_API_BASE_STORAGE_KEY = "cc-switch-web-api-base";

const WEB_UNSUPPORTED_COMMANDS: Record<string, string> = {
  test_api_endpoints: "Web 端暂不支持端点测速，请使用桌面版。",
  get_custom_endpoints: "Web 端暂不支持获取 VSCode 自定义端点，请使用桌面版。",
  add_custom_endpoint: "Web 端暂不支持添加 VSCode 自定义端点，请使用桌面版。",
  remove_custom_endpoint: "Web 端暂不支持删除 VSCode 自定义端点，请使用桌面版。",
  update_endpoint_last_used:
    "Web 端暂不支持记录端点使用情况，请使用桌面版。",
  parse_deeplink: "Web 端暂不支持 Deeplink 解析，请使用桌面版。",
  import_from_deeplink: "Web 端暂不支持 Deeplink 导入，请使用桌面版。",
};

const webUnsupportedNotices = new Set<string>();

const notifyWebUnsupported = (cmd: string, message: string): never => {
  if (typeof window !== "undefined") {
    if (!webUnsupportedNotices.has(cmd)) {
      webUnsupportedNotices.add(cmd);
      try {
        toast.error(message);
      } catch {
        console.warn(`cc-switch: ${message}`);
      }
    } else {
      console.warn(`cc-switch: ${message}`);
    }
  }
  throw new Error(message);
};

const getEnvNumber = (value: unknown, fallback: number) => {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
};
const WEB_FETCH_TIMEOUT_MS = Math.max(
  0,
  getEnvNumber(import.meta.env?.VITE_WEB_FETCH_TIMEOUT_MS, 180_000),
);
const WEB_FETCH_MAX_RETRIES = Math.max(
  0,
  Math.floor(getEnvNumber(import.meta.env?.VITE_WEB_FETCH_RETRIES, 1)),
);
const WEB_FETCH_RETRY_DELAY_MS = Math.max(
  0,
  getEnvNumber(import.meta.env?.VITE_WEB_FETCH_RETRY_DELAY_MS, 500),
);

export function normalizeWebApiBase(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  if (trimmed === "/") return "/";
  const normalized = trimmed.replace(/\/+$/, "");
  if (!normalized) return null;
  return normalized;
}

const isRelativeWebApiBase = (value: string): boolean =>
  value.startsWith("/") && !value.startsWith("//");

const parseHttpUrl = (value: string): URL | null => {
  try {
    const parsed = new URL(value);
    if (
      (parsed.protocol === "http:" || parsed.protocol === "https:") &&
      !parsed.username &&
      !parsed.password
    ) {
      return parsed;
    }
  } catch {
    return null;
  }
  return null;
};

const parseIpv4Address = (value: string): number[] | null => {
  if (!/^\d{1,3}(\.\d{1,3}){3}$/.test(value)) return null;
  const parts = value.split(".").map((part) => Number(part));
  if (parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) {
    return null;
  }
  return parts;
};

const isPrivateIpv4Address = (hostname: string): boolean => {
  const parts = parseIpv4Address(hostname);
  if (!parts) return false;
  const [first, second] = parts;
  if (first === 10) return true;
  if (first === 127) return true;
  if (first === 169 && second === 254) return true;
  if (first === 172 && second >= 16 && second <= 31) return true;
  if (first === 192 && second === 168) return true;
  return false;
};

const getIpv6FirstHextet = (hostname: string): number | null => {
  if (!hostname.includes(":")) return null;
  const [first] = hostname.split(":");
  if (first === "") return 0;
  if (!/^[0-9a-f]{1,4}$/.test(first)) return null;
  return parseInt(first, 16);
};

const isPrivateIpv6Address = (hostname: string): boolean => {
  if (!hostname.includes(":")) return false;
  if (hostname === "::1" || hostname === "0:0:0:0:0:0:0:1") return true;
  const firstHextet = getIpv6FirstHextet(hostname);
  if (firstHextet === null) return false;
  if (firstHextet >= 0xfc00 && firstHextet <= 0xfdff) return true;
  if (firstHextet >= 0xfe80 && firstHextet <= 0xfebf) return true;
  return false;
};

const isPrivateHostname = (hostname: string): boolean => {
  const normalized = hostname.toLowerCase();
  if (normalized === "localhost") return true;
  if (isPrivateIpv4Address(normalized)) return true;
  if (isPrivateIpv6Address(normalized)) return true;
  return false;
};

const isPrivateWebApiOrigin = (origin: string): boolean => {
  const parsed = parseHttpUrl(origin);
  if (!parsed) return false;
  return isPrivateHostname(parsed.hostname);
};

const WEB_API_ORIGIN_BLOCKED_MESSAGE =
  "API 地址不在允许列表，请设置 CORS_ALLOW_ORIGINS 或启用 ALLOW_LAN_CORS（局域网自动放行）";

export function resolveWebOrigin(url: string): string | null {
  if (typeof window === "undefined") return null;
  try {
    const parsed = new URL(url, window.location.origin);
    if (parsed.username || parsed.password) return null;
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return null;
    }
    return parsed.origin;
  } catch {
    return null;
  }
}

const getAllowedWebApiOrigins = (): Set<string> => {
  const origins = new Set<string>();
  if (typeof window !== "undefined" && window.location?.origin) {
    origins.add(window.location.origin);
  }
  const allowedOrigins = import.meta.env?.VITE_WEB_API_ALLOWED_ORIGINS;
  if (typeof allowedOrigins === "string" && allowedOrigins.trim()) {
    for (const entry of allowedOrigins.split(",")) {
      const trimmed = entry.trim();
      if (!trimmed) continue;
      const origin = resolveWebOrigin(trimmed);
      if (origin) origins.add(origin);
    }
  }
  return origins;
};

const isAllowedWebApiOrigin = (origin: string): boolean => {
  if (typeof window === "undefined") return true;
  if (getAllowedWebApiOrigins().has(origin)) return true;
  const currentOrigin = window.location?.origin;
  if (!currentOrigin) return false;
  return isPrivateWebApiOrigin(currentOrigin) && isPrivateWebApiOrigin(origin);
};

const getWebApiBaseProtocolError = (value: string): string | null => {
  if (typeof window === "undefined") return null;
  if (window.location?.protocol !== "https:") return null;
  const parsed = parseHttpUrl(value);
  if (parsed?.protocol === "http:") {
    return "当前页面为 HTTPS，API 地址必须使用 https 或相对路径";
  }
  return null;
};

export function getWebApiBaseValidationError(value: string): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  const normalized = normalizeWebApiBase(trimmed);
  if (!normalized) return "API 地址无效";
  if (isRelativeWebApiBase(normalized)) return null;
  const parsed = parseHttpUrl(normalized);
  if (!parsed) return "API 地址无效";
  const protocolError = getWebApiBaseProtocolError(normalized);
  if (protocolError) return protocolError;
  if (!isAllowedWebApiOrigin(parsed.origin)) {
    return WEB_API_ORIGIN_BLOCKED_MESSAGE;
  }
  return null;
}

export function isValidWebApiBase(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed) return false;
  if (isRelativeWebApiBase(trimmed)) return true;
  const parsed = parseHttpUrl(trimmed);
  if (!parsed) return false;
  if (getWebApiBaseProtocolError(trimmed) !== null) return false;
  return isAllowedWebApiOrigin(parsed.origin);
}

const resolveWebApiBase = (value: unknown): string | null => {
  const normalized = normalizeWebApiBase(value);
  if (!normalized) return null;
  if (!isValidWebApiBase(normalized)) return null;
  return normalized;
};

export function getWebApiBase(): string {
  const stored = getStoredWebApiBase();
  if (stored) return stored;
  if (typeof window !== "undefined") {
    const fromWindow = resolveWebApiBase(window.__CC_SWITCH_API_BASE__);
    if (fromWindow) return fromWindow;
  }
  const fromEnv = resolveWebApiBase(import.meta.env?.VITE_WEB_API_BASE);
  if (fromEnv) return fromEnv;
  return DEFAULT_WEB_API_BASE;
}

export function buildWebApiUrlWithBase(base: string, path: string): string {
  const trimmedPath = path.trim();
  if (!trimmedPath) return base;
  const normalizedBase = base.replace(/\/+$/, "");
  const normalizedPath = trimmedPath.startsWith("/")
    ? trimmedPath
    : `/${trimmedPath}`;
  if (!normalizedBase) return normalizedPath;
  return `${normalizedBase}${normalizedPath}`;
}

export function buildWebApiUrl(path: string): string {
  return buildWebApiUrlWithBase(getWebApiBase(), path);
}

const encode = (value: unknown) => encodeURIComponent(String(value));

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isAllowedExternalUrl = (value: string): boolean => {
  const trimmed = value.trim();
  if (!trimmed) return false;
  try {
    const base =
      typeof window !== "undefined" ? window.location.origin : undefined;
    const parsed = new URL(trimmed, base);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
};

const requireArg = <T = unknown>(args: unknown, key: string, cmd: string): T => {
  if (!isRecord(args)) {
    throw new Error(
      `Missing argument "${key}" for command "${cmd}" in web mode`,
    );
  }
  const value = args[key];
  if (value === undefined || value === null) {
    throw new Error(
      `Missing argument "${key}" for command "${cmd}" in web mode`,
    );
  }
  return value as T;
};

export function isWeb(): boolean {
  if (import.meta.env?.VITE_MODE === "web") {
    return true;
  }
  if (typeof window === "undefined") {
    return true;
  }

  const tauriGlobal =
    (window as any).__TAURI__ || (window as any).__TAURI_INTERNALS__;
  return !tauriGlobal;
}

declare global {
  interface Window {
    __CC_SWITCH_TOKENS__?: {
      csrfToken: string;
      __noticeShown?: boolean;
    };
    __CC_SWITCH_API_BASE__?: string;
  }
}

function getAutoTokens() {
  if (typeof window === "undefined") return undefined;
  const tokens = window.__CC_SWITCH_TOKENS__;
  if (tokens?.csrfToken) {
    if (!tokens.__noticeShown) {
      console.info("cc-switch: 已自动应用内置 CSRF Token");
      tokens.__noticeShown = true;
    }
    return { csrfToken: tokens.csrfToken };
  }
  return undefined;
}

async function fetchWithTimeout(
  url: string,
  init: RequestInit,
  timeoutMs: number,
): Promise<Response> {
  if (timeoutMs <= 0) {
    return fetch(url, init);
  }

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  try {
    return await fetch(url, { ...init, signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
}

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const getErrorMessage = (payload: unknown): string => {
  if (!payload) return "";
  if (typeof payload === "string") {
    return payload;
  }
  if (typeof payload === "object") {
    const obj = payload as Record<string, unknown>;
    const candidate = obj.message ?? obj.error ?? obj.detail;
    if (typeof candidate === "string" && candidate.trim()) {
      return candidate;
    }
    const nested = obj.payload;
    if (typeof nested === "string" && nested.trim()) {
      return nested;
    }
    if (nested && typeof nested === "object") {
      const nestedObj = nested as Record<string, unknown>;
      const nestedCandidate =
        nestedObj.message ?? nestedObj.error ?? nestedObj.detail;
      if (typeof nestedCandidate === "string" && nestedCandidate.trim()) {
        return nestedCandidate;
      }
    }
  }
  return "";
};

/**
 * Base64 encode a UTF-8 string, with fallbacks for different environments.
 * Exported for reuse across modules.
 */
export function base64EncodeUtf8(value: string): string {
  if (typeof window !== "undefined" && typeof window.btoa === "function") {
    const bytes = new TextEncoder().encode(value);
    let binary = "";
    for (const byte of bytes) {
      binary += String.fromCharCode(byte);
    }
    return window.btoa(binary);
  }

  if (typeof Buffer !== "undefined") {
    return Buffer.from(value, "utf8").toString("base64");
  }

  throw new Error("Base64 encoder is not available");
}

interface StoredWebCredentialsPayload {
  token: string;
  apiBase: string | null;
  legacy: boolean;
}

const parseStoredWebCredentialsValue = (
  value: string,
): StoredWebCredentialsPayload | null => {
  const trimmed = value.trim();
  if (!trimmed) return null;
  if (trimmed.startsWith("{")) {
    try {
      const parsed = JSON.parse(trimmed);
      if (isRecord(parsed) && typeof parsed.token === "string") {
        const token = parsed.token.trim();
        if (!token) return null;
        const apiBase =
          typeof parsed.apiBase === "string" ? parsed.apiBase : null;
        return { token, apiBase, legacy: false };
      }
      return null;
    } catch {
      return null;
    }
  }
  return { token: trimmed, apiBase: null, legacy: true };
};

const isSameWebOrigin = (origin: string): boolean =>
  typeof window !== "undefined" && window.location?.origin === origin;

function getStoredWebCredentials(targetUrl?: string): string | undefined {
  if (typeof window === "undefined") return undefined;
  try {
    const value = window.sessionStorage?.getItem(WEB_AUTH_STORAGE_KEY);
    if (!value) return undefined;
    const parsed = parseStoredWebCredentialsValue(value);
    if (!parsed) return undefined;
    const targetOrigin =
      typeof targetUrl === "string" && targetUrl.trim()
        ? resolveWebOrigin(targetUrl)
        : window.location?.origin;
    if (!targetOrigin) return undefined;
    if (!isAllowedWebApiOrigin(targetOrigin)) return undefined;
    const sameOrigin = isSameWebOrigin(targetOrigin);
    if (parsed.legacy) {
      return sameOrigin ? parsed.token : undefined;
    }
    const normalizedApiBase = normalizeWebApiBase(parsed.apiBase);
    if (normalizedApiBase && !isValidWebApiBase(normalizedApiBase)) {
      return undefined;
    }
    if (!normalizedApiBase || isRelativeWebApiBase(normalizedApiBase)) {
      return sameOrigin ? parsed.token : undefined;
    }
    const storedOrigin = resolveWebOrigin(normalizedApiBase);
    if (!storedOrigin) return undefined;
    return storedOrigin === targetOrigin ? parsed.token : undefined;
  } catch {
    return undefined;
  }
}

export function getStoredWebApiBase(): string | undefined {
  if (typeof window === "undefined") return undefined;
  try {
    const value = window.localStorage?.getItem(WEB_API_BASE_STORAGE_KEY);
    if (!value) return undefined;
    const resolved = resolveWebApiBase(value);
    if (!resolved) {
      window.localStorage?.removeItem(WEB_API_BASE_STORAGE_KEY);
      return undefined;
    }
    return resolved;
  } catch {
    return undefined;
  }
}

function getStoredWebCsrfToken(): string | undefined {
  if (typeof window === "undefined") return undefined;
  try {
    const value = window.sessionStorage?.getItem(WEB_CSRF_STORAGE_KEY);
    if (!value) return undefined;
    return value;
  } catch {
    return undefined;
  }
}

export function buildWebAuthHeadersForUrl(url: string): Record<string, string> {
  if (typeof window === "undefined") return {};
  const origin = resolveWebOrigin(url);
  if (!origin) {
    throw new Error("API 地址无效");
  }
  if (!isAllowedWebApiOrigin(origin)) {
    throw new Error(WEB_API_ORIGIN_BLOCKED_MESSAGE);
  }
  const headers: Record<string, string> = {};
  const tokens = getAutoTokens();
  const csrfToken = tokens?.csrfToken ?? getStoredWebCsrfToken();
  if (csrfToken) headers["X-CSRF-Token"] = csrfToken;
  const storedAuth = getStoredWebCredentials(url);
  if (storedAuth) {
    headers.Authorization = `Basic ${storedAuth}`;
  }
  return headers;
}

export function setWebCredentials(password: string, apiBase?: string | null) {
  if (typeof window === "undefined") return;
  const encoded = base64EncodeUtf8(`admin:${password}`);
  const normalizedApiBase = normalizeWebApiBase(apiBase);
  const storedApiBase =
    normalizedApiBase && isValidWebApiBase(normalizedApiBase)
      ? normalizedApiBase
      : null;
  const payload = JSON.stringify({
    token: encoded,
    apiBase: storedApiBase,
  });
  try {
    window.sessionStorage?.setItem(WEB_AUTH_STORAGE_KEY, payload);
  } catch {
    // ignore
  }
}

export function setWebApiBaseOverride(value: string | null) {
  if (typeof window === "undefined") return;
  try {
    const normalized = normalizeWebApiBase(value);
    if (!normalized) {
      clearWebApiBaseOverride();
      return;
    }
    if (!isValidWebApiBase(normalized)) return;
    window.localStorage?.setItem(WEB_API_BASE_STORAGE_KEY, normalized);
  } catch {
    // ignore
  }
}

export function clearWebApiBaseOverride() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage?.removeItem(WEB_API_BASE_STORAGE_KEY);
  } catch {
    // ignore
  }
}

export function clearWebCredentials() {
  if (typeof window === "undefined") return;
  try {
    window.sessionStorage?.removeItem(WEB_AUTH_STORAGE_KEY);
    window.sessionStorage?.removeItem(WEB_CSRF_STORAGE_KEY);
  } catch {
    // ignore
  }
}

export function commandToEndpoint(
  cmd: string,
  args: CommandArgs = {},
): Endpoint {
  const apiBase = getWebApiBase();
  switch (cmd) {
    // Provider commands
    case "get_providers": {
      const app = requireArg(args, "app", cmd);
      return { method: "GET", url: `${apiBase}/providers/${encode(app)}` };
    }
    case "get_current_provider": {
      const app = requireArg(args, "app", cmd);
      return {
        method: "GET",
        url: `${apiBase}/providers/${encode(app)}/current`,
      };
    }
    case "get_backup_provider": {
      const app = requireArg(args, "app", cmd);
      return {
        method: "GET",
        url: `${apiBase}/providers/${encode(app)}/backup`,
      };
    }
    case "set_backup_provider": {
      const app = requireArg(args, "app", cmd);
      return {
        method: "PUT",
        url: `${apiBase}/providers/${encode(app)}/backup`,
        body: { id: args.id ?? null },
      };
    }
    case "add_provider": {
      const app = requireArg(args, "app", cmd);
      const provider = requireArg(args, "provider", cmd);
      return {
        method: "POST",
        url: `${apiBase}/providers/${encode(app)}`,
        body: provider,
      };
    }
    case "update_provider": {
      const app = requireArg(args, "app", cmd);
      const provider = requireArg<Record<string, unknown>>(
        args,
        "provider",
        cmd,
      );
      const providerId =
        (provider.id ?? provider.providerId ?? args.id) as
          | string
          | number
          | null
          | undefined;
      if (!providerId) {
        throw new Error(`Missing provider id for command "${cmd}" in web mode`);
      }
      return {
        method: "PUT",
        url: `${apiBase}/providers/${encode(app)}/${encode(providerId)}`,
        body: provider,
      };
    }
    case "delete_provider": {
      const app = requireArg(args, "app", cmd);
      const id = requireArg(args, "id", cmd);
      return {
        method: "DELETE",
        url: `${apiBase}/providers/${encode(app)}/${encode(id)}`,
      };
    }
    case "switch_provider": {
      const app = requireArg(args, "app", cmd);
      const id = requireArg(args, "id", cmd);
      return {
        method: "POST",
        url: `${apiBase}/providers/${encode(app)}/${encode(id)}/switch`,
      };
    }
    case "import_default_config": {
      const app = requireArg(args, "app", cmd);
      return {
        method: "POST",
        url: `${apiBase}/providers/${encode(app)}/import-default`,
      };
    }
    case "read_live_provider_settings": {
      const app = requireArg(args, "app", cmd);
      return {
        method: "GET",
        url: `${apiBase}/providers/${encode(app)}/live-settings`,
      };
    }
    case "update_tray_menu": {
      return { method: "POST", url: `${apiBase}/tray/update` };
    }
    case "update_providers_sort_order": {
      const app = requireArg(args, "app", cmd);
      const updates = requireArg(args, "updates", cmd);
      return {
        method: "PUT",
        url: `${apiBase}/providers/${encode(app)}/sort-order`,
        body: { updates },
      };
    }
    case "queryProviderUsage": {
      const app = requireArg(args, "app", cmd);
      const providerId = requireArg(args, "providerId", cmd);
      return {
        method: "POST",
        url: `${apiBase}/providers/${encode(app)}/${encode(providerId)}/usage`,
      };
    }
    case "testUsageScript": {
      const app = requireArg(args, "app", cmd);
      const providerId = requireArg(args, "providerId", cmd);
      return {
        method: "POST",
        url: `${apiBase}/providers/${encode(app)}/${encode(providerId)}/usage/test`,
        body: {
          scriptCode: requireArg(args, "scriptCode", cmd),
          timeout: args.timeout,
          apiKey: args.apiKey,
          baseUrl: args.baseUrl,
          accessToken: args.accessToken,
          userId: args.userId,
        },
      };
    }

    // MCP commands
    case "get_claude_mcp_status":
      return { method: "GET", url: `${apiBase}/mcp/status` };
    case "read_claude_mcp_config":
      return { method: "GET", url: `${apiBase}/mcp/config/claude` };
    case "upsert_claude_mcp_server": {
      const id = requireArg(args, "id", cmd);
      const spec = requireArg(args, "spec", cmd);
      return {
        method: "PUT",
        url: `${apiBase}/mcp/config/claude/servers/${encode(id)}`,
        body: { spec },
      };
    }
    case "delete_claude_mcp_server": {
      const id = requireArg(args, "id", cmd);
      return {
        method: "DELETE",
        url: `${apiBase}/mcp/config/claude/servers/${encode(id)}`,
      };
    }
    case "validate_mcp_command":
      return {
        method: "POST",
        url: `${apiBase}/mcp/validate`,
        body: { cmd: requireArg(args, "cmd", cmd) },
      };
    case "get_mcp_config": {
      const app = requireArg(args, "app", cmd);
      return { method: "GET", url: `${apiBase}/mcp/config/${encode(app)}` };
    }
    case "upsert_mcp_server_in_config": {
      const app = requireArg(args, "app", cmd);
      const id = requireArg(args, "id", cmd);
      const spec = requireArg(args, "spec", cmd);
      return {
        method: "PUT",
        url: `${apiBase}/mcp/config/${encode(app)}/servers/${encode(id)}`,
        body: {
          spec,
          ...(args.syncOtherSide !== undefined
            ? { syncOtherSide: args.syncOtherSide }
            : {}),
        },
      };
    }
    case "delete_mcp_server_in_config": {
      const app = requireArg(args, "app", cmd);
      const id = requireArg(args, "id", cmd);
      return {
        method: "DELETE",
        url: `${apiBase}/mcp/config/${encode(app)}/servers/${encode(id)}`,
        body:
          args.syncOtherSide !== undefined
            ? { syncOtherSide: args.syncOtherSide }
            : undefined,
      };
    }
    case "set_mcp_enabled": {
      const app = requireArg(args, "app", cmd);
      const id = requireArg(args, "id", cmd);
      const enabled = requireArg(args, "enabled", cmd);
      return {
        method: "POST",
        url: `${apiBase}/mcp/config/${encode(app)}/servers/${encode(id)}/enabled`,
        body: { enabled },
      };
    }
    case "get_mcp_servers":
      return { method: "GET", url: `${apiBase}/mcp/servers` };
    case "upsert_mcp_server": {
      const server = requireArg(args, "server", cmd);
      const id = requireArg(server, "id", cmd);
      return {
        method: "PUT",
        url: `${apiBase}/mcp/servers/${encode(id)}`,
        body: server,
      };
    }
    case "delete_mcp_server": {
      const id = requireArg(args, "id", cmd);
      return {
        method: "DELETE",
        url: `${apiBase}/mcp/servers/${encode(id)}`,
      };
    }
    case "toggle_mcp_app": {
      const serverId = requireArg(args, "serverId", cmd);
      const app = requireArg(args, "app", cmd);
      const enabled = requireArg(args, "enabled", cmd);
      return {
        method: "POST",
        url: `${apiBase}/mcp/servers/${encode(serverId)}/apps/${encode(app)}`,
        body: { enabled },
      };
    }

    // Prompt commands
    case "get_prompts": {
      const app = requireArg(args, "app", cmd);
      return { method: "GET", url: `${apiBase}/prompts/${encode(app)}` };
    }
    case "upsert_prompt": {
      const app = requireArg(args, "app", cmd);
      const id = requireArg(args, "id", cmd);
      const prompt = requireArg(args, "prompt", cmd);
      return {
        method: "PUT",
        url: `${apiBase}/prompts/${encode(app)}/${encode(id)}`,
        body: prompt,
      };
    }
    case "delete_prompt": {
      const app = requireArg(args, "app", cmd);
      const id = requireArg(args, "id", cmd);
      return {
        method: "DELETE",
        url: `${apiBase}/prompts/${encode(app)}/${encode(id)}`,
      };
    }
    case "enable_prompt": {
      const app = requireArg(args, "app", cmd);
      const id = requireArg(args, "id", cmd);
      return {
        method: "POST",
        url: `${apiBase}/prompts/${encode(app)}/${encode(id)}/enable`,
      };
    }
    case "import_prompt_from_file": {
      const app = requireArg(args, "app", cmd);
      return {
        method: "POST",
        url: `${apiBase}/prompts/${encode(app)}/import-from-file`,
      };
    }
    case "get_current_prompt_file_content": {
      const app = requireArg(args, "app", cmd);
      return {
        method: "GET",
        url: `${apiBase}/prompts/${encode(app)}/current-file`,
      };
    }

    // Skill commands
    case "get_skills": {
      const app = typeof args.app === "string" ? args.app : undefined;
      return {
        method: "GET",
        url: app
          ? `${apiBase}/skills?app=${encode(app)}`
          : `${apiBase}/skills`,
      };
    }
    case "install_skill":
      return {
        method: "POST",
        url: `${apiBase}/skills/install`,
        body: (() => {
          const directory = requireArg<string>(args, "directory", cmd);
          const payload: { directory: string; force?: boolean; app?: string } =
            { directory };
          if (typeof args.force === "boolean") {
            payload.force = args.force;
          }
          if (typeof args.app === "string") {
            payload.app = args.app;
          }
          return payload;
        })(),
      };
    case "uninstall_skill":
      return {
        method: "POST",
        url: `${apiBase}/skills/uninstall`,
        body: (() => {
          const payload: { directory: string; app?: string } = {
            directory: requireArg(args, "directory", cmd),
          };
          if (typeof args.app === "string") {
            payload.app = args.app;
          }
          return payload;
        })(),
      };
    case "get_skill_repos":
      return { method: "GET", url: `${apiBase}/skills/repos` };
    case "add_skill_repo":
      return {
        method: "POST",
        url: `${apiBase}/skills/repos`,
        body: requireArg(args, "repo", cmd),
      };
    case "remove_skill_repo": {
      const owner = requireArg(args, "owner", cmd);
      const name = requireArg(args, "name", cmd);
      return {
        method: "DELETE",
        url: `${apiBase}/skills/repos/${encode(owner)}/${encode(name)}`,
      };
    }

    // Settings / system commands
    case "get_settings":
      return { method: "GET", url: `${apiBase}/settings` };
    case "save_settings":
      return {
        method: "PUT",
        url: `${apiBase}/settings`,
        body: requireArg(args, "settings", cmd),
      };
    case "restart_app":
      return { method: "POST", url: `${apiBase}/system/restart` };
    case "check_for_updates":
      return { method: "POST", url: `${apiBase}/system/check-updates` };
    case "is_portable_mode":
      return { method: "GET", url: `${apiBase}/system/is-portable` };
    case "get_config_dir": {
      const app = requireArg(args, "app", cmd);
      return { method: "GET", url: `${apiBase}/config/${encode(app)}/dir` };
    }
    case "open_config_folder": {
      const app = requireArg(args, "app", cmd);
      return { method: "POST", url: `${apiBase}/config/${encode(app)}/open` };
    }
    case "pick_directory":
      return {
        method: "POST",
        url: `${apiBase}/fs/pick-directory`,
        body:
          args.defaultPath !== undefined
            ? { defaultPath: args.defaultPath }
            : undefined,
      };
    case "get_claude_code_config_path":
      return { method: "GET", url: `${apiBase}/config/claude-code/path` };
    case "get_app_config_path":
      return { method: "GET", url: `${apiBase}/config/app/path` };
    case "open_app_config_folder":
      return { method: "POST", url: `${apiBase}/config/app/open` };
    case "get_app_config_dir_override":
      return { method: "GET", url: `${apiBase}/config/app/override` };
    case "set_app_config_dir_override":
      return {
        method: "PUT",
        url: `${apiBase}/config/app/override`,
        body: { path: args.path },
      };
    case "apply_claude_plugin_config":
      return {
        method: "POST",
        url: `${apiBase}/config/claude/plugin`,
        body: { official: requireArg(args, "official", cmd) },
      };
    case "save_file_dialog":
      return {
        method: "POST",
        url: `${apiBase}/fs/save-file`,
        body: { defaultName: requireArg(args, "defaultName", cmd) },
      };
    case "open_file_dialog":
      return { method: "POST", url: `${apiBase}/fs/open-file` };
    case "export_config_to_file":
      return {
        method: "POST",
        url: `${apiBase}/config/export`,
        body: { filePath: requireArg(args, "filePath", cmd) },
      };
    case "import_config_from_file": {
      const body: Record<string, string> = {
        filePath: requireArg(args, "filePath", cmd),
      };
      // Web 模式下需要传递文件内容，因为浏览器无法访问服务器文件系统
      if (typeof args.content === "string") {
        body.content = args.content;
      }
      return {
        method: "POST",
        url: `${apiBase}/config/import`,
        body,
      };
    }
    case "sync_current_providers_live":
      return { method: "POST", url: `${apiBase}/providers/sync-current` };
    case "open_external":
      return {
        method: "POST",
        url: `${apiBase}/system/open-external`,
        body: { url: requireArg(args, "url", cmd) },
      };

    // Config snippet commands
    case "get_claude_common_config_snippet":
      return {
        method: "GET",
        url: `${apiBase}/config/claude/common-snippet`,
      };
    case "set_claude_common_config_snippet":
      return {
        method: "PUT",
        url: `${apiBase}/config/claude/common-snippet`,
        body: { snippet: requireArg(args, "snippet", cmd) },
      };
    case "get_common_config_snippet": {
      const appType = requireArg(args, "appType", cmd);
      return {
        method: "GET",
        url: `${apiBase}/config/${encode(appType)}/common-snippet`,
      };
    }
    case "set_common_config_snippet": {
      const appType = requireArg(args, "appType", cmd);
      return {
        method: "PUT",
        url: `${apiBase}/config/${encode(appType)}/common-snippet`,
        body: { snippet: requireArg(args, "snippet", cmd) },
      };
    }

    default:
      throw new Error(`Command ${cmd} is not supported in web mode`);
  }
}

export async function invoke(
  cmd: "check_for_updates",
  args?: CommandArgs,
): Promise<null>;
export async function invoke(
  cmd: "get_env_var",
  args?: CommandArgs,
): Promise<null>;
export async function invoke(
  cmd: "set_env_var",
  args?: CommandArgs,
): Promise<null>;
export async function invoke<T>(
  cmd: string,
  args?: CommandArgs,
): Promise<T>;
export async function invoke<T>(
  cmd: string,
  args: CommandArgs = {},
): Promise<T | null> {
  if (!isWeb()) {
    return tauriInvoke<T>(cmd, args);
  }

  const unsupportedMessage = WEB_UNSUPPORTED_COMMANDS[cmd];
  if (unsupportedMessage) {
    return notifyWebUnsupported(cmd, unsupportedMessage);
  }

  switch (cmd) {
    case "update_tray_menu":
      return true as T;
    case "check_for_updates":
      return null;
    case "restart_app":
      return undefined as T;
    case "is_portable_mode":
      return false as T;
    case "check_env_conflicts":
      return [] as T;
    case "delete_env_vars":
      return {
        backupPath: "",
        timestamp: "",
        conflicts: [],
      } as T;
    case "restore_env_backup":
      return undefined as T;
    case "get_env_var":
    case "set_env_var":
      return null;
    case "open_external": {
      const url = args.url as string | undefined;
      if (typeof window !== "undefined" && typeof url === "string") {
        const trimmed = url.trim();
        if (isAllowedExternalUrl(trimmed)) {
          window.open(trimmed, "_blank", "noopener,noreferrer");
        } else {
          console.warn("cc-switch: blocked unsafe open_external url");
        }
      }
      return true as T;
    }
    default:
      break;
  }

  const endpoint = commandToEndpoint(cmd, args);
  const headers: Record<string, string> = {
    Accept: "application/json",
    ...buildWebAuthHeadersForUrl(endpoint.url),
  };
  const init: RequestInit = {
    method: endpoint.method,
    credentials: "include",
    headers,
  };

  if (endpoint.method !== "GET" && endpoint.body !== undefined) {
    headers["Content-Type"] = "application/json";
    init.body = JSON.stringify(endpoint.body);
  }

  const canRetry = endpoint.method === "GET" || endpoint.method === "HEAD";
  const maxRetries = canRetry ? WEB_FETCH_MAX_RETRIES : 0;

  for (let attempt = 0; attempt <= maxRetries; attempt += 1) {
    try {
      const response = await fetchWithTimeout(
        endpoint.url,
        init,
        WEB_FETCH_TIMEOUT_MS,
      );

      if (!response.ok) {
        const contentType = response.headers.get("content-type") || "";
        const rawText = await response.text();
        let errorPayload: unknown;
        if (contentType.includes("application/json") && rawText.trim()) {
          try {
            errorPayload = JSON.parse(rawText);
          } catch {
            errorPayload = undefined;
          }
        }
        if (errorPayload !== undefined) {
          const message =
            getErrorMessage(errorPayload) ||
            `Request failed with status ${response.status}`;
          const error = new Error(message);
          (error as any).payload = errorPayload;
          (error as any).status = response.status;
          throw error;
        }
        const message = rawText.trim()
          ? rawText
          : `Request failed with status ${response.status}`;
        const error = new Error(message);
        (error as any).status = response.status;
        throw error;
      }

      if (response.status === 204) {
        return undefined as T;
      }

      const contentType = response.headers.get("content-type") || "";
      if (contentType.includes("application/json")) {
        return (await response.json()) as T;
      }

      const text = await response.text();
      return text as unknown as T;
    } catch (error) {
      const errorName = (error as any)?.name;
      const isAbortError = errorName === "AbortError";
      const isNetworkError = error instanceof TypeError;
      const shouldRetry =
        canRetry && attempt < maxRetries && (isAbortError || isNetworkError);

      if (!shouldRetry) {
        throw error;
      }

      if (WEB_FETCH_RETRY_DELAY_MS > 0) {
        await delay(WEB_FETCH_RETRY_DELAY_MS);
      }
    }
  }

  throw new Error("Request failed after retries");
}
