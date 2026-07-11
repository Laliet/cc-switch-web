import type {
  ProxyAppId,
  ProxyRouteAppId,
  FailoverQueueItem,
  ModelPricingRecord,
  ProxyRecentLog,
  ProxySettings,
  ProxyStatus,
  ProxyTakeoverResult,
  ProxyTestResult,
  Settings,
  WebDavSettings,
  WebDavAutoSyncResult,
  WebDavBackupEntry,
  WebDavSnapshotPreview,
  WebDavSyncResult,
} from "@/types";
import { invoke } from "./adapter";
import type { AppId } from "./types";

export type ConfigDirSource =
  | "override"
  | "service-home-default"
  | "account-home-fallback";

export interface ConfigDirInfo {
  dir: string;
  source: ConfigDirSource;
  overrideDir?: string;
  serviceHome?: string;
  accountHome?: string;
  homeMismatch: boolean;
}

export interface ConfigTransferResult {
  success: boolean;
  message: string;
  filePath?: string;
  backupId?: string;
}

export interface BackupEntry {
  filename: string;
  sizeBytes: number;
  createdAt: string;
}

export const settingsApi = {
  async get(): Promise<Settings> {
    return await invoke("get_settings");
  },

  async save(settings: Settings): Promise<boolean> {
    return await invoke("save_settings", { settings });
  },

  async restart(): Promise<boolean> {
    return await invoke("restart_app");
  },

  async checkUpdates(): Promise<void> {
    await invoke("check_for_updates");
  },

  async isPortable(): Promise<boolean> {
    return await invoke("is_portable_mode");
  },

  async getConfigDir(appId: AppId): Promise<string> {
    return await invoke("get_config_dir", { app: appId });
  },

  async getConfigDirInfo(appId: AppId): Promise<ConfigDirInfo> {
    return await invoke("get_config_dir_info", { app: appId });
  },

  async openConfigFolder(appId: AppId): Promise<void> {
    await invoke("open_config_folder", { app: appId });
  },

  async selectConfigDirectory(defaultPath?: string): Promise<string | null> {
    return await invoke("pick_directory", { defaultPath });
  },

  async getClaudeCodeConfigPath(): Promise<string> {
    return await invoke("get_claude_code_config_path");
  },

  async getAppConfigPath(): Promise<string> {
    return await invoke("get_app_config_path");
  },

  async openAppConfigFolder(): Promise<void> {
    await invoke("open_app_config_folder");
  },

  async getAppConfigDirOverride(): Promise<string | null> {
    return await invoke("get_app_config_dir_override");
  },

  async setAppConfigDirOverride(path: string | null): Promise<boolean> {
    return await invoke("set_app_config_dir_override", { path });
  },

  async applyClaudePluginConfig(options: {
    official: boolean;
  }): Promise<boolean> {
    const { official } = options;
    return await invoke("apply_claude_plugin_config", { official });
  },

  async saveFileDialog(defaultName: string): Promise<string | null> {
    return await invoke("save_file_dialog", { defaultName });
  },

  async openFileDialog(): Promise<string | null> {
    return await invoke("open_file_dialog");
  },

  async exportConfigToFile(filePath: string): Promise<ConfigTransferResult> {
    return await invoke("export_config_to_file", { filePath });
  },

  async importConfigFromFile(
    filePath: string,
    fileContent?: string,
  ): Promise<ConfigTransferResult> {
    return await invoke("import_config_from_file", {
      filePath,
      ...(typeof fileContent === "string" ? { content: fileContent } : {}),
    });
  },

  async syncCurrentProvidersLive(): Promise<void> {
    const result = (await invoke("sync_current_providers_live")) as
      | boolean
      | {
          success?: boolean;
          message?: string;
        };

    const success =
      result === true ||
      (typeof result === "object" && Boolean(result?.success));

    if (!success) {
      const message =
        typeof result === "object" && result?.message
          ? result.message
          : "Sync current providers failed";
      throw new Error(message);
    }
  },

  async updateWebCredentials(
    username: string,
    password: string,
  ): Promise<boolean> {
    return await invoke("update_web_credentials", { username, password });
  },

  async openExternal(url: string): Promise<void> {
    let u: URL;
    try {
      u = new URL(url);
    } catch {
      throw new Error("Invalid URL");
    }
    const scheme = u.protocol.replace(":", "").toLowerCase();
    if (scheme !== "http" && scheme !== "https") {
      throw new Error("Unsupported URL scheme");
    }
    await invoke("open_external", { url });
  },

  async getProxyStatus(): Promise<ProxyStatus> {
    return await invoke("proxy_status");
  },

  async getProxyConfig(): Promise<ProxySettings> {
    return await invoke("proxy_config");
  },

  async saveProxyConfig(settings: ProxySettings): Promise<ProxySettings> {
    return await invoke("save_proxy_config", { settings });
  },

  async saveProxySettings(settings: ProxySettings): Promise<boolean> {
    return await invoke("save_proxy_settings", { settings });
  },

  async startProxy(settings: ProxySettings): Promise<ProxyStatus> {
    return await invoke("start_proxy", { settings });
  },

  async stopProxy(): Promise<ProxyStatus> {
    return await invoke("stop_proxy");
  },

  async testProxy(settings: ProxySettings): Promise<ProxyTestResult> {
    return await invoke("test_proxy", { settings });
  },

  async setProxyTakeover(
    app: ProxyAppId,
    enabled: boolean,
  ): Promise<ProxyTakeoverResult> {
    return await invoke("set_proxy_takeover", { app, enabled });
  },

  async restoreProxy(): Promise<ProxyStatus> {
    return await invoke("restore_proxy");
  },

  async recoverStaleProxyTakeover(): Promise<ProxyStatus> {
    return await invoke("recover_stale_proxy_takeover");
  },

  async getProxyRecentLogs(): Promise<ProxyRecentLog[]> {
    return await invoke("proxy_recent_logs");
  },

  async getFailoverQueue(app: ProxyRouteAppId): Promise<FailoverQueueItem[]> {
    return await invoke("get_failover_queue", { app });
  },

  async replaceFailoverQueue(
    app: ProxyRouteAppId,
    providerIds: string[],
  ): Promise<FailoverQueueItem[]> {
    return await invoke("replace_failover_queue", { app, providerIds });
  },

  async addFailoverProvider(
    app: ProxyRouteAppId,
    providerId: string,
  ): Promise<FailoverQueueItem[]> {
    return await invoke("add_failover_provider", { app, providerId });
  },

  async removeFailoverProvider(
    app: ProxyRouteAppId,
    providerId: string,
  ): Promise<FailoverQueueItem[]> {
    return await invoke("remove_failover_provider", { app, providerId });
  },

  async clearFailoverQueue(app: ProxyRouteAppId): Promise<FailoverQueueItem[]> {
    return await invoke("clear_failover_queue", { app });
  },

  async resetProviderCircuit(
    app: ProxyAppId,
    providerId: string,
  ): Promise<ProxyStatus> {
    return await invoke("reset_provider_circuit", { app, providerId });
  },

  async listModelPricing(): Promise<ModelPricingRecord[]> {
    return await invoke("list_model_pricing");
  },

  async upsertModelPricing(record: ModelPricingRecord): Promise<boolean> {
    return await invoke("upsert_model_pricing", { record });
  },

  async deleteModelPricing(modelId: string): Promise<boolean> {
    return await invoke("delete_model_pricing", { modelId });
  },

  async uploadWebDavSnapshot(
    settings?: WebDavSettings,
  ): Promise<WebDavSyncResult> {
    return await invoke("upload_webdav_snapshot", { settings });
  },

  async previewWebDavSnapshot(
    settings?: WebDavSettings,
  ): Promise<WebDavSnapshotPreview> {
    return await invoke("preview_webdav_snapshot", { settings });
  },

  async downloadWebDavSnapshot(
    settings?: WebDavSettings,
  ): Promise<WebDavSyncResult> {
    return await invoke("download_webdav_snapshot", { settings });
  },

  async syncWebDavSnapshot(
    settings?: WebDavSettings,
  ): Promise<WebDavAutoSyncResult> {
    return await invoke("sync_webdav_snapshot", { settings });
  },

  async listWebDavBackups(
    settings?: WebDavSettings,
  ): Promise<WebDavBackupEntry[]> {
    return await invoke("list_webdav_backups", { settings });
  },

  async restoreWebDavBackup(
    backupId: string,
    settings?: WebDavSettings,
  ): Promise<WebDavSyncResult> {
    return await invoke("restore_webdav_backup", { backupId, settings });
  },
};

export const backupsApi = {
  createDbBackup: (): Promise<string> => invoke("create_db_backup"),
  listDbBackups: (): Promise<BackupEntry[]> => invoke("list_db_backups"),
  restoreDbBackup: (filename: string): Promise<string> =>
    invoke("restore_db_backup", { filename }),
  renameDbBackup: (oldFilename: string, newName: string): Promise<string> =>
    invoke("rename_db_backup", { oldFilename, newName }),
  deleteDbBackup: async (filename: string): Promise<void> => {
    await invoke("delete_db_backup", { filename });
  },
};
