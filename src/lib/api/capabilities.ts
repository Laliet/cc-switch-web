import { invoke } from "./adapter";
import type { AppId } from "./types";

export interface FeatureCapabilities {
  directoryPicker: boolean;
  openExternal: boolean;
  endpointTest: boolean;
  workspace: boolean;
  subscriptionQuota: boolean;
  tray: boolean;
  terminalLaunch: boolean;
  configDirOverride: boolean;
  fileDialogs: boolean;
  sessionManager: boolean;
  usageDashboard: boolean;
  environmentManagement: boolean;
  appUpdate: boolean;
  portableMode: boolean;
  claudePluginIntegration: boolean;
}

export interface AppCapabilities {
  providers: boolean;
  prompts: boolean;
  mcp: boolean;
  skills: boolean;
  usage: boolean;
  sessions: boolean;
  localRouting: boolean;
  additiveProviderMode: boolean;
  hostManaged: boolean;
}

export interface RuntimeCapabilities {
  runtime: "web" | "desktop";
  host: "server" | "local";
  apps: AppId[];
  features: FeatureCapabilities;
  appFeatures: Partial<Record<AppId, AppCapabilities>>;
}

export const capabilitiesApi = {
  async get(): Promise<RuntimeCapabilities> {
    return await invoke("get_capabilities");
  },
};
