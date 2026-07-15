import { http, HttpResponse } from "msw";
import type { AppId } from "@/lib/api/types";
import type { SkillRepo } from "@/lib/api/skills";
import type {
  McpServer,
  Provider,
  ProxyAppId,
  ProxySettings,
  Settings,
} from "@/types";
import {
  addProvider,
  deleteProvider,
  getCurrentProviderId,
  getBackupProviderId,
  getProviders,
  listProviders,
  resetProviderState,
  setBackupProviderId,
  setCurrentProviderId,
  updateProvider,
  updateSortOrder,
  getSettings,
  setSettings,
  getAppConfigDirOverride,
  setAppConfigDirOverrideState,
  getMcpConfig,
  setMcpServerEnabled,
  upsertMcpServer,
  deleteMcpServer,
  getUnifiedMcpServers,
  upsertUnifiedMcpServer,
  deleteUnifiedMcpServer,
  toggleMcpAppState,
  getSkillsState,
  installSkillState,
  uninstallSkillState,
  getSkillReposState,
  addSkillRepoState,
  removeSkillRepoState,
  getProxyConfigState,
  getProxyRecentLogsState,
  getProxyStatusState,
  recoverStaleProxyTakeoverState,
  restoreProxyState,
  setProxyConfigState,
  setProxyTakeoverState,
  startProxyState,
  stopProxyState,
  testProxyState,
  getCapabilitiesState,
  getOpenClawStatusState,
  setOpenClawDefaultModelState,
  getWorkspaceFilesState,
  getWorkspaceFileState,
  writeWorkspaceFileState,
  getWorkspaceBackupsState,
  restoreWorkspaceBackupState,
  getDailyMemoryState,
  getDailyMemoryFileState,
  writeDailyMemoryState,
  getSessionsPageState,
  getStreamCheckLogsState,
  addStreamCheckLogState,
} from "./state";

const TAURI_ENDPOINT = "http://tauri.local";

const withJson = async <T>(request: Request): Promise<T> => {
  try {
    const body = await request.text();
    if (!body) return {} as T;
    return JSON.parse(body) as T;
  } catch {
    return {} as T;
  }
};

const success = <T>(payload: T) => HttpResponse.json(payload as any);

const getMockConfigDir = (app: AppId): string => {
  switch (app) {
    case "claude":
      return "/default/claude";
    case "codex":
      return "/default/codex";
    case "gemini":
      return "/default/gemini";
    case "opencode":
    case "openclaw":
    case "omo":
    case "omo-slim":
      return "/default/opencode";
    case "claude-desktop":
      return "/default/claude-desktop";
    default:
      return "/default/unknown";
  }
};

