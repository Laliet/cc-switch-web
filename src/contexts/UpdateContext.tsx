import React, {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
  useRef,
} from "react";
import type { UpdateInfo, UpdateHandle } from "../lib/updater";
import { checkForUpdate } from "../lib/updater";
import { useCapabilitiesQuery } from "@/lib/query";

interface UpdateContextValue {
  // 更新状态
  hasUpdate: boolean;
  updateInfo: UpdateInfo | null;
  updateHandle: UpdateHandle | null;
  isChecking: boolean;
  error: string | null;

  // 提示状态
  isDismissed: boolean;
  dismissUpdate: () => void;

  // 操作方法
  checkUpdate: () => Promise<"available" | "up-to-date" | "error" | "skipped">;
  resetDismiss: () => void;
}

const UpdateContext = createContext<UpdateContextValue | undefined>(undefined);

export function UpdateProvider({ children }: { children: React.ReactNode }) {
  const DISMISSED_VERSION_KEY = "ccswitch:update:dismissedVersion";
  const LEGACY_DISMISSED_KEY = "dismissedUpdateVersion"; // 兼容旧键

  const [hasUpdate, setHasUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateHandle, setUpdateHandle] = useState<UpdateHandle | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isDismissed, setIsDismissed] = useState(false);
  const { data: capabilities } = useCapabilitiesQuery();
  const appUpdateSupported = capabilities?.features.appUpdate === true;

  const safeGetItem = useCallback((key: string) => {
    if (typeof window === "undefined" || !window?.localStorage) return null;
    try {
      return window.localStorage.getItem(key);
    } catch (error) {
      console.warn(
        `[UpdateContext] Failed to read localStorage key ${key}:`,
        error,
      );
      return null;
    }
  }, []);

  const safeSetItem = useCallback((key: string, value: string) => {
    if (typeof window === "undefined" || !window?.localStorage) return;
    try {
      window.localStorage.setItem(key, value);
    } catch (error) {
      console.warn(
        `[UpdateContext] Failed to write localStorage key ${key}:`,
        error,
      );
    }
  }, []);

  const safeRemoveItem = useCallback((key: string) => {
    if (typeof window === "undefined" || !window?.localStorage) return;
    try {
      window.localStorage.removeItem(key);
    } catch (error) {
      console.warn(
        `[UpdateContext] Failed to remove localStorage key ${key}:`,
        error,
      );
    }
  }, []);

  const getDismissedVersion = useCallback(() => {
    let dismissedVersion = safeGetItem(DISMISSED_VERSION_KEY);
    if (!dismissedVersion) {
      const legacy = safeGetItem(LEGACY_DISMISSED_KEY);
      if (legacy) {
        safeSetItem(DISMISSED_VERSION_KEY, legacy);
        safeRemoveItem(LEGACY_DISMISSED_KEY);
        dismissedVersion = legacy;
      }
    }

    return dismissedVersion;
  }, [safeGetItem, safeRemoveItem, safeSetItem]);

  const persistDismissedVersion = useCallback(
    (version: string) => {
      safeSetItem(DISMISSED_VERSION_KEY, version);
      safeRemoveItem(LEGACY_DISMISSED_KEY);
    },
    [safeRemoveItem, safeSetItem],
  );

  const clearDismissedVersion = useCallback(() => {
    safeRemoveItem(DISMISSED_VERSION_KEY);
    safeRemoveItem(LEGACY_DISMISSED_KEY);
  }, [safeRemoveItem]);

  // 从 localStorage 读取已关闭的版本
  useEffect(() => {
    const current = updateInfo?.availableVersion;
    if (!current) return;

    const dismissedVersion = getDismissedVersion();
    setIsDismissed(dismissedVersion === current);
  }, [getDismissedVersion, updateInfo?.availableVersion]);

  const isCheckingRef = useRef(false);

  const checkUpdate = useCallback(async () => {
    if (!appUpdateSupported) return "skipped";
    if (isCheckingRef.current) return "skipped";
    isCheckingRef.current = true;
    setIsChecking(true);
    setError(null);

    try {
      const result = await checkForUpdate({ timeout: 30000 });

      if (result.status === "available") {
        setHasUpdate(true);
        setUpdateInfo(result.info);
        setUpdateHandle(result.update);

        const dismissedVersion = getDismissedVersion();
        setIsDismissed(dismissedVersion === result.info.availableVersion);
        return "available";
      } else {
        setHasUpdate(false);
        setUpdateInfo(null);
        setUpdateHandle(null);
        setIsDismissed(false);
        return "up-to-date";
      }
    } catch (err) {
      console.error("检查更新失败:", err);
      const message = err instanceof Error ? err.message : "检查更新失败";
      setError(message);
      setHasUpdate(false);
      return "error";
    } finally {
      setIsChecking(false);
      isCheckingRef.current = false;
    }
  }, [appUpdateSupported, getDismissedVersion]);

  const dismissUpdate = useCallback(() => {
    setIsDismissed(true);
    if (updateInfo?.availableVersion) {
      persistDismissedVersion(updateInfo.availableVersion);
    }
  }, [persistDismissedVersion, updateInfo?.availableVersion]);

  const resetDismiss = useCallback(() => {
    setIsDismissed(false);
    clearDismissedVersion();
  }, [clearDismissedVersion]);

  // 应用启动时自动检查更新
  useEffect(() => {
    if (!appUpdateSupported) return;
    // 延迟1秒后检查，避免影响启动体验
    const timer = setTimeout(() => {
      void checkUpdate();
    }, 1000);

    return () => clearTimeout(timer);
  }, [appUpdateSupported, checkUpdate]);

  const value: UpdateContextValue = {
    hasUpdate,
    updateInfo,
    updateHandle,
    isChecking,
    error,
    isDismissed,
    dismissUpdate,
    checkUpdate,
    resetDismiss,
  };

  return (
    <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>
  );
}

export function useUpdate() {
  const context = useContext(UpdateContext);
  if (!context) {
    throw new Error("useUpdate must be used within UpdateProvider");
  }
  return context;
}
