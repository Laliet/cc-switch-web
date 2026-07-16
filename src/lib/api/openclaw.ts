import { invoke } from "./adapter";

export interface OpenClawDefaultModel {
  primary: string;
  fallbacks?: string[];
  [key: string]: unknown;
}

export interface OpenClawModelCatalogEntry {
  alias?: string;
  [key: string]: unknown;
}

export interface OpenClawAgentsDefaults {
  model?: OpenClawDefaultModel;
  models?: Record<string, OpenClawModelCatalogEntry>;
  workspace?: string;
  timeoutSeconds?: number;
  timeout?: number;
  contextTokens?: number;
  maxConcurrent?: number;
  [key: string]: unknown;
}

export type OpenClawEnvConfig = Record<string, unknown>;

export type OpenClawToolsProfile = "minimal" | "coding" | "messaging" | "full";

export interface OpenClawToolsConfig {
  profile?: OpenClawToolsProfile | string;
  allow?: string[];
  deny?: string[];
  [key: string]: unknown;
}

export interface OpenClawSection<T> {
  value: T;
  etag: string;
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
  etag: string;
}

export interface OpenClawWriteOutcome {
  backupPath?: string;
  warnings: OpenClawHealthWarning[];
  etag: string;
}

export type OpenClawReconciliationStatus =
  | "new"
  | "changed"
  | "unchanged"
  | "invalid";

export interface OpenClawReconciliationItem {
  providerId: string;
  displayName: string;
  status: OpenClawReconciliationStatus;
  modelCount: number;
  hasApiKey: boolean;
  liveConfigManaged: boolean;
  reason?: string;
}

export interface OpenClawReconciliationPreview {
  etag: string;
  liveCount: number;
  storedCount: number;
  items: OpenClawReconciliationItem[];
}

export interface OpenClawReconciliationOutcome {
  imported: number;
  updated: number;
  unchanged: number;
  ignored: number;
  invalid: number;
  etag: string;
}

export const openclawApi = {
  getStatus(): Promise<OpenClawLiveStatus> {
    return invoke("get_openclaw_status");
  },
  getRawConfig(): Promise<OpenClawSection<string>> {
    return invoke("get_openclaw_raw_config");
  },
  setRawConfig(
    source: string,
    expectedEtag?: string,
  ): Promise<OpenClawWriteOutcome> {
    return invoke("set_openclaw_raw_config", {
      source,
      expectedEtag: expectedEtag ?? null,
    });
  },
  getProviders(): Promise<OpenClawLiveProviderSummary[]> {
    return invoke("get_openclaw_live_providers");
  },
  getProvider(providerId: string): Promise<OpenClawLiveProviderSummary | null> {
    return invoke("get_openclaw_live_provider", { providerId });
  },
  previewReconciliation(): Promise<OpenClawReconciliationPreview> {
    return invoke("preview_openclaw_provider_reconciliation");
  },
  applyReconciliation(
    providerIds: string[],
    updateExisting: boolean,
    expectedEtag: string,
  ): Promise<OpenClawReconciliationOutcome> {
    return invoke("apply_openclaw_provider_reconciliation", {
      providerIds,
      updateExisting,
      expectedEtag,
    });
  },
  importLiveProviders(): Promise<number> {
    return invoke("import_openclaw_providers_from_live");
  },
  getDefaultModel(): Promise<OpenClawDefaultModel | null> {
    return invoke("get_openclaw_default_model");
  },
  setDefaultModel(
    model: OpenClawDefaultModel,
    expectedEtag?: string,
  ): Promise<OpenClawWriteOutcome> {
    return invoke("set_openclaw_default_model", {
      model,
      expectedEtag: expectedEtag ?? null,
    });
  },
  clearDefaultModel(expectedEtag?: string): Promise<OpenClawWriteOutcome> {
    return invoke("clear_openclaw_default_model", {
      expectedEtag: expectedEtag ?? null,
    });
  },
  getModelCatalog(): Promise<
    OpenClawSection<Record<string, OpenClawModelCatalogEntry> | null>
  > {
    return invoke("get_openclaw_model_catalog");
  },
  setModelCatalog(
    catalog: Record<string, OpenClawModelCatalogEntry>,
    expectedEtag?: string,
  ): Promise<OpenClawWriteOutcome> {
    return invoke("set_openclaw_model_catalog", {
      catalog,
      expectedEtag: expectedEtag ?? null,
    });
  },
  getAgentsDefaults(): Promise<OpenClawSection<OpenClawAgentsDefaults | null>> {
    return invoke("get_openclaw_agents_defaults");
  },
  setAgentsDefaults(
    defaults: OpenClawAgentsDefaults,
    expectedEtag?: string,
  ): Promise<OpenClawWriteOutcome> {
    return invoke("set_openclaw_agents_defaults", {
      defaults,
      expectedEtag: expectedEtag ?? null,
    });
  },
  getEnv(): Promise<OpenClawSection<OpenClawEnvConfig>> {
    return invoke("get_openclaw_env");
  },
  setEnv(
    env: OpenClawEnvConfig,
    expectedEtag?: string,
  ): Promise<OpenClawWriteOutcome> {
    return invoke("set_openclaw_env", {
      env,
      expectedEtag: expectedEtag ?? null,
    });
  },
  getTools(): Promise<OpenClawSection<OpenClawToolsConfig>> {
    return invoke("get_openclaw_tools");
  },
  setTools(
    tools: OpenClawToolsConfig,
    expectedEtag?: string,
  ): Promise<OpenClawWriteOutcome> {
    return invoke("set_openclaw_tools", {
      tools,
      expectedEtag: expectedEtag ?? null,
    });
  },
  getHealth(): Promise<OpenClawHealthWarning[]> {
    return invoke("scan_openclaw_config_health");
  },
};
