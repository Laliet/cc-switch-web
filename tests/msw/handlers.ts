import { http, HttpResponse } from "msw";
import type { AppId } from "@/lib/api/types";
import type { SkillRepo } from "@/lib/api/skills";
import type { McpServer, Provider, Settings } from "@/types";
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

export const handlers = [
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
    const { app, id } = await withJson<{ app: AppId; id: string | null }>(request);
    setBackupProviderId(app, id ?? null);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/update_providers_sort_order`, async ({ request }) => {
    const { updates = [], app } = await withJson<{
      updates: { id: string; sortIndex: number }[];
      app: AppId;
    }>(request);
    updateSortOrder(app, updates);
    return success(true);
  }),

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

  // Skill APIs
  http.post(`${TAURI_ENDPOINT}/get_skills`, () => success(getSkillsState())),

  http.post(`${TAURI_ENDPOINT}/install_skill`, async ({ request }) => {
    const { directory } = await withJson<{ directory: string; force?: boolean }>(request);
    installSkillState(directory);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/uninstall_skill`, async ({ request }) => {
    const { directory } = await withJson<{ directory: string }>(request);
    uninstallSkillState(directory);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/get_skill_repos`, () => success(getSkillReposState())),

  http.post(`${TAURI_ENDPOINT}/add_skill_repo`, async ({ request }) => {
    const { repo } = await withJson<{ repo: SkillRepo }>(request);
    addSkillRepoState(repo);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/remove_skill_repo`, async ({ request }) => {
    const { owner, name } = await withJson<{ owner: string; name: string }>(request);
    removeSkillRepoState(owner, name);
    return success(true);
  }),

  // MCP APIs
  http.post(`${TAURI_ENDPOINT}/get_mcp_config`, async ({ request }) => {
    const { app } = await withJson<{ app: AppId }>(request);
    return success(getMcpConfig(app));
  }),

  http.post(`${TAURI_ENDPOINT}/get_mcp_servers`, () => success(getUnifiedMcpServers())),

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

  http.post(`${TAURI_ENDPOINT}/upsert_mcp_server_in_config`, async ({ request }) => {
    const { app, id, spec } = await withJson<{
      app: AppId;
      id: string;
      spec: McpServer;
    }>(request);
    upsertMcpServer(app, id, spec);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/delete_mcp_server_in_config`, async ({ request }) => {
    const { app, id } = await withJson<{ app: AppId; id: string }>(request);
    deleteMcpServer(app, id);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/restart_app`, () => success(true)),

  http.post(`${TAURI_ENDPOINT}/check_env_conflicts`, () => success([])),

  http.post(`${TAURI_ENDPOINT}/get_settings`, () => success(getSettings())),

  http.post(`${TAURI_ENDPOINT}/save_settings`, async ({ request }) => {
    const { settings } = await withJson<{ settings: Settings }>(request);
    setSettings(settings);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/set_app_config_dir_override`, async ({ request }) => {
    const { path } = await withJson<{ path: string | null }>(request);
    setAppConfigDirOverrideState(path ?? null);
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/get_app_config_dir_override`, () =>
    success(getAppConfigDirOverride()),
  ),

  http.post(`${TAURI_ENDPOINT}/apply_claude_plugin_config`, async ({ request }) => {
    const { official } = await withJson<{ official: boolean }>(request);
    setSettings({ enableClaudePluginIntegration: !official });
    return success(true);
  }),

  http.post(`${TAURI_ENDPOINT}/get_config_dir`, async ({ request }) => {
    const { app } = await withJson<{ app: AppId }>(request);
    return success(app === "claude" ? "/default/claude" : "/default/codex");
  }),

  http.post(`${TAURI_ENDPOINT}/is_portable_mode`, () => success(false)),

  http.post(`${TAURI_ENDPOINT}/select_config_directory`, async ({ request }) => {
    const { defaultPath, default_path } = await withJson<{
      defaultPath?: string;
      default_path?: string;
    }>(request);
    const initial = defaultPath ?? default_path;
    return success(initial ? `${initial}/picked` : "/mock/selected-dir");
  }),

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

  http.post(`${TAURI_ENDPOINT}/import_config_from_file`, async ({ request }) => {
    const { filePath } = await withJson<{ filePath: string }>(request);
    if (!filePath) {
      return success({ success: false, message: "Missing file" });
    }
    setSettings({ language: "en" });
    return success({ success: true, backupId: "backup-123" });
  }),

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
          current_status: { status: 1, latency: 1500, timestamp: Date.now() / 1000 },
          timeline: [{ availability: 95 }, { availability: 98 }],
        },
        {
          provider: "duckcoding",
          provider_url: "https://duckcoding.com",
          service: "cc",
          category: "commercial",
          current_status: { status: 2, latency: 3000, timestamp: Date.now() / 1000 },
          timeline: [{ availability: 85 }],
        },
        {
          provider: "packycode",
          provider_url: "https://packyapi.com",
          service: "cc",
          category: "commercial",
          current_status: { status: 0, latency: 0, timestamp: Date.now() / 1000 },
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
          current_status: { status: 1, latency: 1500, timestamp: Date.now() / 1000 },
          timeline: [{ availability: 95 }, { availability: 98 }],
        },
        {
          provider: "duckcoding",
          provider_url: "https://duckcoding.com",
          service: "cc",
          category: "commercial",
          current_status: { status: 2, latency: 3000, timestamp: Date.now() / 1000 },
          timeline: [{ availability: 85 }],
        },
        {
          provider: "packycode",
          provider_url: "https://packyapi.com",
          service: "cc",
          category: "commercial",
          current_status: { status: 0, latency: 0, timestamp: Date.now() / 1000 },
          timeline: [{ availability: 20 }],
        },
      ],
    }),
  ),
];