export const handlers = [
  http.post(`${TAURI_ENDPOINT}/get_capabilities`, () =>
    success(getCapabilitiesState()),
  ),

  http.post(`${TAURI_ENDPOINT}/get_openclaw_status`, () =>
    success(getOpenClawStatusState()),
  ),
  http.post(`${TAURI_ENDPOINT}/get_openclaw_live_providers`, () =>
    success(getOpenClawStatusState().providers),
  ),
  http.post(
    `${TAURI_ENDPOINT}/get_openclaw_live_provider`,
    async ({ request }) => {
      const { providerId } = await withJson<{ providerId: string }>(request);
      return success(
        getOpenClawStatusState().providers.find(
          (provider) => provider.id === providerId,
        ) ?? null,
      );
    },
  ),
  http.post(`${TAURI_ENDPOINT}/get_openclaw_default_model`, () =>
    success(getOpenClawStatusState().defaultModel ?? null),
  ),
  http.post(
    `${TAURI_ENDPOINT}/set_openclaw_default_model`,
    async ({ request }) => {
      const { model } = await withJson<{
        model: { primary: string; fallbacks?: string[] };
      }>(request);
      return success(setOpenClawDefaultModelState(model));
    },
  ),
  http.post(`${TAURI_ENDPOINT}/clear_openclaw_default_model`, () =>
    success(setOpenClawDefaultModelState({ primary: "", fallbacks: [] })),
  ),
  http.post(`${TAURI_ENDPOINT}/scan_openclaw_config_health`, () => success([])),

  http.post(`${TAURI_ENDPOINT}/list_workspace_files`, () =>
    success(getWorkspaceFilesState()),
  ),
  http.post(`${TAURI_ENDPOINT}/read_workspace_file`, async ({ request }) => {
    const { filename } = await withJson<{ filename: string }>(request);
    const file = getWorkspaceFileState(filename);
    return file
      ? success({
          ...file,
          name: filename,
          sizeBytes: file.content.length,
        })
      : HttpResponse.json({ error: "workspace_not_found" }, { status: 404 });
  }),
  http.post(`${TAURI_ENDPOINT}/write_workspace_file`, async ({ request }) => {
    const { filename, content, expectedEtag } = await withJson<{
      filename: string;
      content: string;
      expectedEtag?: string | null;
    }>(request);
    try {
      return success(writeWorkspaceFileState(filename, content, expectedEtag));
    } catch {
      return HttpResponse.json(
        { error: "workspace_etag_conflict" },
        { status: 409 },
      );
    }
  }),
  http.post(`${TAURI_ENDPOINT}/list_workspace_backups`, async ({ request }) => {
    const { filename } = await withJson<{ filename: string }>(request);
    return success(getWorkspaceBackupsState(filename));
  }),
  http.post(
    `${TAURI_ENDPOINT}/restore_workspace_backup`,
    async ({ request }) => {
      const { filename, backupId, expectedEtag } = await withJson<{
        filename: string;
        backupId: string;
        expectedEtag?: string | null;
      }>(request);
      try {
        return success(
          restoreWorkspaceBackupState(filename, backupId, expectedEtag),
        );
      } catch {
        return HttpResponse.json(
          { error: "workspace_restore_failed" },
          { status: 409 },
        );
      }
    },
  ),
  http.post(`${TAURI_ENDPOINT}/list_daily_memory_files`, () =>
    success(getDailyMemoryState()),
  ),
  http.post(`${TAURI_ENDPOINT}/read_daily_memory_file`, async ({ request }) => {
    const { date } = await withJson<{ date: string }>(request);
    const file = getDailyMemoryFileState(date);
    return file
      ? success({
          ...file,
          name: `${date}.md`,
          sizeBytes: file.content.length,
        })
      : HttpResponse.json({ error: "workspace_not_found" }, { status: 404 });
  }),
  http.post(
    `${TAURI_ENDPOINT}/write_daily_memory_file`,
    async ({ request }) => {
      const { date, content, expectedEtag } = await withJson<{
        date: string;
        content: string;
        expectedEtag?: string | null;
      }>(request);
      try {
        return success(writeDailyMemoryState(date, content, expectedEtag));
      } catch {
        return HttpResponse.json(
          { error: "workspace_etag_conflict" },
          { status: 409 },
        );
      }
    },
  ),

  http.post(`${TAURI_ENDPOINT}/list_sessions_page`, async ({ request }) => {
    const { cursor, limit, providerId } = await withJson<{
      cursor?: string;
      limit?: number;
      providerId?: string;
    }>(request);
    return success(getSessionsPageState(cursor, limit, providerId));
  }),
  http.post(`${TAURI_ENDPOINT}/list_sessions`, () =>
    success(getSessionsPageState().sessions),
  ),
  http.post(`${TAURI_ENDPOINT}/get_session_messages`, () =>
    success([{ role: "user", content: "Mock session message" }]),
  ),
  http.post(`${TAURI_ENDPOINT}/delete_session`, () => success(true)),
  http.post(`${TAURI_ENDPOINT}/delete_sessions`, async ({ request }) => {
    const { items = [] } = await withJson<{ items?: unknown[] }>(request);
    return success(items.map(() => ({ success: true })));
  }),

  http.post(`${TAURI_ENDPOINT}/get_stream_check_logs`, async ({ request }) => {
    const { query } = await withJson<{
      query?: { appType?: string; providerId?: string };
    }>(request);
    return success(getStreamCheckLogsState(query?.appType, query?.providerId));
  }),
  http.post(
    `${TAURI_ENDPOINT}/get_latest_stream_check_logs`,
    async ({ request }) => {
      const { appType } = await withJson<{ appType?: string }>(request);
      return success(getStreamCheckLogsState(appType));
    },
  ),
  http.post(`${TAURI_ENDPOINT}/stream_check_provider`, async ({ request }) => {
    const { appType, providerId } = await withJson<{
      appType: string;
      providerId: string;
    }>(request);
    const result = {
      status: "operational",
      success: true,
      message: "ok",
      responseTimeMs: 42,
      httpStatus: 200,
      modelUsed: "mock-model",
      testedAt: Math.floor(Date.now() / 1000),
      retryCount: 0,
    };
    addStreamCheckLogState({
      id: Date.now(),
      providerId,
      providerName: providerId,
      appType,
      ...result,
    });
    return success(result);
  }),
  http.post(`${TAURI_ENDPOINT}/stream_check_all_providers`, () => success([])),
  http.post(`${TAURI_ENDPOINT}/get_stream_check_config`, () =>
    success({
      timeoutSecs: 45,
      maxRetries: 2,
      degradedThresholdMs: 6000,
      claudeModel: "claude-test",
      codexModel: "gpt-test",
      geminiModel: "gemini-test",
      testPrompt: "Who are you?",
    }),
  ),
  http.post(`${TAURI_ENDPOINT}/save_stream_check_config`, () => success(true)),

  http.post(`${TAURI_ENDPOINT}/get_providers`, async ({ request }) => {
    const { app } = await withJson<{ app: AppId }>(request);
    return success(getProviders(app));
  }),

  http.post(`${TAURI_ENDPOINT}/get_current_provider`, async ({ request }) => {
    const { app } = await withJson<{ app: AppId }>(request);
    return success(getCurrentProviderId(app));
  }),

  http.post(`${TAURI_ENDPOINT}/get_backup_provider`, async ({ request }) => {
    const { app } = await withJson<{ app: AppId }>(request);
    return success(getBackupProviderId(app));
  }),

  http.post(`${TAURI_ENDPOINT}/set_backup_provider`, async ({ request }) => {
    const { app, id } = await withJson<{ app: AppId; id: string | null }>(
      request,
    );
    setBackupProviderId(app, id ?? null);
    return success(true);
  }),

  http.post(
    `${TAURI_ENDPOINT}/update_providers_sort_order`,
    async ({ request }) => {
      const { updates = [], app } = await withJson<{
        updates: { id: string; sortIndex: number }[];
        app: AppId;
      }>(request);
      updateSortOrder(app, updates);
      return success(true);
    },
  ),

  http.post(`${TAURI_ENDPOINT}/update_tray_menu`, () => success(true)),

  http.post(`${TAURI_ENDPOINT}/switch_provider`, async ({ request }) => {
    const { id, app } = await withJson<{ id: string; app: AppId }>(request);
    const providers = listProviders(app);
    if (!providers[id]) {
      return HttpResponse.json(false, { status: 404 });
    }
    setCurrentProviderId(app, id);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/add_provider`, async ({ request }) => {
    const { provider, app } = await withJson<{
      provider: Provider & { id?: string };
      app: AppId;
    }>(request);

    const newId = provider.id ?? `mock-${Date.now()}`;
    addProvider(app, { ...provider, id: newId });
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/update_provider`, async ({ request }) => {
    const { provider, app } = await withJson<{
      provider: Provider;
      app: AppId;
    }>(request);
    updateProvider(app, provider);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/delete_provider`, async ({ request }) => {
    const { id, app } = await withJson<{ id: string; app: AppId }>(request);
    deleteProvider(app, id);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/import_default_config`, async () => {
    resetProviderState();
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/open_external`, () => success(true)),

  http.post(
    `${TAURI_ENDPOINT}/query_subscription_quota`,
    async ({ request }) => {
      const { provider } = await withJson<{ provider: string }>(request);
      return success({
        provider,
        source: "mock_credentials",
        status: "unavailable",
        windows: [],
        fetchedAt: Date.now(),
        error: "No mock subscription credentials",
      });
    },
  ),

  // Skill APIs
  http.post(`${TAURI_ENDPOINT}/get_skills`, () => success(getSkillsState())),

  http.post(`${TAURI_ENDPOINT}/install_skill`, async ({ request }) => {
    const { directory } = await withJson<{
      directory: string;
      force?: boolean;
    }>(request);
    installSkillState(directory);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/uninstall_skill`, async ({ request }) => {
    const { directory } = await withJson<{ directory: string }>(request);
    uninstallSkillState(directory);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/get_skill_repos`, () =>
    success(getSkillReposState()),
  ),

  http.post(`${TAURI_ENDPOINT}/add_skill_repo`, async ({ request }) => {
    const { repo } = await withJson<{ repo: SkillRepo }>(request);
    addSkillRepoState(repo);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/remove_skill_repo`, async ({ request }) => {
    const { owner, name } = await withJson<{ owner: string; name: string }>(
      request,
    );
    removeSkillRepoState(owner, name);
    return success(true);
  }),

  // MCP APIs
  http.post(`${TAURI_ENDPOINT}/get_mcp_config`, async ({ request }) => {
    const { app } = await withJson<{ app: AppId }>(request);
    return success(getMcpConfig(app));
  }),

  http.post(`${TAURI_ENDPOINT}/get_mcp_servers`, () =>
    success(getUnifiedMcpServers()),
  ),

  http.post(`${TAURI_ENDPOINT}/import_mcp_from_claude`, () => success(1)),
  http.post(`${TAURI_ENDPOINT}/import_mcp_from_codex`, () => success(1)),

  http.post(`${TAURI_ENDPOINT}/set_mcp_enabled`, async ({ request }) => {
    const { app, id, enabled } = await withJson<{
      app: AppId;
      id: string;
      enabled: boolean;
    }>(request);
    setMcpServerEnabled(app, id, enabled);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/toggle_mcp_app`, async ({ request }) => {
    const { serverId, app, enabled } = await withJson<{
      serverId: string;
      app: AppId;
      enabled: boolean;
    }>(request);
    toggleMcpAppState(serverId, app, enabled);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/upsert_mcp_server`, async ({ request }) => {
    const { server } = await withJson<{ server: McpServer }>(request);
    upsertUnifiedMcpServer(server);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/delete_mcp_server`, async ({ request }) => {
    const { id } = await withJson<{ id: string }>(request);
    deleteUnifiedMcpServer(id);
    return success(true);
  }),

  http.post(
    `${TAURI_ENDPOINT}/upsert_mcp_server_in_config`,
    async ({ request }) => {
      const { app, id, spec } = await withJson<{
        app: AppId;
        id: string;
        spec: McpServer;
      }>(request);
      upsertMcpServer(app, id, spec);
      return success(true);
    },
  ),

  http.post(
    `${TAURI_ENDPOINT}/delete_mcp_server_in_config`,
    async ({ request }) => {
      const { app, id } = await withJson<{ app: AppId; id: string }>(request);
      deleteMcpServer(app, id);
      return success(true);
    },
  ),

  http.post(`${TAURI_ENDPOINT}/restart_app`, () => success(true)),

  http.post(`${TAURI_ENDPOINT}/check_env_conflicts`, () => success([])),

  http.post(`${TAURI_ENDPOINT}/get_settings`, () => success(getSettings())),

  http.post(`${TAURI_ENDPOINT}/list_db_backups`, () => success([])),

  http.post(`${TAURI_ENDPOINT}/save_settings`, async ({ request }) => {
    const { settings } = await withJson<{ settings: Settings }>(request);
    setSettings(settings);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/proxy_status`, () =>
    success(getProxyStatusState()),
  ),

  http.post(`${TAURI_ENDPOINT}/proxy_recent_logs`, () =>
    success(getProxyRecentLogsState()),
  ),

  http.post(`${TAURI_ENDPOINT}/proxy_config`, () =>
    success(getProxyConfigState()),
  ),

  http.post(`${TAURI_ENDPOINT}/save_proxy_config`, async ({ request }) => {
    const { settings } = await withJson<{ settings: ProxySettings }>(request);
    return success(setProxyConfigState(settings));
  }),

  http.post(`${TAURI_ENDPOINT}/save_proxy_settings`, async ({ request }) => {
    const { settings } = await withJson<{ settings: ProxySettings }>(request);
    setProxyConfigState(settings);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/start_proxy`, async ({ request }) => {
    const { settings } = await withJson<{ settings: ProxySettings }>(request);
    return success(startProxyState(settings));
  }),

  http.post(`${TAURI_ENDPOINT}/stop_proxy`, () => success(stopProxyState())),

  http.post(`${TAURI_ENDPOINT}/test_proxy`, async ({ request }) => {
    const { settings } = await withJson<{ settings: ProxySettings }>(request);
    return success(testProxyState(settings));
  }),

  http.post(`${TAURI_ENDPOINT}/set_proxy_takeover`, async ({ request }) => {
    const { app, enabled } = await withJson<{
      app: ProxyAppId;
      enabled: boolean;
    }>(request);
    return success(setProxyTakeoverState(app, enabled));
  }),

  http.post(`${TAURI_ENDPOINT}/restore_proxy`, () =>
    success(restoreProxyState()),
  ),

  http.post(`${TAURI_ENDPOINT}/recover_stale_proxy_takeover`, () =>
    success(recoverStaleProxyTakeoverState()),
  ),

  http.post(
    `${TAURI_ENDPOINT}/set_app_config_dir_override`,
    async ({ request }) => {
      const { path } = await withJson<{ path: string | null }>(request);
      setAppConfigDirOverrideState(path ?? null);
      return success(true);
    },
  ),

  http.post(`${TAURI_ENDPOINT}/get_app_config_dir_override`, () =>
    success(getAppConfigDirOverride()),
  ),

  http.post(`${TAURI_ENDPOINT}/get_config_dir_info`, async ({ request }) => {
    const { app } = await withJson<{ app: AppId }>(request);
    return success({
      dir: getMockConfigDir(app),
      source: "service-home-default",
      homeMismatch: false,
      serviceHome: "/home/mock",
      accountHome: "/home/mock",
    });
  }),

  http.post(
    `${TAURI_ENDPOINT}/apply_claude_plugin_config`,
    async ({ request }) => {
      const { official } = await withJson<{ official: boolean }>(request);
      setSettings({ enableClaudePluginIntegration: !official });
      return success(true);
    },
  ),

  http.get("*/api/proxy/status", () => success(getProxyStatusState())),

  http.get("*/api/proxy/logs/recent", () => success(getProxyRecentLogsState())),

  http.get("*/api/proxy/config", () => success(getProxyConfigState())),

  http.put("*/api/proxy/config", async ({ request }) => {
    const { settings } = await withJson<{ settings: ProxySettings }>(request);
    return success(setProxyConfigState(settings));
  }),

  http.put("*/api/proxy/settings", async ({ request }) => {
    const { settings } = await withJson<{ settings: ProxySettings }>(request);
    setProxyConfigState(settings);
    return success(true);
  }),

  http.post("*/api/proxy/start", async ({ request }) => {
    const { settings } = await withJson<{ settings: ProxySettings }>(request);
    return success(startProxyState(settings));
  }),

  http.post("*/api/proxy/stop", () => success(stopProxyState())),

  http.post("*/api/proxy/test", async ({ request }) => {
    const { settings } = await withJson<{ settings: ProxySettings }>(request);
    return success(testProxyState(settings));
  }),

  http.get("*/api/proxy/takeover", () => success(getProxyStatusState())),

  http.put("*/api/proxy/takeover/:app", async ({ params, request }) => {
    const { enabled } = await withJson<{ enabled: boolean }>(request);
    return success(setProxyTakeoverState(params.app as ProxyAppId, enabled));
  }),

  http.post("*/api/proxy/restore", () => success(restoreProxyState())),

  http.post("*/api/proxy/recover-stale-takeover", () =>
    success(recoverStaleProxyTakeoverState()),
  ),

  http.post(`${TAURI_ENDPOINT}/get_config_dir`, async ({ request }) => {
    const { app } = await withJson<{ app: AppId }>(request);
    return success(getMockConfigDir(app));
  }),

  http.post(`${TAURI_ENDPOINT}/is_portable_mode`, () => success(false)),

  http.post(
    `${TAURI_ENDPOINT}/select_config_directory`,
    async ({ request }) => {
      const { defaultPath, default_path } = await withJson<{
        defaultPath?: string;
        default_path?: string;
      }>(request);
      const initial = defaultPath ?? default_path;
      return success(initial ? `${initial}/picked` : "/mock/selected-dir");
    },
  ),

  http.post(`${TAURI_ENDPOINT}/pick_directory`, async ({ request }) => {
    const { defaultPath, default_path } = await withJson<{
      defaultPath?: string;
      default_path?: string;
    }>(request);
    const initial = defaultPath ?? default_path;
    return success(initial ? `${initial}/picked` : "/mock/selected-dir");
  }),

  http.post(`${TAURI_ENDPOINT}/open_file_dialog`, () =>
    success("/mock/import-settings.json"),
  ),

  http.post(
    `${TAURI_ENDPOINT}/import_config_from_file`,
    async ({ request }) => {
      const { filePath } = await withJson<{ filePath: string }>(request);
      if (!filePath) {
        return success({ success: false, message: "Missing file" });
      }
      setSettings({ language: "en" });
      return success({ success: true, backupId: "backup-123" });
    },
  ),

  http.post(`${TAURI_ENDPOINT}/export_config_to_file`, async ({ request }) => {
    const { filePath } = await withJson<{ filePath: string }>(request);
    if (!filePath) {
      return success({ success: false, message: "Invalid destination" });
    }
    return success({ success: true, filePath });
  }),

  http.post(`${TAURI_ENDPOINT}/save_file_dialog`, () =>
    success("/mock/export-settings.json"),
  ),

  // Sync current providers live (no-op success)
  http.post(`${TAURI_ENDPOINT}/sync_current_providers_live`, () =>
    success({ success: true }),
  ),

  // check_relay_pulse: GUI 模式下的健康检查代理
  http.post(`${TAURI_ENDPOINT}/check_relay_pulse`, () =>
    HttpResponse.json({
      meta: { period: "24h", count: 3 },
      data: [
        {
          provider: "88code",
          provider_url: "https://88code.com",
          service: "cc",
          category: "commercial",
          current_status: {
            status: 1,
            latency: 1500,
            timestamp: Date.now() / 1000,
          },
          timeline: [{ availability: 95 }, { availability: 98 }],
        },
        {
          provider: "duckcoding",
          provider_url: "https://duckcoding.com",
          service: "cc",
          category: "commercial",
          current_status: {
            status: 2,
            latency: 3000,
            timestamp: Date.now() / 1000,
          },
          timeline: [{ availability: 85 }],
        },
        {
          provider: "packycode",
          provider_url: "https://packyapi.com",
          service: "cc",
          category: "commercial",
          current_status: {
            status: 0,
            latency: 0,
            timestamp: Date.now() / 1000,
          },
          timeline: [{ availability: 20 }],
        },
      ],
    }),
  ),

  http.get("https://relaypulse.top/api/status", () =>
    HttpResponse.json({
      meta: { period: "24h", count: 3 },
      data: [
        {
          provider: "88code",
          provider_url: "https://88code.com",
          service: "cc",
          category: "commercial",
          current_status: {
            status: 1,
            latency: 1500,
            timestamp: Date.now() / 1000,
          },
          timeline: [{ availability: 95 }, { availability: 98 }],
        },
        {
          provider: "duckcoding",
          provider_url: "https://duckcoding.com",
          service: "cc",
          category: "commercial",
          current_status: {
            status: 2,
            latency: 3000,
            timestamp: Date.now() / 1000,
          },
          timeline: [{ availability: 85 }],
        },
        {
          provider: "packycode",
          provider_url: "https://packyapi.com",
          service: "cc",
          category: "commercial",
          current_status: {
            status: 0,
            latency: 0,
            timestamp: Date.now() / 1000,
          },
          timeline: [{ availability: 20 }],
        },
      ],
    }),
  ),
];
