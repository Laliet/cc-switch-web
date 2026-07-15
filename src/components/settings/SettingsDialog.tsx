import { useCallback, useEffect, useMemo, useState } from "react";
import { Loader2, Save } from "lucide-react";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { settingsApi } from "@/lib/api";
import {
  getStoredWebUsername,
  getWebApiBase,
  isWeb,
  setWebCredentials,
} from "@/lib/api/adapter";
import { LanguageSettings } from "@/components/settings/LanguageSettings";
import { ThemeSettings } from "@/components/settings/ThemeSettings";
import { WindowSettings } from "@/components/settings/WindowSettings";
import { DirectorySettings } from "@/components/settings/DirectorySettings";
import { ImportExportSection } from "@/components/settings/ImportExportSection";
import { BackupListSection } from "@/components/settings/BackupListSection";
import { AboutSection } from "@/components/settings/AboutSection";
import { ProxySettingsSection } from "@/components/settings/ProxySettingsSection";
import { WebDavSettingsSection } from "@/components/settings/WebDavSettingsSection";
import { NetworkSettingsSection } from "@/components/settings/NetworkSettingsSection";
import { SkillSettingsSection } from "@/components/settings/SkillSettingsSection";
import { ModelTestConfigPanel } from "@/components/usage/ModelTestConfigPanel";
import { UniversalProvidersSection } from "@/components/settings/UniversalProvidersSection";
import { AuthCenterSection } from "@/components/settings/AuthCenterSection";
import { useSettings } from "@/hooks/useSettings";
import { useImportExport } from "@/hooks/useImportExport";
import { useTranslation } from "react-i18next";
import { useCapabilitiesQuery } from "@/lib/query";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImportSuccess?: () => void | Promise<void>;
  onProvidersChanged?: () => void | Promise<void>;
}

