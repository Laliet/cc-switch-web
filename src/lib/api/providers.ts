import type { Provider } from "@/types";
import { invoke, isWeb } from "./adapter";
import type { AppId } from "./types";

type UnlistenFn = () => void;

export interface ProviderSortUpdate {
  id: string;
  sortIndex: number;
}

export interface ProviderSwitchEvent {
  appType: AppId;
  providerId: string;
}

export const providersApi = {
  async getAll(appId: AppId): Promise<Record<string, Provider>> {
    return await invoke("get_providers", { app: appId });
  },

  async getCurrent(appId: AppId): Promise<string> {
    return await invoke("get_current_provider", { app: appId });
  },

  async getBackup(appId: AppId): Promise<string | null> {
    return await invoke("get_backup_provider", { app: appId });
  },

  async setBackup(id: string | null, appId: AppId): Promise<boolean> {
    return await invoke("set_backup_provider", { id, app: appId });
  },

  async add(provider: Provider, appId: AppId): Promise<boolean> {
    return await invoke("add_provider", { provider, app: appId });
  },

  async update(provider: Provider, appId: AppId): Promise<boolean> {
    return await invoke("update_provider", { provider, app: appId });
  },

  async delete(id: string, appId: AppId): Promise<boolean> {
    return await invoke("delete_provider", { id, app: appId });
  },

  async switch(id: string, appId: AppId): Promise<boolean> {
    const result = await invoke("switch_provider", { id, app: appId });
    const success = Boolean(result);
    if (isWeb()) {
      try {
        await invoke("sync_current_providers_live");
      } catch (error) {
        const detail =
          error instanceof Error && error.message
            ? error.message
            : "同步配置失败";
        throw new Error(`切换成功，但同步配置失败：${detail}`);
      }
    }
    return success;
  },

  async importDefault(appId: AppId): Promise<boolean> {
    return await invoke("import_default_config", { app: appId });
  },

  async updateTrayMenu(): Promise<boolean> {
    return await invoke("update_tray_menu");
  },

  async updateSortOrder(
    updates: ProviderSortUpdate[],
    appId: AppId,
  ): Promise<boolean> {
    return await invoke("update_providers_sort_order", { updates, app: appId });
  },

  async getOmoPluginStatus(): Promise<boolean> {
    return await invoke("get_omo_plugin_status");
  },

  async onSwitched(
    handler: (event: ProviderSwitchEvent) => void,
  ): Promise<UnlistenFn> {
    if (isWeb()) {
      // Web builds have no Tauri event bridge; return a noop unsubscriber.
      return () => {};
    }
    const { listen } = await import("@tauri-apps/api/event");
    return await listen("provider-switched", (event) => {
      const payload = event.payload as ProviderSwitchEvent;
      handler(payload);
    });
  },
};
