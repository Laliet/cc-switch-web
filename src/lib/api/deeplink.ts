import { invoke } from "./adapter";
import type { AppId } from "./types";

export type DeepLinkResourceType = "provider" | "prompt" | "mcp" | "skill";

export interface DeepLinkImportRequest {
  version: string;
  resource: DeepLinkResourceType;
  app?: AppId;
  name?: string;
  enabled?: boolean;
  homepage?: string;
  endpoint?: string;
  apiKey?: string;
  icon?: string;
  model?: string;
  notes?: string;
  haikuModel?: string;
  sonnetModel?: string;
  opusModel?: string;
  content?: string;
  description?: string;
  apps?: string;
  repo?: string;
  directory?: string;
  branch?: string;
  skillsPath?: string;
  config?: string;
  configFormat?: string;
  configUrl?: string;
  usageEnabled?: boolean;
  usageScript?: string;
  usageApiKey?: string;
  usageBaseUrl?: string;
  usageAccessToken?: string;
  usageUserId?: string;
  usageAutoInterval?: number;
}

export type DeepLinkImportResult =
  | { type: "provider"; id: string; result: { id: string } }
  | { type: "prompt"; id: string; result: { id: string } }
  | {
      type: "mcp";
      importedCount?: number;
      importedIds?: string[];
      failed?: Array<{ id: string; error: string }>;
      result: {
        importedCount: number;
        importedIds: string[];
        failed: Array<{ id: string; error: string }>;
      };
    }
  | {
      type: "skill";
      id?: string | null;
      key?: string;
      result: { key: string };
    };

export const deeplinkApi = {
  async parse(url: string): Promise<DeepLinkImportRequest> {
    return await invoke("parse_deeplink", { url });
  },

  async parseDeeplink(url: string): Promise<DeepLinkImportRequest> {
    return await this.parse(url);
  },

  async import(request: DeepLinkImportRequest): Promise<DeepLinkImportResult> {
    return await invoke("import_from_deeplink_unified", { request });
  },

  async mergeDeeplinkConfig(
    request: DeepLinkImportRequest,
  ): Promise<DeepLinkImportRequest> {
    return await invoke("merge_deeplink_config", { request });
  },

  async importFromDeeplink(
    request: DeepLinkImportRequest,
  ): Promise<DeepLinkImportResult> {
    return await this.import(request);
  },
};