export function SettingsDialog({
  open,
  onOpenChange,
  onImportSuccess,
  onProvidersChanged,
}: SettingsDialogProps) {
  const { t } = useTranslation();
  const { data: capabilities } = useCapabilitiesQuery();
  const {
    settings,
    isLoading,
    isSaving,
    isPortable,
    appConfigDir,
    resolvedDirs,
    resolvedDirInfo,
    updateSettings,
    updateDirectory,
    updateAppConfigDir,
    browseDirectory,
    browseAppConfigDir,
    resetDirectory,
    resetAppConfigDir,
    applyWslTemplate,
    saveSettings,
    resetSettings,
    requiresRestart,
    acknowledgeRestart,
  } = useSettings();

  const {
    selectedFile,
    status: importStatus,
    errorMessage,
    backupId,
    isImporting,
    selectImportFile,
    importConfig,
    exportConfig,
    clearSelection,
    resetStatus,
  } = useImportExport({ onImportSuccess });

  const [activeTab, setActiveTab] = useState<string>("general");
  const [showRestartPrompt, setShowRestartPrompt] = useState(false);
  const [webUsername, setWebUsername] = useState("");
  const [webPassword, setWebPassword] = useState("");
  const [webPasswordConfirm, setWebPasswordConfirm] = useState("");
  const [isUpdatingWebCredentials, setIsUpdatingWebCredentials] =
    useState(false);

  useEffect(() => {
    if (open) {
      setActiveTab("general");
      resetStatus();
      if (isWeb()) {
        setWebUsername(getStoredWebUsername());
        setWebPassword("");
        setWebPasswordConfirm("");
      }
    }
  }, [open, resetStatus]);

  useEffect(() => {
    if (requiresRestart) {
      setShowRestartPrompt(true);
    }
  }, [requiresRestart]);

  const closeDialog = useCallback(() => {
    // 取消/直接关闭：恢复到初始设置（包括语言回滚）
    resetSettings();
    acknowledgeRestart();
    clearSelection();
    resetStatus();
    onOpenChange(false);
  }, [
    acknowledgeRestart,
    clearSelection,
    onOpenChange,
    resetSettings,
    resetStatus,
  ]);

  const closeAfterSave = useCallback(() => {
    // 保存成功后关闭：不再重置语言，避免需要“保存两次”才生效
    acknowledgeRestart();
    clearSelection();
    resetStatus();
    onOpenChange(false);
  }, [acknowledgeRestart, clearSelection, onOpenChange, resetStatus]);

  const handleDialogChange = useCallback(
    (nextOpen: boolean) => {
      if (!nextOpen) {
        closeDialog();
      } else {
        onOpenChange(true);
      }
    },
    [closeDialog, onOpenChange],
  );

  const handleCancel = useCallback(() => {
    closeDialog();
  }, [closeDialog]);

  const handleSave = useCallback(async () => {
    try {
      const result = await saveSettings();
      if (!result) return;
      if (result.requiresRestart) {
        setShowRestartPrompt(true);
        return;
      }
      closeAfterSave();
    } catch (error) {
      console.error("[SettingsDialog] Failed to save settings", error);
    }
  }, [closeAfterSave, saveSettings]);

  const handleRestartLater = useCallback(() => {
    setShowRestartPrompt(false);
    closeAfterSave();
  }, [closeAfterSave]);

  const handleRestartNow = useCallback(async () => {
    setShowRestartPrompt(false);
    if (import.meta.env.DEV) {
      toast.success(t("settings.devModeRestartHint"));
      closeAfterSave();
      return;
    }

    try {
      await settingsApi.restart();
    } catch (error) {
      console.error("[SettingsDialog] Failed to restart app", error);
      toast.error(t("settings.restartFailed"));
    } finally {
      closeAfterSave();
    }
  }, [closeAfterSave, t]);

  const isBusy = useMemo(() => isLoading && !settings, [isLoading, settings]);
  const showWebCredentials = useMemo(() => isWeb(), []);

  const handleUpdateWebCredentials = useCallback(async () => {
    if (isUpdatingWebCredentials) return;

    const trimmedUsername = webUsername.trim();
    const trimmedPassword = webPassword.trim();
    const trimmedConfirm = webPasswordConfirm.trim();
    if (!trimmedUsername || !trimmedPassword || !trimmedConfirm) {
      toast.error(
        t("settings.webCredentials.validation.required", {
          defaultValue: "请输入用户名和密码",
        }),
      );
      return;
    }
    if (trimmedUsername.includes(":")) {
      toast.error(
        t("settings.webCredentials.validation.invalidUsername", {
          defaultValue: "用户名不能包含 ':'",
        }),
      );
      return;
    }
    if (trimmedPassword !== trimmedConfirm) {
      toast.error(
        t("settings.webCredentials.validation.passwordMismatch", {
          defaultValue: "两次输入的密码不一致",
        }),
      );
      return;
    }
    if (trimmedPassword.length < 8) {
      toast.error(
        t("settings.webCredentials.validation.passwordTooShort", {
          defaultValue: "密码至少需要 8 个字符",
          min: 8,
        }),
      );
      return;
    }

    setIsUpdatingWebCredentials(true);
    try {
      const result = await settingsApi.updateWebCredentials(
        trimmedUsername,
        trimmedPassword,
      );
      if (!result) {
        throw new Error(
          t("settings.webCredentials.updateFailed", {
            defaultValue: "更新失败",
          }),
        );
      }
      setWebCredentials(trimmedUsername, trimmedPassword, getWebApiBase());
      setWebUsername(trimmedUsername);
      setWebPassword("");
      setWebPasswordConfirm("");
      toast.success(
        t("settings.webCredentials.updateSuccess", {
          defaultValue: "Web 登录凭据已更新",
        }),
      );
    } catch (error) {
      const message =
        error instanceof Error && error.message
          ? error.message
          : t("settings.webCredentials.updateFailed", {
              defaultValue: "更新失败",
            });
      toast.error(message);
    } finally {
      setIsUpdatingWebCredentials(false);
    }
  }, [isUpdatingWebCredentials, webPassword, webPasswordConfirm, webUsername]);

  return (
    <Dialog open={open} onOpenChange={handleDialogChange}>
      <DialogContent className="max-w-3xl max-h-[90vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>{t("settings.title")}</DialogTitle>
          <DialogDescription>
            {t("settings.description", {
              defaultValue: "管理语言、主题、目录等应用偏好。",
            })}
          </DialogDescription>
        </DialogHeader>

        {isBusy ? (
          <div className="flex min-h-[320px] items-center justify-center">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : (
          <div className="flex-1 overflow-y-auto px-6 py-4">
            <Tabs
              value={activeTab}
              onValueChange={setActiveTab}
              className="flex flex-col h-full"
            >
              <TabsList className="grid w-full grid-cols-5">
                <TabsTrigger value="general">
                  {t("settings.tabGeneral")}
                </TabsTrigger>
                <TabsTrigger value="auth">
                  {t("settings.tabAuth", { defaultValue: "Auth Center" })}
                </TabsTrigger>
                <TabsTrigger value="advanced">
                  {t("settings.tabAdvanced")}
                </TabsTrigger>
                <TabsTrigger value="stream">
                  {t("streamCheck.settingsTab", {
                    defaultValue: "Stream Check",
                  })}
                </TabsTrigger>
                <TabsTrigger value="about">{t("common.about")}</TabsTrigger>
              </TabsList>

              <TabsContent
                value="general"
                className="space-y-6 mt-6 min-h-[400px]"
              >
                {settings ? (
                  <>
                    <LanguageSettings
                      value={settings.language}
                      onChange={(lang) => updateSettings({ language: lang })}
                    />
                    <ThemeSettings />
                    {capabilities?.features.tray === true ||
                    capabilities?.features.claudePluginIntegration === true ? (
                      <WindowSettings
                        settings={settings}
                        onChange={updateSettings}
                      />
                    ) : null}
                  </>
                ) : null}
              </TabsContent>

              <TabsContent
                value="auth"
                className="space-y-6 mt-6 min-h-[400px]"
              >
                <AuthCenterSection />
              </TabsContent>

              <TabsContent
                value="advanced"
                className="space-y-6 mt-6 min-h-[400px]"
              >
                {settings ? (
                  <>
                    <DirectorySettings
                      appConfigDir={appConfigDir}
                      resolvedDirs={resolvedDirs}
                      resolvedDirInfo={resolvedDirInfo}
                      onAppConfigChange={updateAppConfigDir}
                      onBrowseAppConfig={browseAppConfigDir}
                      onResetAppConfig={resetAppConfigDir}
                      claudeDir={settings.claudeConfigDir}
                      codexDir={settings.codexConfigDir}
                      geminiDir={settings.geminiConfigDir}
                      opencodeDir={settings.opencodeConfigDir}
                      onDirectoryChange={updateDirectory}
                      onBrowseDirectory={browseDirectory}
                      onResetDirectory={resetDirectory}
                      onApplyWslTemplate={applyWslTemplate}
                    />
                    <ImportExportSection
                      status={importStatus}
                      selectedFile={selectedFile}
                      errorMessage={errorMessage}
                      backupId={backupId}
                      isImporting={isImporting}
                      onSelectFile={selectImportFile}
                      onImport={importConfig}
                      onExport={exportConfig}
                      onClear={clearSelection}
                    />
                    <BackupListSection
                      intervalHours={settings.backupIntervalHours}
                      retainCount={settings.backupRetainCount}
                      onSettingsChange={updateSettings}
                    />
                    <WebDavSettingsSection
                      value={settings.webDav}
                      onChange={(webDav) => updateSettings({ webDav })}
                    />
                    <SkillSettingsSection
                      storageLocation={settings.skillStorageLocation}
                      syncMethod={settings.skillSyncMethod}
                      onChange={updateSettings}
                    />
                    <NetworkSettingsSection
                      value={settings.network}
                      onChange={(network) => updateSettings({ network })}
                    />
                    {showWebCredentials ? (
                      <UniversalProvidersSection
                        onProvidersChanged={onProvidersChanged}
                      />
                    ) : null}
                    {showWebCredentials && settings.proxy ? (
                      <ProxySettingsSection
                        value={settings.proxy}
                        onChange={(proxy) => updateSettings({ proxy })}
                      />
                    ) : null}
                    {showWebCredentials ? (
                      <form
                        className="space-y-3"
                        onSubmit={(event) => {
                          event.preventDefault();
                          void handleUpdateWebCredentials();
                        }}
                      >
                        <div>
                          <h3 className="text-sm font-medium">
                            {t("settings.webCredentials.title", {
                              defaultValue: "Web 登录凭据",
                            })}
                          </h3>
                          <p className="text-xs text-muted-foreground">
                            {t("settings.webCredentials.description", {
                              defaultValue: "更新 Web 模式的用户名与密码。",
                            })}
                          </p>
                        </div>
                        <div className="grid gap-3 sm:grid-cols-2">
                          <div className="space-y-2">
                            <Label htmlFor="cc-switch-web-cred-username">
                              {t("settings.webCredentials.username", {
                                defaultValue: "用户名",
                              })}
                            </Label>
                            <Input
                              id="cc-switch-web-cred-username"
                              name="web-username"
                              type="text"
                              autoComplete="username"
                              value={webUsername}
                              onChange={(e) => setWebUsername(e.target.value)}
                              disabled={isUpdatingWebCredentials}
                            />
                          </div>
                          <div className="space-y-2">
                            <Label htmlFor="cc-switch-web-cred-password">
                              {t("settings.webCredentials.password", {
                                defaultValue: "密码",
                              })}
                            </Label>
                            <Input
                              id="cc-switch-web-cred-password"
                              name="web-password"
                              type="password"
                              autoComplete="new-password"
                              value={webPassword}
                              onChange={(e) => setWebPassword(e.target.value)}
                              disabled={isUpdatingWebCredentials}
                            />
                          </div>
                          <div className="space-y-2">
                            <Label htmlFor="cc-switch-web-cred-password-confirm">
                              {t("settings.webCredentials.confirmPassword", {
                                defaultValue: "确认密码",
                              })}
                            </Label>
                            <Input
                              id="cc-switch-web-cred-password-confirm"
                              name="web-password-confirm"
                              type="password"
                              autoComplete="new-password"
                              value={webPasswordConfirm}
                              onChange={(e) =>
                                setWebPasswordConfirm(e.target.value)
                              }
                              disabled={isUpdatingWebCredentials}
                            />
                          </div>
                        </div>
                        <div>
                          <Button
                            type="submit"
                            disabled={isUpdatingWebCredentials}
                          >
                            {isUpdatingWebCredentials
                              ? t("settings.webCredentials.updating", {
                                  defaultValue: "更新中...",
                                })
                              : t("settings.webCredentials.submit", {
                                  defaultValue: "更新凭据",
                                })}
                          </Button>
                        </div>
                      </form>
                    ) : null}
                  </>
                ) : null}
              </TabsContent>

              <TabsContent
                value="stream"
                className="space-y-6 mt-6 min-h-[400px]"
              >
                <ModelTestConfigPanel />
              </TabsContent>

              <TabsContent value="about" className="mt-6 min-h-[400px]">
                <AboutSection isPortable={isPortable} />
              </TabsContent>
            </Tabs>
          </div>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={handleCancel}>
            {t("common.cancel")}
          </Button>
          <Button onClick={handleSave} disabled={isSaving || isBusy}>
            {isSaving ? (
              <span className="inline-flex items-center gap-2">
                <Loader2 className="h-4 w-4 animate-spin" />
                {t("settings.saving")}
              </span>
            ) : (
              <>
                <Save className="mr-2 h-4 w-4" />
                {t("common.save")}
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>

      <Dialog
        open={showRestartPrompt}
        onOpenChange={(open) => !open && handleRestartLater()}
      >
        <DialogContent zIndex="alert" className="max-w-md">
          <DialogHeader>
            <DialogTitle>{t("settings.restartRequired")}</DialogTitle>
            <DialogDescription>
              {t("settings.restartRequiredDescription", {
                defaultValue: "某些更改需要重启应用后才会生效。",
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="px-6">
            <p className="text-sm text-muted-foreground">
              {t("settings.restartRequiredMessage")}
            </p>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={handleRestartLater}>
              {t("settings.restartLater")}
            </Button>
            <Button onClick={handleRestartNow}>
              {t("settings.restartNow")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Dialog>
  );
}
