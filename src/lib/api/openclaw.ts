import { invoke } from "./adapter";

export interface OpenClawDefaultModel {
  primary: string;
  fallbacks: string[];
}

export interface OpenClawHealthWarning {
  code: string;
  message: string;
  path?: string;
}

export interface OpenClawLiveModelSummary {
  id: string;
  name?: string;
  alias?: string;
}

export interface OpenClawLiveProviderSummary {
  id: string;
  baseUrl?: string;
  api?: string;
  models: OpenClawLiveModelSummary[];
  hasApiKey: boolean;
}

export interface OpenClawLiveStatus {
  defaultModel?: OpenClawDefaultModel;
  providers: OpenClawLiveProviderSummary[];
  warnings: OpenClawHealthWarning[];
}

export interface OpenClawWriteOutcome {
  backupPath?: string;
  warnings: OpenClawHealthWarning[];
}

export const openclawApi = {
  getStatus(): Promise<OpenClawLiveStatus> {
    return invoke("get_openclaw_status");
  },
  getProviders(): Promise<OpenClawLiveProviderSummary[]> {
    return invoke("get_openclaw_live_providers");
  },
  getProvider(providerId: string): Promise<OpenClawLiveProviderSummary | null> {
    return invoke("get_openclaw_live_provider", { providerId });
  },
  getDefaultModel(): Promise<OpenClawDefaultModel | null> {
    return invoke("get_openclaw_default_model");
  },
  setDefaultModel(model: OpenClawDefaultModel): Promise<OpenClawWriteOutcome> {
    return invoke("set_openclaw_default_model", { model });
  },
  clearDefaultModel(): Promise<OpenClawWriteOutcome> {
    return invoke("clear_openclaw_default_model");
  },
  getHealth(): Promise<OpenClawHealthWarning[]> {
    return invoke("scan_openclaw_config_health");
  },
};
