import { useCallback, useEffect, useState } from "react";
import { settingsApi } from "@/lib/api";
import { useCapabilitiesQuery } from "@/lib/query";

export interface UseSettingsMetadataResult {
  isPortable: boolean;
  requiresRestart: boolean;
  isLoading: boolean;
  acknowledgeRestart: () => void;
  setRequiresRestart: (value: boolean) => void;
}

/**
 * useSettingsMetadata - 元数据管理
 * 负责：
 * - isPortable（便携模式）
 * - requiresRestart（需要重启标志）
 */
export function useSettingsMetadata(): UseSettingsMetadataResult {
  const [isPortable, setIsPortable] = useState(false);
  const [requiresRestart, setRequiresRestart] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const { data: capabilities } = useCapabilitiesQuery();

  // 加载元数据
  useEffect(() => {
    if (!capabilities) return;
    if (!capabilities.features.portableMode) {
      setIsPortable(false);
      setIsLoading(false);
      return;
    }
    let active = true;
    setIsLoading(true);

    const load = async () => {
      try {
        const portable = await settingsApi.isPortable();

        if (!active) return;

        setIsPortable(portable);
      } catch (error) {
        console.error("[useSettingsMetadata] Failed to load metadata", error);
      } finally {
        if (active) {
          setIsLoading(false);
        }
      }
    };

    void load();
    return () => {
      active = false;
    };
  }, [capabilities]);

  const acknowledgeRestart = useCallback(() => {
    setRequiresRestart(false);
  }, []);

  return {
    isPortable,
    requiresRestart,
    isLoading,
    acknowledgeRestart,
    setRequiresRestart,
  };
}
