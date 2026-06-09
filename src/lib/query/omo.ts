import { useMutation } from "@tanstack/react-query";
import { providersApi } from "@/lib/api";
import type { AppId } from "@/lib/api";
import type { OmoLocalFileData } from "@/types/omo";

function splitOmoLiveSettings(
  appId: AppId,
  config: Record<string, unknown>,
): OmoLocalFileData {
  const { agents, categories, ...otherFields } = config;
  return {
    agents:
      agents && typeof agents === "object" && !Array.isArray(agents)
        ? (agents as Record<string, Record<string, unknown>>)
        : undefined,
    categories:
      categories && typeof categories === "object" && !Array.isArray(categories)
        ? (categories as Record<string, Record<string, unknown>>)
        : undefined,
    otherFields: Object.keys(otherFields).length > 0 ? otherFields : undefined,
    filePath:
      appId === "omo-slim"
        ? "oh-my-opencode-slim.jsonc"
        : "oh-my-openagent.jsonc",
  };
}

function createReadLocalFileMutation(appId: AppId) {
  return useMutation({
    mutationFn: async () => {
      const config = await providersApi.readLiveSettings(appId);
      return splitOmoLiveSettings(appId, config);
    },
  });
}

export const useReadOmoLocalFile = () => createReadLocalFileMutation("omo");
export const useReadOmoSlimLocalFile = () =>
  createReadLocalFileMutation("omo-slim");
