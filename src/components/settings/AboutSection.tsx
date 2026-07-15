import { useCallback, useEffect, useState } from "react";
import { Download, ExternalLink, Info, Loader2, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { settingsApi } from "@/lib/api";
import { useUpdate } from "@/contexts/UpdateContext";
import { getCurrentVersion, relaunchApp } from "@/lib/updater";
import { useCapabilitiesQuery } from "@/lib/query";

interface AboutSectionProps {
  isPortable: boolean;
}

export function AboutSection({ isPortable }: AboutSectionProps) {
  const { t } = useTranslation();
  const [version, setVersion] = useState<string | null>(null);
  const [isLoadingVersion, setIsLoadingVersion] = useState(true);
  const [isDownloading, setIsDownloading] = useState(false);
  const { data: capabilities } = useCapabilitiesQuery();
  const appUpdateSupported = capabilities?.features.appUpdate === true;

  const {
    hasUpdate,
    updateInfo,
    updateHandle,
    checkUpdate,
    resetDismiss,
    isChecking,
  } = useUpdate();

  useEffect(() => {
    let active = true;
    const load = async () => {
      try {
        const loaded = await getCurrentVersion();
        if (active) {
          setVersion(loaded);
        }
      } catch (error) {
        console.error("[AboutSection] Failed to get version", error);
        if (active) {
          setVersion(null);
        }
      } finally {
        if (active) {
          setIsLoadingVersion(false);
        }
      }
    };

    void load();
    return () => {
      active = false;
    };
  }, []);

  const handleOpenReleaseNotes = useCallback(async () => {
    try {
      const targetVersion = updateInfo?.availableVersion ?? version ?? "";
      const displayVersion = targetVersion.startsWith("v")
        ? targetVersion
        : targetVersion
          ? `v${targetVersion}`
          : "";

      if (!displayVersion) {
        await settingsApi.openExternal(
          "https://github.com/Laliet/cc-switch-web/releases",
        );
        return;
      }

      await settingsApi.openExternal(
        `https://github.com/Laliet/cc-switch-web/releases/tag/${displayVersion}`,
      );
    } catch (error) {
      console.error("[AboutSection] Failed to open release notes", error);
      toast.error(t("settings.openReleaseNotesFailed"));
    }
  }, [t, updateInfo?.availableVersion, version]);

  const handleCheckUpdate = useCallback(async () => {
    if (hasUpdate && updateHandle) {
      if (isPortable) {
        try {
          await settingsApi.checkUpdates();
        } catch (error) {
          console.error("[AboutSection] Portable update failed", error);
        }
        return;
      }

      setIsDownloading(true);
      try {
        resetDismiss();
        await updateHandle.downloadAndInstall();
        await relaunchApp();
      } catch (error) {
        console.error("[AboutSection] Update failed", error);
        toast.error(t("settings.updateFailed"));
        try {
          await settingsApi.checkUpdates();
        } catch (fallbackError) {
          console.error(
            "[AboutSection] Failed to open fallback updater",
            fallbackError,
          );
        }
      } finally {
        setIsDownloading(false);
      }
      return;
    }

    const result = await checkUpdate();
    if (result === "up-to-date") {
      toast.success(t("settings.upToDate"));
      return;
    }
    if (result === "error") {
      toast.error(t("settings.checkUpdateFailed"));
    }
  }, [checkUpdate, hasUpdate, isPortable, resetDismiss, t, updateHandle]);

  const displayVersion = version || t("common.unknown");

  return (
    <section className="space-y-4">
      <header className="space-y-1">
        <h3 className="text-sm font-medium">{t("common.about")}</h3>
        <p className="text-xs text-muted-foreground">
          {t("settings.aboutHint")}
        </p>
      </header>

      <div className="space-y-4 rounded-lg border border-border-default p-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="space-y-1">
            <p className="text-sm font-medium text-foreground">CC Switch</p>
            <p className="text-xs text-muted-foreground">
              {t("common.version")}{" "}
              {isLoadingVersion ? (
                <Loader2 className="inline h-3 w-3 animate-spin" />
              ) : (
                `v${displayVersion}`
              )}
            </p>
            {isPortable ? (
              <p className="inline-flex items-center gap-1 text-xs text-muted-foreground">
                <Info className="h-3 w-3" />
                {t("settings.portableMode")}
              </p>
            ) : null}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={handleOpenReleaseNotes}
            >
              <ExternalLink className="mr-2 h-4 w-4" />
              {t("settings.releaseNotes")}
            </Button>
            {appUpdateSupported ? (
              <Button
                type="button"
                size="sm"
                onClick={handleCheckUpdate}
                disabled={isChecking || isDownloading}
                className="min-w-[140px]"
              >
                {isDownloading ? (
                  <span className="inline-flex items-center gap-2">
                    <Loader2 className="h-4 w-4 animate-spin" />
                    {t("settings.updating")}
                  </span>
                ) : hasUpdate ? (
                  <span className="inline-flex items-center gap-2">
                    <Download className="h-4 w-4" />
                    {t("settings.updateTo", {
                      version: updateInfo?.availableVersion ?? "",
                    })}
                  </span>
                ) : isChecking ? (
                  <span className="inline-flex items-center gap-2">
                    <RefreshCw className="h-4 w-4 animate-spin" />
                    {t("settings.checking")}
                  </span>
                ) : (
                  t("settings.checkForUpdates")
                )}
              </Button>
            ) : null}
          </div>
        </div>

        {hasUpdate && updateInfo ? (
          <div className="rounded-md bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
            <p>
              {t("settings.updateAvailable", {
                version: updateInfo.availableVersion,
              })}
            </p>
            {updateInfo.notes ? (
              <p className="mt-1 line-clamp-3">{updateInfo.notes}</p>
            ) : null}
          </div>
        ) : null}
      </div>
    </section>
  );
}
