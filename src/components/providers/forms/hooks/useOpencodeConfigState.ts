import { useCallback, useMemo, useState } from "react";
import type { OpenCodeModel } from "@/types";
import {
  OPENCODE_DEFAULT_CONFIG,
  OPENCODE_DEFAULT_NPM,
  isKnownOpencodeOptionKey,
  parseExtraOptions,
  parseOpencodeConfig,
} from "../helpers/opencodeFormUtils";

interface UseOpencodeConfigStateParams {
  initialData?: {
    settingsConfig?: Record<string, unknown>;
  };
  onSettingsConfigChange: (value: string) => void;
  getSettingsConfig: () => string;
}

export function useOpencodeConfigState({
  initialData,
  onSettingsConfigChange,
  getSettingsConfig,
}: UseOpencodeConfigStateParams) {
  const initialConfig = useMemo(
    () => parseOpencodeConfig(initialData?.settingsConfig),
    [initialData],
  );

  const [npm, setNpm] = useState(initialConfig.npm ?? OPENCODE_DEFAULT_NPM);
  const [apiKey, setApiKey] = useState(
    typeof initialConfig.options.apiKey === "string"
      ? initialConfig.options.apiKey
      : "",
  );
  const [baseUrl, setBaseUrl] = useState(
    typeof initialConfig.options.baseURL === "string"
      ? initialConfig.options.baseURL
      : "",
  );
  const [models, setModels] = useState<Record<string, OpenCodeModel>>(
    initialConfig.models,
  );
  const [extraOptions, setExtraOptions] = useState<Record<string, string>>(
    () => parseExtraOptions(initialConfig.options),
  );

  const updateConfig = useCallback(
    (updater: (config: Record<string, any>) => void) => {
      try {
        const config = JSON.parse(
          getSettingsConfig() || OPENCODE_DEFAULT_CONFIG,
        ) as Record<string, any>;
        updater(config);
        onSettingsConfigChange(JSON.stringify(config, null, 2));
      } catch {
        // The JSON editor owns parse errors; field edits are best-effort.
      }
    },
    [getSettingsConfig, onSettingsConfigChange],
  );

  const reset = useCallback(
    (value?: Record<string, unknown>) => {
      const parsed = parseOpencodeConfig(value);
      setNpm(parsed.npm ?? OPENCODE_DEFAULT_NPM);
      setApiKey(
        typeof parsed.options.apiKey === "string" ? parsed.options.apiKey : "",
      );
      setBaseUrl(
        typeof parsed.options.baseURL === "string"
          ? parsed.options.baseURL
          : "",
      );
      setModels(parsed.models);
      setExtraOptions(parseExtraOptions(parsed.options));
    },
    [],
  );

  const handleNpmChange = useCallback(
    (value: string) => {
      setNpm(value);
      updateConfig((config) => {
        config.npm = value.trim() || OPENCODE_DEFAULT_NPM;
      });
    },
    [updateConfig],
  );

  const handleApiKeyChange = useCallback(
    (value: string) => {
      setApiKey(value);
      updateConfig((config) => {
        config.options = config.options ?? {};
        config.options.apiKey = value.trim();
      });
    },
    [updateConfig],
  );

  const handleBaseUrlChange = useCallback(
    (value: string) => {
      const normalized = value.trim().replace(/\/+$/, "");
      setBaseUrl(normalized);
      updateConfig((config) => {
        config.options = config.options ?? {};
        config.options.baseURL = normalized;
      });
    },
    [updateConfig],
  );

  const handleModelsChange = useCallback(
    (value: Record<string, OpenCodeModel>) => {
      setModels(value);
      updateConfig((config) => {
        config.models = value;
      });
    },
    [updateConfig],
  );

  const handleExtraOptionsChange = useCallback(
    (value: Record<string, string>) => {
      setExtraOptions(value);
      updateConfig((config) => {
        config.options = config.options ?? {};
        for (const key of Object.keys(config.options)) {
          if (!isKnownOpencodeOptionKey(key)) {
            delete config.options[key];
          }
        }
        for (const [key, rawValue] of Object.entries(value)) {
          const trimmedKey = key.trim();
          if (!trimmedKey || trimmedKey.startsWith("option-")) continue;
          try {
            config.options[trimmedKey] = JSON.parse(rawValue);
          } catch {
            config.options[trimmedKey] = rawValue;
          }
        }
      });
    },
    [updateConfig],
  );

  return {
    npm,
    apiKey,
    baseUrl,
    models,
    extraOptions,
    reset,
    handleNpmChange,
    handleApiKeyChange,
    handleBaseUrlChange,
    handleModelsChange,
    handleExtraOptionsChange,
  };
}
