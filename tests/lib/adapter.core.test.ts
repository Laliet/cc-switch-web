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
      expect(
        getWebApiBaseValidationError("https://api.example.com"),
      ).toBe(
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

    setWebCredentials("secret", "/api");
    const stored = window.sessionStorage.getItem(WEB_AUTH_STORAGE_KEY);
    expect(stored).not.toBeNull();
    const parsed = JSON.parse(stored as string) as {
      token: string;
      apiBase: string | null;
    };
    expect(parsed).toEqual({
      token: Buffer.from("admin:secret").toString("base64"),
      apiBase: "/api",
    });

    window.sessionStorage.setItem(WEB_CSRF_STORAGE_KEY, "csrf");
    clearWebCredentials();

    expect(window.sessionStorage.getItem(WEB_AUTH_STORAGE_KEY)).toBeNull();
    expect(window.sessionStorage.getItem(WEB_CSRF_STORAGE_KEY)).toBeNull();
  });
});

describe("commandToEndpoint", () => {
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
        cmd: "restart_app",
        args: {},
        expected: { method: "POST", url: "/api/system/restart" },
      },
      {
        cmd: "check_for_updates",
        args: {},
        expected: { method: "POST", url: "/api/system/check-updates" },
      },
      {
        cmd: "is_portable_mode",
        args: {},
        expected: { method: "GET", url: "/api/system/is-portable" },
      },
      {
        cmd: "get_config_dir",
        args: { app: "foo bar" },
        expected: { method: "GET", url: "/api/config/foo%20bar/dir" },
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
      "Missing argument \"app\"",
    );

    expect(() =>
      commandToEndpoint("get_providers", undefined),
    ).toThrow("Missing argument \"app\"");

    expect(() =>
      commandToEndpoint("update_provider", {
        app: "claude",
        provider: { name: "No id" },
      }),
    ).toThrow("Missing provider id");
  });
});

describe("invoke (web mode)", () => {
  it("returns short-circuit responses for special commands", async () => {
    const { invoke } = await importAdapter();

    await expect(invoke("check_for_updates")).resolves.toBeNull();
    await expect(invoke("restart_app")).resolves.toBeUndefined();
    await expect(invoke("is_portable_mode")).resolves.toBe(false);
    await expect(invoke("check_env_conflicts")).resolves.toEqual([]);
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
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      {
        ok: false,
        status: 500,
        headers: new Headers({ "content-type": "text/plain" }),
        text: async () => "boom",
      } as Response,
    );

    await expect(invoke("get_app_config_path")).rejects.toMatchObject({
      message: "boom",
      status: 500,
    });
  });

  it("returns undefined for 204 responses", async () => {
    const { invoke } = await importAdapter();
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      {
        ok: true,
        status: 204,
        headers: new Headers(),
        text: async () => "",
      } as Response,
    );

    await expect(invoke("get_app_config_path")).resolves.toBeUndefined();
  });

  it("returns text for non-json responses", async () => {
    const { invoke } = await importAdapter();
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      mockTextResponse("plain"),
    );

    await expect(invoke("get_app_config_path")).resolves.toBe("plain");
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
