import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const importAdapter = async () => {
  vi.resetModules();
  return import("@/lib/api/adapter");
};

const mockJsonResponse = (payload: unknown, ok = true, status = 200) =>
  ({
    ok,
    status,
    headers: new Headers({ "content-type": "application/json" }),
    json: async () => payload,
    text: async () => JSON.stringify(payload),
  }) as Response;

const mockTextResponse = (text: string, ok = true, status = 200) =>
  ({
    ok,
    status,
    headers: new Headers({ "content-type": "text/plain" }),
    json: async () => ({ text }),
    text: async () => text,
  }) as Response;

let originalTauri: unknown;
let originalTauriInternals: unknown;

beforeEach(() => {
  vi.restoreAllMocks();
  originalTauri = (window as any).__TAURI__;
  originalTauriInternals = (window as any).__TAURI_INTERNALS__;
  delete (window as any).__TAURI__;
  delete (window as any).__TAURI_INTERNALS__;
  delete (window as any).__CC_SWITCH_API_BASE__;
  window.sessionStorage.clear();
  window.localStorage.clear();
});

afterEach(() => {
  (window as any).__TAURI__ = originalTauri;
  (window as any).__TAURI_INTERNALS__ = originalTauriInternals;
  vi.useRealTimers();
});

describe("adapter helpers", () => {
  it("isWeb reflects tauri globals", async () => {
    const { isWeb } = await importAdapter();

    expect(isWeb()).toBe(true);

    (window as any).__TAURI__ = {};
    expect(isWeb()).toBe(false);
  });

  it("base64EncodeUtf8 encodes utf-8 strings", async () => {
    const { base64EncodeUtf8 } = await importAdapter();
    const value = "hello 世界";

    expect(base64EncodeUtf8(value)).toBe(
      Buffer.from(value, "utf8").toString("base64"),
    );
  });

  it("getWebApiBase trims and uses window override", async () => {
    (window as any).__CC_SWITCH_API_BASE__ = " /custom-api/ ";
    const { getWebApiBase } = await importAdapter();

    expect(getWebApiBase()).toBe("/custom-api");
  });

  it("getWebApiBase prefers stored override when valid", async () => {
    const { getWebApiBase, WEB_API_BASE_STORAGE_KEY } = await importAdapter();
    vi.stubGlobal("location", {
      origin: "https://api.example.com",
      protocol: "https:",
    });
    try {
      window.localStorage.setItem(
        WEB_API_BASE_STORAGE_KEY,
        " https://api.example.com/base/ ",
      );
      (window as any).__CC_SWITCH_API_BASE__ = "/custom-api";

      expect(getWebApiBase()).toBe("https://api.example.com/base");
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("getWebApiBase ignores invalid stored override", async () => {
    const { getWebApiBase, WEB_API_BASE_STORAGE_KEY } = await importAdapter();
    window.localStorage.setItem(
      WEB_API_BASE_STORAGE_KEY,
      "javascript:alert(1)",
    );
    (window as any).__CC_SWITCH_API_BASE__ = "/custom-api";

    expect(getWebApiBase()).toBe("/custom-api");
    expect(window.localStorage.getItem(WEB_API_BASE_STORAGE_KEY)).toBeNull();
  });

  it("normalizeWebApiBase trims values and drops trailing slashes", async () => {
    const { normalizeWebApiBase } = await importAdapter();

    expect(normalizeWebApiBase(" https://example.com/api/ ")).toBe(
      "https://example.com/api",
    );
    expect(normalizeWebApiBase(" /api/ ")).toBe("/api");
    expect(normalizeWebApiBase("/")).toBe("/");
    expect(normalizeWebApiBase("   ")).toBeNull();
    expect(normalizeWebApiBase(null)).toBeNull();
  });

  it("getWebApiBaseValidationError rejects invalid schemes and protocol-relative urls", async () => {
    const { getWebApiBaseValidationError } = await importAdapter();

    expect(getWebApiBaseValidationError("ftp://example.com/api")).toBe(
      "API 地址无效",
    );
    expect(getWebApiBaseValidationError("//example.com/api")).toBe(
      "API 地址无效",
    );
  });

  it("getWebApiBaseValidationError blocks http base on https pages", async () => {
    const { getWebApiBaseValidationError } = await importAdapter();
    vi.stubGlobal("location", { protocol: "https:" });

    try {
      expect(getWebApiBaseValidationError("http://example.com/api")).toBe(
        "当前页面为 HTTPS，API 地址必须使用 https 或相对路径",
      );
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("getWebApiBaseValidationError blocks unallowlisted origins", async () => {
    const { getWebApiBaseValidationError } = await importAdapter();
    vi.stubGlobal("location", {
      origin: "https://app.example.com",
      protocol: "https:",
    });

    try {
      expect(getWebApiBaseValidationError("https://api.example.com")).toBe(
        "API 地址不在允许列表，请设置 CORS_ALLOW_ORIGINS 或启用 ALLOW_LAN_CORS（局域网自动放行）",
      );
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("getWebApiBaseValidationError allows private origin pairs", async () => {
    const { getWebApiBaseValidationError } = await importAdapter();
    vi.stubGlobal("location", {
      origin: "http://192.168.1.10:3000",
      protocol: "http:",
    });

    try {
      expect(
        getWebApiBaseValidationError("http://192.168.1.11:3000"),
      ).toBeNull();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("getWebApiBaseValidationError blocks public api from private origin", async () => {
    const { getWebApiBaseValidationError } = await importAdapter();
    vi.stubGlobal("location", {
      origin: "http://192.168.1.10:3000",
      protocol: "http:",
    });

    try {
      expect(getWebApiBaseValidationError("https://api.example.com")).toBe(
        "API 地址不在允许列表，请设置 CORS_ALLOW_ORIGINS 或启用 ALLOW_LAN_CORS（局域网自动放行）",
      );
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("buildWebApiUrl joins paths with stored base", async () => {
    const { buildWebApiUrl, WEB_API_BASE_STORAGE_KEY } = await importAdapter();
    vi.stubGlobal("location", {
      origin: "https://api.example.com",
      protocol: "https:",
    });
    try {
      window.localStorage.setItem(
        WEB_API_BASE_STORAGE_KEY,
        "https://api.example.com/base/",
      );

      expect(buildWebApiUrl("settings")).toBe(
        "https://api.example.com/base/settings",
      );
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("setWebApiBaseOverride and clearWebApiBaseOverride manage stored base", async () => {
    const {
      setWebApiBaseOverride,
      clearWebApiBaseOverride,
      getStoredWebApiBase,
      WEB_API_BASE_STORAGE_KEY,
    } = await importAdapter();

    vi.stubGlobal("location", {
      origin: "https://api.example.com",
      protocol: "https:",
    });
    try {
      setWebApiBaseOverride(" https://api.example.com/base/ ");
      expect(getStoredWebApiBase()).toBe("https://api.example.com/base");
      expect(window.localStorage.getItem(WEB_API_BASE_STORAGE_KEY)).toBe(
        "https://api.example.com/base",
      );
    } finally {
      vi.unstubAllGlobals();
    }

    clearWebApiBaseOverride();
    expect(getStoredWebApiBase()).toBeUndefined();
    expect(window.localStorage.getItem(WEB_API_BASE_STORAGE_KEY)).toBeNull();
  });

  it("setWebCredentials and clearWebCredentials manage session storage", async () => {
    const {
      setWebCredentials,
      clearWebCredentials,
      WEB_AUTH_STORAGE_KEY,
      WEB_CSRF_STORAGE_KEY,
    } = await importAdapter();

    setWebCredentials("alice", "secret", "/api");
    const stored = window.sessionStorage.getItem(WEB_AUTH_STORAGE_KEY);
    expect(stored).not.toBeNull();
    const parsed = JSON.parse(stored as string) as {
      token: string;
      apiBase: string | null;
      username: string;
    };
    expect(parsed).toEqual({
      token: Buffer.from("alice:secret").toString("base64"),
      apiBase: "/api",
      username: "alice",
    });

    window.sessionStorage.setItem(WEB_CSRF_STORAGE_KEY, "csrf");
    clearWebCredentials();

    expect(window.sessionStorage.getItem(WEB_AUTH_STORAGE_KEY)).toBeNull();
    expect(window.sessionStorage.getItem(WEB_CSRF_STORAGE_KEY)).toBeNull();
  });

  it("getStoredWebUsername returns stored username when available", async () => {
    const { setWebCredentials, getStoredWebUsername } = await importAdapter();

    setWebCredentials("alice", "secret", "/api");

    expect(getStoredWebUsername()).toBe("alice");
  });

  it("getStoredWebUsername infers stored remote api base when no target is provided", async () => {
    const { WEB_AUTH_STORAGE_KEY, getStoredWebUsername } =
      await importAdapter();

    vi.stubGlobal("location", {
      origin: "https://api.example.com",
      protocol: "https:",
    });
    try {
      window.sessionStorage.setItem(
        WEB_AUTH_STORAGE_KEY,
        JSON.stringify({
          token: Buffer.from("remote-user:secret").toString("base64"),
          apiBase: "https://api.example.com/api",
          username: "remote-user",
        }),
      );

      expect(getStoredWebUsername()).toBe("remote-user");
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("getStoredWebUsername falls back to admin for legacy tokens", async () => {
    const { WEB_AUTH_STORAGE_KEY, getStoredWebUsername } =
      await importAdapter();

    window.sessionStorage.setItem(
      WEB_AUTH_STORAGE_KEY,
      Buffer.from("admin:secret").toString("base64"),
    );

    expect(getStoredWebUsername()).toBe("admin");
  });

  it("getStoredWebUsername falls back to admin for legacy payloads", async () => {
    const { WEB_AUTH_STORAGE_KEY, getStoredWebUsername } =
      await importAdapter();

    window.sessionStorage.setItem(
      WEB_AUTH_STORAGE_KEY,
      JSON.stringify({
        token: Buffer.from("admin:secret").toString("base64"),
        apiBase: "/api",
      }),
    );

    expect(getStoredWebUsername()).toBe("admin");
  });
});

describe("commandToEndpoint", () => {
  it("maps session manager commands without exposing a terminal endpoint", async () => {
    const { commandToEndpoint } = await importAdapter();
    expect(commandToEndpoint("list_sessions")).toEqual({
      method: "GET",
      url: "/api/sessions",
    });
    expect(
      commandToEndpoint("list_sessions_page", {
        cursor: "100",
        limit: 100,
        providerId: "openclaw",
        query: "migration plan",
        refresh: true,
      }),
    ).toEqual({
      method: "GET",
      url: "/api/sessions/page?cursor=100&limit=100&providerId=openclaw&query=migration+plan&refresh=true",
    });
    expect(
      commandToEndpoint("get_session_messages", {
        providerId: "claude",
        sourcePath: "/server/.claude/projects/session.jsonl",
      }),
    ).toEqual({
      method: "POST",
      url: "/api/sessions/messages",
      body: {
        providerId: "claude",
        sourcePath: "/server/.claude/projects/session.jsonl",
      },
    });
    expect(
      commandToEndpoint("delete_session", {
        providerId: "claude",
        sessionId: "session-1",
        sourcePath: "/server/.claude/projects/session.jsonl",
      }),
    ).toMatchObject({ method: "DELETE", url: "/api/sessions" });
    expect(() => commandToEndpoint("launch_session_terminal")).toThrow(
      "not supported in web mode",
    );
  });
  it("maps commands to endpoints", async () => {
    const { commandToEndpoint } = await importAdapter();

    const cases = [
      {
        cmd: "get_providers",
        args: { app: "claude" },
        expected: { method: "GET", url: "/api/providers/claude" },
      },
      {
        cmd: "get_current_provider",
        args: { app: "codex" },
        expected: { method: "GET", url: "/api/providers/codex/current" },
      },
      {
        cmd: "get_backup_provider",
        args: { app: "gemini" },
        expected: { method: "GET", url: "/api/providers/gemini/backup" },
      },
      {
        cmd: "set_backup_provider",
        args: { app: "claude", id: "backup" },
        expected: {
          method: "PUT",
          url: "/api/providers/claude/backup",
          body: { id: "backup" },
        },
      },
      {
        cmd: "add_provider",
        args: { app: "claude", provider: { name: "Test" } },
        expected: {
          method: "POST",
          url: "/api/providers/claude",
          body: { name: "Test" },
        },
      },
      {
        cmd: "update_provider",
        args: { app: "claude", provider: { providerId: "p-1" } },
        expected: {
          method: "PUT",
          url: "/api/providers/claude/p-1",
          body: { providerId: "p-1" },
        },
      },
      {
        cmd: "delete_provider",
        args: { app: "claude", id: "p-2" },
        expected: {
          method: "DELETE",
          url: "/api/providers/claude/p-2",
        },
      },
      {
        cmd: "switch_provider",
        args: { app: "claude", id: "p-3" },
        expected: {
          method: "POST",
          url: "/api/providers/claude/p-3/switch",
        },
      },
      {
        cmd: "import_default_config",
        args: { app: "claude" },
        expected: {
          method: "POST",
          url: "/api/providers/claude/import-default",
        },
      },
      {
        cmd: "update_tray_menu",
        args: {},
        expected: { method: "POST", url: "/api/tray/update" },
      },
      {
        cmd: "update_providers_sort_order",
        args: { app: "claude", updates: [{ id: "p-1", order: 1 }] },
        expected: {
          method: "PUT",
          url: "/api/providers/claude/sort-order",
          body: { updates: [{ id: "p-1", order: 1 }] },
        },
      },
      {
        cmd: "preview_universal_provider",
        args: { provider: { id: "gateway" } },
        expected: {
          method: "POST",
          url: "/api/providers/universal/preview",
          body: { id: "gateway" },
        },
      },
      {
        cmd: "queryProviderUsage",
        args: { app: "claude", providerId: "p-1" },
        expected: {
          method: "POST",
          url: "/api/providers/claude/p-1/usage",
        },
      },
      {
        cmd: "testUsageScript",
        args: {
          app: "claude",
          providerId: "p-2",
          scriptCode: "return 1;",
          timeout: 10,
          apiKey: "k",
          baseUrl: "https://api.example.com",
          accessToken: "token",
          userId: "user",
          templateType: "balance",
        },
        expected: {
          method: "POST",
          url: "/api/providers/claude/p-2/usage/test",
          body: {
            scriptCode: "return 1;",
            timeout: 10,
            apiKey: "k",
            baseUrl: "https://api.example.com",
            accessToken: "token",
            userId: "user",
            templateType: "balance",
          },
        },
      },
      {
        cmd: "get_claude_mcp_status",
        args: {},
        expected: { method: "GET", url: "/api/mcp/status" },
      },
      {
        cmd: "read_claude_mcp_config",
        args: {},
        expected: { method: "GET", url: "/api/mcp/config/claude" },
      },
      {
        cmd: "upsert_claude_mcp_server",
        args: { id: "srv", spec: { command: "cmd" } },
        expected: {
          method: "PUT",
          url: "/api/mcp/config/claude/servers/srv",
          body: { spec: { command: "cmd" } },
        },
      },
      {
        cmd: "delete_claude_mcp_server",
        args: { id: "srv" },
        expected: {
          method: "DELETE",
          url: "/api/mcp/config/claude/servers/srv",
        },
      },
      {
        cmd: "validate_mcp_command",
        args: { cmd: "npx" },
        expected: {
          method: "POST",
          url: "/api/mcp/validate",
          body: { cmd: "npx" },
        },
      },
      {
        cmd: "get_mcp_config",
        args: { app: "claude" },
        expected: { method: "GET", url: "/api/mcp/config/claude" },
      },
      {
        cmd: "upsert_mcp_server_in_config",
        args: {
          app: "codex",
          id: "srv",
          spec: { command: "node" },
          syncOtherSide: true,
        },
        expected: {
          method: "PUT",
          url: "/api/mcp/config/codex/servers/srv",
          body: { spec: { command: "node" }, syncOtherSide: true },
        },
      },
      {
        cmd: "delete_mcp_server_in_config",
        args: { app: "codex", id: "srv", syncOtherSide: false },
        expected: {
          method: "DELETE",
          url: "/api/mcp/config/codex/servers/srv",
          body: { syncOtherSide: false },
        },
      },
      {
        cmd: "set_mcp_enabled",
        args: { app: "codex", id: "srv", enabled: true },
        expected: {
          method: "POST",
          url: "/api/mcp/config/codex/servers/srv/enabled",
          body: { enabled: true },
        },
      },
      {
        cmd: "get_mcp_servers",
        args: {},
        expected: { method: "GET", url: "/api/mcp/servers" },
      },
      {
        cmd: "import_mcp_from_apps",
        args: {},
        expected: {
          method: "POST",
          url: "/api/mcp/servers/import-from-apps",
        },
      },
      {
        cmd: "upsert_mcp_server",
        args: { server: { id: "srv", command: "x" } },
        expected: {
          method: "PUT",
          url: "/api/mcp/servers/srv",
          body: { id: "srv", command: "x" },
        },
      },
      {
        cmd: "delete_mcp_server",
        args: { id: "srv" },
        expected: { method: "DELETE", url: "/api/mcp/servers/srv" },
      },
      {
        cmd: "toggle_mcp_app",
        args: { serverId: "srv", app: "claude", enabled: true },
        expected: {
          method: "POST",
          url: "/api/mcp/servers/srv/apps/claude",
          body: { enabled: true },
        },
      },
      {
        cmd: "get_prompts",
        args: { app: "claude" },
        expected: { method: "GET", url: "/api/prompts/claude" },
      },
      {
        cmd: "upsert_prompt",
        args: { app: "claude", id: "p1", prompt: { name: "n" } },
        expected: {
          method: "PUT",
          url: "/api/prompts/claude/p1",
          body: { name: "n" },
        },
      },
      {
        cmd: "delete_prompt",
        args: { app: "claude", id: "p1" },
        expected: { method: "DELETE", url: "/api/prompts/claude/p1" },
      },
      {
        cmd: "enable_prompt",
        args: { app: "claude", id: "p1" },
        expected: {
          method: "POST",
          url: "/api/prompts/claude/p1/enable",
        },
      },
      {
        cmd: "import_prompt_from_file",
        args: { app: "claude" },
        expected: {
          method: "POST",
          url: "/api/prompts/claude/import-from-file",
        },
      },
      {
        cmd: "get_current_prompt_file_content",
        args: { app: "claude" },
        expected: {
          method: "GET",
          url: "/api/prompts/claude/current-file",
        },
      },
      {
        cmd: "get_skills",
        args: {},
        expected: { method: "GET", url: "/api/skills" },
      },
      {
        cmd: "get_skills",
        args: { app: "codex" },
        expected: { method: "GET", url: "/api/skills?app=codex" },
      },
      {
        cmd: "install_skill",
        args: { directory: "/skills/notes" },
        expected: {
          method: "POST",
          url: "/api/skills/install",
          body: { directory: "/skills/notes" },
        },
      },
      {
        cmd: "install_skill",
        args: { directory: "/skills/notes", force: true },
        expected: {
          method: "POST",
          url: "/api/skills/install",
          body: { directory: "/skills/notes", force: true },
        },
      },
      {
        cmd: "install_skill",
        args: { directory: "/skills/notes", app: "gemini" },
        expected: {
          method: "POST",
          url: "/api/skills/install",
          body: { directory: "/skills/notes", app: "gemini" },
        },
      },
      {
        cmd: "uninstall_skill",
        args: { directory: "/skills/notes" },
        expected: {
          method: "POST",
          url: "/api/skills/uninstall",
          body: { directory: "/skills/notes" },
        },
      },
      {
        cmd: "uninstall_skill",
        args: { directory: "/skills/notes", app: "codex" },
        expected: {
          method: "POST",
          url: "/api/skills/uninstall",
          body: { directory: "/skills/notes", app: "codex" },
        },
      },
      {
        cmd: "scan_unmanaged_skills",
        args: {},
        expected: { method: "GET", url: "/api/skills/discovery" },
      },
      {
        cmd: "import_skills_from_apps",
        args: {
          imports: [
            {
              directory: "demo",
              source: "claude",
              apps: ["claude"],
              overwrite: false,
            },
          ],
        },
        expected: {
          method: "POST",
          url: "/api/skills/discovery/import",
          body: {
            imports: [
              {
                directory: "demo",
                source: "claude",
                apps: ["claude"],
                overwrite: false,
              },
            ],
          },
        },
      },
      {
        cmd: "get_skill_backups",
        args: {},
        expected: { method: "GET", url: "/api/skills/backups" },
      },
      {
        cmd: "restore_skill_backup",
        args: { backupId: "backup-1", app: "claude", force: true },
        expected: {
          method: "POST",
          url: "/api/skills/backups/restore",
          body: { backupId: "backup-1", app: "claude", force: true },
        },
      },
      {
        cmd: "delete_skill_backup",
        args: { backupId: "backup-1" },
        expected: {
          method: "DELETE",
          url: "/api/skills/backups/backup-1",
        },
      },
      {
        cmd: "install_skills_from_zip",
        args: {
          contentBase64: "UEs=",
          fileName: "demo.skill",
          app: "claude",
          force: false,
        },
        expected: {
          method: "POST",
          url: "/api/skills/import-zip",
          body: {
            contentBase64: "UEs=",
            fileName: "demo.skill",
            app: "claude",
            force: false,
          },
        },
      },
      {
        cmd: "migrate_skill_storage",
        args: { target: "unified" },
        expected: {
          method: "POST",
          url: "/api/skills/storage/migrate",
          body: { target: "unified" },
        },
      },
      {
        cmd: "check_skill_updates",
        args: {},
        expected: { method: "GET", url: "/api/skills/updates" },
      },
      {
        cmd: "update_skill",
        args: { id: "owner/repo:demo" },
        expected: {
          method: "POST",
          url: "/api/skills/updates/apply",
          body: { id: "owner/repo:demo" },
        },
      },
      {
        cmd: "search_skills_sh",
        args: { query: "code review", limit: 20, offset: 0 },
        expected: {
          method: "GET",
          url: "/api/skills/catalog/search?query=code%20review&limit=20&offset=0",
        },
      },
      {
        cmd: "install_catalog_skill",
        args: {
          directory: "demo",
          repoOwner: "owner",
          repoName: "repo",
          repoBranch: "main",
          app: "codex",
          force: false,
        },
        expected: {
          method: "POST",
          url: "/api/skills/catalog/install",
          body: {
            directory: "demo",
            repoOwner: "owner",
            repoName: "repo",
            repoBranch: "main",
            app: "codex",
            force: false,
          },
        },
      },
      {
        cmd: "get_skill_repos",
        args: {},
        expected: { method: "GET", url: "/api/skills/repos" },
      },
      {
        cmd: "add_skill_repo",
        args: { repo: { owner: "me", name: "repo" } },
        expected: {
          method: "POST",
          url: "/api/skills/repos",
          body: { owner: "me", name: "repo" },
        },
      },
      {
        cmd: "remove_skill_repo",
        args: { owner: "me", name: "repo" },
        expected: {
          method: "DELETE",
          url: "/api/skills/repos/me/repo",
        },
      },
      {
        cmd: "get_settings",
        args: {},
        expected: { method: "GET", url: "/api/settings" },
      },
      {
        cmd: "save_settings",
        args: { settings: { theme: "dark" } },
        expected: {
          method: "PUT",
          url: "/api/settings",
          body: { theme: "dark" },
        },
      },
      {
        cmd: "merge_deeplink_config",
        args: {
          request: {
            version: "v1",
            resource: "provider",
            app: "claude",
            config: "e30",
          },
        },
        expected: {
          method: "POST",
          url: "/api/deeplink/merge-config",
          body: {
            version: "v1",
            resource: "provider",
            app: "claude",
            config: "e30",
          },
        },
      },
      {
        cmd: "proxy_status",
        args: {},
        expected: { method: "GET", url: "/api/proxy/status" },
      },
      {
        cmd: "proxy_config",
        args: {},
        expected: { method: "GET", url: "/api/proxy/config" },
      },
      {
        cmd: "save_proxy_config",
        args: { settings: { host: "127.0.0.1", port: 3456 } },
        expected: {
          method: "PUT",
          url: "/api/proxy/config",
          body: { settings: { host: "127.0.0.1", port: 3456 } },
        },
      },
      {
        cmd: "save_proxy_settings",
        args: { settings: { host: "127.0.0.1", port: 3456 } },
        expected: {
          method: "PUT",
          url: "/api/proxy/settings",
          body: { settings: { host: "127.0.0.1", port: 3456 } },
        },
      },
      {
        cmd: "start_proxy",
        args: { settings: { host: "127.0.0.1", port: 3456 } },
        expected: {
          method: "POST",
          url: "/api/proxy/start",
          body: { settings: { host: "127.0.0.1", port: 3456 } },
        },
      },
      {
        cmd: "stop_proxy",
        args: {},
        expected: { method: "POST", url: "/api/proxy/stop" },
      },
      {
        cmd: "test_proxy",
        args: { settings: { host: "127.0.0.1", port: 3456 } },
        expected: {
          method: "POST",
          url: "/api/proxy/test",
          body: { settings: { host: "127.0.0.1", port: 3456 } },
        },
      },
      {
        cmd: "set_proxy_takeover",
        args: { app: "gemini", enabled: true },
        expected: {
          method: "PUT",
          url: "/api/proxy/takeover/gemini",
          body: { enabled: true },
        },
      },
      {
        cmd: "set_proxy_takeover",
        args: { app: "open code", enabled: false },
        expected: {
          method: "PUT",
          url: "/api/proxy/takeover/open%20code",
          body: { enabled: false },
        },
      },
      {
        cmd: "restore_proxy",
        args: {},
        expected: { method: "POST", url: "/api/proxy/restore" },
      },
      {
        cmd: "recover_stale_proxy_takeover",
        args: {},
        expected: {
          method: "POST",
          url: "/api/proxy/recover-stale-takeover",
        },
      },
      {
        cmd: "proxy_recent_logs",
        args: {},
        expected: { method: "GET", url: "/api/proxy/logs/recent" },
      },
      {
        cmd: "restart_app",
        args: {},
        expected: { method: "POST", url: "/api/unsupported/restart_app" },
      },
      {
        cmd: "check_for_updates",
        args: {},
        expected: {
          method: "POST",
          url: "/api/unsupported/check_for_updates",
        },
      },
      {
        cmd: "is_portable_mode",
        args: {},
        expected: {
          method: "GET",
          url: "/api/unsupported/is_portable_mode",
        },
      },
      {
        cmd: "get_config_dir",
        args: { app: "foo bar" },
        expected: { method: "GET", url: "/api/config/foo%20bar/dir" },
      },
      {
        cmd: "get_config_dir_info",
        args: { app: "foo bar" },
        expected: { method: "GET", url: "/api/config/foo%20bar/dir-info" },
      },
      {
        cmd: "open_config_folder",
        args: { app: "claude" },
        expected: { method: "POST", url: "/api/config/claude/open" },
      },
      {
        cmd: "pick_directory",
        args: { defaultPath: "/tmp" },
        expected: {
          method: "POST",
          url: "/api/fs/pick-directory",
          body: { defaultPath: "/tmp" },
        },
      },
      {
        cmd: "get_claude_code_config_path",
        args: {},
        expected: { method: "GET", url: "/api/config/claude-code/path" },
      },
      {
        cmd: "get_app_config_path",
        args: {},
        expected: { method: "GET", url: "/api/config/app/path" },
      },
      {
        cmd: "open_app_config_folder",
        args: {},
        expected: { method: "POST", url: "/api/config/app/open" },
      },
      {
        cmd: "get_app_config_dir_override",
        args: {},
        expected: { method: "GET", url: "/api/config/app/override" },
      },
      {
        cmd: "set_app_config_dir_override",
        args: { path: "/override" },
        expected: {
          method: "PUT",
          url: "/api/config/app/override",
          body: { path: "/override" },
        },
      },
      {
        cmd: "apply_claude_plugin_config",
        args: { official: true },
        expected: {
          method: "POST",
          url: "/api/config/claude/plugin",
          body: { official: true },
        },
      },
      {
        cmd: "save_file_dialog",
        args: { defaultName: "config.json" },
        expected: {
          method: "POST",
          url: "/api/fs/save-file",
          body: { defaultName: "config.json" },
        },
      },
      {
        cmd: "open_file_dialog",
        args: {},
        expected: { method: "POST", url: "/api/fs/open-file" },
      },
      {
        cmd: "export_config_to_file",
        args: { filePath: "/tmp/config.json" },
        expected: {
          method: "POST",
          url: "/api/config/export",
          body: { filePath: "/tmp/config.json" },
        },
      },
      {
        cmd: "import_config_from_file",
        args: { filePath: "/tmp/config.json", content: "{}" },
        expected: {
          method: "POST",
          url: "/api/config/import",
          body: { filePath: "/tmp/config.json", content: "{}" },
        },
      },
      {
        cmd: "create_db_backup",
        args: {},
        expected: { method: "POST", url: "/api/config/backups" },
      },
      {
        cmd: "list_db_backups",
        args: {},
        expected: { method: "GET", url: "/api/config/backups" },
      },
      {
        cmd: "restore_db_backup",
        args: { filename: "db_backup.db" },
        expected: {
          method: "POST",
          url: "/api/config/backups/restore",
          body: { filename: "db_backup.db" },
        },
      },
      {
        cmd: "rename_db_backup",
        args: { oldFilename: "old.db", newName: "new" },
        expected: {
          method: "POST",
          url: "/api/config/backups/rename",
          body: { oldFilename: "old.db", newName: "new" },
        },
      },
      {
        cmd: "delete_db_backup",
        args: { filename: "backup name.db" },
        expected: {
          method: "DELETE",
          url: "/api/config/backups/backup%20name.db",
        },
      },
      {
        cmd: "sync_current_providers_live",
        args: {},
        expected: { method: "POST", url: "/api/providers/sync-current" },
      },
      {
        cmd: "open_external",
        args: { url: "https://example.com" },
        expected: {
          method: "POST",
          url: "/api/system/open-external",
          body: { url: "https://example.com" },
        },
      },
      {
        cmd: "get_claude_common_config_snippet",
        args: {},
        expected: {
          method: "GET",
          url: "/api/config/claude/common-snippet",
        },
      },
      {
        cmd: "set_claude_common_config_snippet",
        args: { snippet: "{}" },
        expected: {
          method: "PUT",
          url: "/api/config/claude/common-snippet",
          body: { snippet: "{}" },
        },
      },
      {
        cmd: "get_common_config_snippet",
        args: { appType: "claude" },
        expected: {
          method: "GET",
          url: "/api/config/claude/common-snippet",
        },
      },
      {
        cmd: "set_common_config_snippet",
        args: { appType: "codex", snippet: "{}" },
        expected: {
          method: "PUT",
          url: "/api/config/codex/common-snippet",
          body: { snippet: "{}" },
        },
      },
      {
        cmd: "fetch_models_for_config",
        args: {
          baseUrl: "https://api.example.com",
          apiKey: "sk-test",
          npm: "@ai-sdk/openai-compatible",
        },
        expected: {
          method: "POST",
          url: "/api/model-fetch",
          body: {
            baseUrl: "https://api.example.com",
            apiKey: "sk-test",
            npm: "@ai-sdk/openai-compatible",
          },
        },
      },
      {
        cmd: "get_codex_oauth_models",
        args: { accountId: "codex/account" },
        expected: {
          method: "GET",
          url: "/api/model-fetch/codex-oauth?accountId=codex%2Faccount",
        },
      },
      {
        cmd: "get_github_copilot_models",
        args: { accountId: null },
        expected: {
          method: "GET",
          url: "/api/model-fetch/github-copilot",
        },
      },
      {
        cmd: "list_managed_auth_accounts",
        args: { provider: "github_copilot" },
        expected: {
          method: "GET",
          url: "/api/auth/accounts?provider=github_copilot",
        },
      },
      {
        cmd: "import_managed_auth_account",
        args: {
          input: {
            provider: "github_copilot",
            id: "gh-1",
            label: "GitHub One",
            tokens: { accessToken: "token-1" },
          },
        },
        expected: {
          method: "POST",
          url: "/api/auth/accounts",
          body: {
            provider: "github_copilot",
            id: "gh-1",
            label: "GitHub One",
            tokens: { accessToken: "token-1" },
          },
        },
      },
      {
        cmd: "set_default_managed_auth_account",
        args: { provider: "github_copilot", accountId: "gh/1" },
        expected: {
          method: "POST",
          url: "/api/auth/accounts/default?provider=github_copilot&accountId=gh%2F1",
        },
      },
      {
        cmd: "delete_managed_auth_account",
        args: { provider: "github_copilot", accountId: "gh/1" },
        expected: {
          method: "DELETE",
          url: "/api/auth/accounts?provider=github_copilot&accountId=gh%2F1",
        },
      },
      {
        cmd: "logout_managed_auth_account",
        args: { provider: "github_copilot", accountId: "gh/1" },
        expected: {
          method: "POST",
          url: "/api/auth/accounts/logout?provider=github_copilot&accountId=gh%2F1",
        },
      },
      {
        cmd: "start_managed_auth_device_login",
        args: { request: { provider: "codex_oauth" } },
        expected: {
          method: "POST",
          url: "/api/auth/device/start",
          body: { provider: "codex_oauth" },
        },
      },
      {
        cmd: "poll_managed_auth_device_login",
        args: {
          request: { provider: "codex_oauth", sessionId: "session-1" },
        },
        expected: {
          method: "POST",
          url: "/api/auth/device/poll",
          body: { provider: "codex_oauth", sessionId: "session-1" },
        },
      },
      {
        cmd: "query_managed_auth_usage",
        args: { provider: "github_copilot", accountId: "gh/1" },
        expected: {
          method: "GET",
          url: "/api/auth/usage?provider=github_copilot&accountId=gh%2F1",
        },
      },
      {
        cmd: "query_subscription_quota",
        args: { provider: "codex", accountId: "account/1", force: true },
        expected: {
          method: "GET",
          url: "/api/subscriptions/quota?provider=codex&accountId=account%2F1&force=true",
        },
      },
      {
        cmd: "stream_check_provider",
        args: { appType: "opencode", providerId: "provider/1" },
        expected: {
          method: "POST",
          url: "/api/stream-check/providers/provider%2F1",
          body: { appType: "opencode" },
        },
      },
      {
        cmd: "stream_check_all_providers",
        args: { appType: "claude", proxyTargetsOnly: true },
        expected: {
          method: "POST",
          url: "/api/stream-check/all",
          body: { appType: "claude", proxyTargetsOnly: true },
        },
      },
      {
        cmd: "get_stream_check_config",
        args: {},
        expected: {
          method: "GET",
          url: "/api/stream-check/config",
        },
      },
      {
        cmd: "save_stream_check_config",
        args: { config: { timeoutSecs: 45 } },
        expected: {
          method: "PUT",
          url: "/api/stream-check/config",
          body: { timeoutSecs: 45 },
        },
      },
      {
        cmd: "get_usage_summary",
        args: {
          startDate: 1,
          endDate: 2,
          appType: "claude",
          providerId: "provider/1",
          model: "claude-sonnet-4",
        },
        expected: {
          method: "GET",
          url: "/api/usage/summary?startDate=1&endDate=2&appType=claude&providerId=provider%2F1&model=claude-sonnet-4",
        },
      },
      {
        cmd: "get_claude_desktop_default_routes",
        args: {},
        expected: {
          method: "GET",
          url: "/api/providers/claude-desktop/default-routes",
        },
      },
      {
        cmd: "get_claude_desktop_status",
        args: {},
        expected: {
          method: "GET",
          url: "/api/providers/claude-desktop/status",
        },
      },
      {
        cmd: "import_claude_desktop_providers_from_claude",
        args: {},
        expected: {
          method: "POST",
          url: "/api/providers/claude-desktop/import-from-claude",
        },
      },
    ];

    for (const testCase of cases) {
      const endpoint = commandToEndpoint(
        testCase.cmd,
        testCase.args as Record<string, unknown>,
      );
      expect(endpoint.method).toBe(testCase.expected.method);
      expect(endpoint.url).toBe(testCase.expected.url);
      if ("body" in testCase.expected) {
        expect(endpoint.body).toEqual((testCase.expected as any).body);
      } else {
        expect(endpoint.body).toBeUndefined();
      }
    }
  });

  it("throws when required args are missing", async () => {
    const { commandToEndpoint } = await importAdapter();
    const args: Record<string, unknown> = {};

    expect(() => commandToEndpoint("get_providers", args)).toThrow(
      'Missing argument "app"',
    );

    expect(() => commandToEndpoint("get_providers", undefined)).toThrow(
      'Missing argument "app"',
    );

    expect(() =>
      commandToEndpoint("update_provider", {
        app: "claude",
        provider: { name: "No id" },
      }),
    ).toThrow("Missing provider id");
  });
});

describe("invoke (web mode)", () => {
  it("surfaces coded 501 responses for unsupported host operations", async () => {
    const { invoke } = await importAdapter();
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      mockJsonResponse(
        {
          code: "operation_unavailable",
          error: "Operation is not available in web server mode",
        },
        false,
        501,
      ),
    );

    for (const command of [
      "check_for_updates",
      "restart_app",
      "is_portable_mode",
      "check_env_conflicts",
    ]) {
      await expect(invoke(command)).rejects.toMatchObject({
        message: "Operation is not available in web server mode",
        status: 501,
      });
    }
    expect(fetchMock).toHaveBeenCalledTimes(4);
  });

  it("includes Authorization header when credentials stored", async () => {
    const { invoke, WEB_AUTH_STORAGE_KEY } = await importAdapter();
    window.sessionStorage.setItem(WEB_AUTH_STORAGE_KEY, "encoded");

    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(mockJsonResponse({ ok: true }));

    await invoke("get_app_config_path");

    const [, init] = fetchMock.mock.calls[0] ?? [];
    const headers = (init as RequestInit)?.headers as Record<string, string>;
    expect(headers.Authorization).toBe("Basic encoded");
  });

  it("parses json error payloads with nested message", async () => {
    const { invoke } = await importAdapter();
    const payload = { payload: { message: "Nested error" } };

    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      mockJsonResponse(payload, false, 400),
    );

    await expect(invoke("get_app_config_path")).rejects.toMatchObject({
      message: "Nested error",
      status: 400,
    });
  });

  it("uses text payloads when json parsing fails", async () => {
    const { invoke } = await importAdapter();
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce({
      ok: false,
      status: 500,
      headers: new Headers({ "content-type": "text/plain" }),
      text: async () => "boom",
    } as Response);

    await expect(invoke("get_app_config_path")).rejects.toMatchObject({
      message: "boom",
      status: 500,
    });
  });

  it("reports html error responses as api failures", async () => {
    const { invoke } = await importAdapter();
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce({
      ok: false,
      status: 404,
      headers: new Headers({ "content-type": "text/html; charset=utf-8" }),
      text: async () => "<!doctype html><html><body>SPA shell</body></html>",
    } as Response);

    await expect(invoke("get_app_config_path")).rejects.toMatchObject({
      message: "API returned HTML 404: SPA shell",
      status: 404,
    });
  });

  it("returns undefined for 204 responses", async () => {
    const { invoke } = await importAdapter();
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce({
      ok: true,
      status: 204,
      headers: new Headers(),
      text: async () => "",
    } as Response);

    await expect(invoke("get_app_config_path")).resolves.toBeUndefined();
  });

  it("returns text for non-json responses", async () => {
    const { invoke } = await importAdapter();
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      mockTextResponse("plain"),
    );

    await expect(invoke("get_app_config_path")).resolves.toBe("plain");
  });

  it("rejects html success responses instead of returning the spa shell", async () => {
    const { invoke } = await importAdapter();
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce({
      ok: true,
      status: 200,
      headers: new Headers({ "content-type": "text/html" }),
      text: async () => "<!doctype html><html></html>",
    } as Response);

    await expect(invoke("get_app_config_path")).rejects.toMatchObject({
      message: "API returned HTML instead of JSON",
      status: 200,
    });
  });

  it("retries once on network errors for GET", async () => {
    vi.useFakeTimers();
    const { invoke } = await importAdapter();

    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockRejectedValueOnce(new TypeError("network"))
      .mockResolvedValueOnce(mockJsonResponse({ ok: true }));

    const promise = invoke("get_app_config_path");
    await vi.advanceTimersByTimeAsync(500);

    await expect(promise).resolves.toEqual({ ok: true });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("normalizes network failures after retries", async () => {
    vi.useFakeTimers();
    const { invoke } = await importAdapter();
    vi.spyOn(globalThis, "fetch").mockRejectedValue(
      new TypeError("Failed to fetch"),
    );

    const promise = invoke("get_app_config_path");
    const assertion = expect(promise).rejects.toThrow(
      "API connection failed. Check whether the cc-switch web server is running.",
    );
    await vi.advanceTimersByTimeAsync(500);

    await assertion;
  });

  it("open_external opens safe urls and blocks unsafe ones", async () => {
    const { invoke } = await importAdapter();
    const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      invoke("open_external", { url: "https://example.com" }),
    ).resolves.toBe(true);
    expect(openSpy).toHaveBeenCalledWith(
      "https://example.com",
      "_blank",
      "noopener,noreferrer",
    );

    openSpy.mockClear();

    await expect(
      invoke("open_external", { url: "javascript:alert(1)" }),
    ).resolves.toBe(true);
    expect(openSpy).not.toHaveBeenCalled();
    expect(warnSpy).toHaveBeenCalled();
  });
});
