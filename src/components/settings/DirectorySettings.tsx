import { useMemo } from "react";
import { FolderSearch, Undo2 } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { useTranslation } from "react-i18next";
import type { DirectoryAppId } from "@/config/apps";
import type { ResolvedDirectories } from "@/hooks/useSettings";
import type { ConfigDirInfo } from "@/lib/api/settings";
import { useCapabilitiesQuery } from "@/lib/query";

interface DirectorySettingsProps {
  appConfigDir?: string;
  resolvedDirs: ResolvedDirectories;
  resolvedDirInfo: Partial<Record<DirectoryAppId, ConfigDirInfo>>;
  onAppConfigChange: (value?: string) => void;
  onBrowseAppConfig: () => Promise<void>;
  onResetAppConfig: () => Promise<void>;
  claudeDir?: string;
  codexDir?: string;
  geminiDir?: string;
  opencodeDir?: string;
  onDirectoryChange: (app: DirectoryAppId, value?: string) => void;
  onBrowseDirectory: (app: DirectoryAppId) => Promise<void>;
  onResetDirectory: (app: DirectoryAppId) => Promise<void>;
  onApplyWslTemplate: (distro?: string) => void;
}

export function DirectorySettings({
  appConfigDir,
  resolvedDirs,
  resolvedDirInfo,
  onAppConfigChange,
  onBrowseAppConfig,
  onResetAppConfig,
  claudeDir,
  codexDir,
  geminiDir,
  opencodeDir,
  onDirectoryChange,
  onBrowseDirectory,
  onResetDirectory,
  onApplyWslTemplate,
}: DirectorySettingsProps) {
  const { t } = useTranslation();
  const { data: capabilities } = useCapabilitiesQuery();
  const canBrowse = capabilities?.features.directoryPicker === true;
  const canOverride = capabilities?.features.configDirOverride === true;

  const handleApplyWslTemplate = () => {
    const distro = window.prompt(
      t("settings.wslDistroPrompt", {
        defaultValue: "请输入 WSL 发行版名称（例如 Ubuntu）",
      }),
      t("settings.wslDefaultDistro", {
        defaultValue: "Ubuntu",
      }),
    );
    if (distro === null) return;
    onApplyWslTemplate(distro);
  };

  return (
    <>
      {/* CC Switch 配置目录 - 独立区块 */}
      <section className="space-y-4">
        <header className="space-y-1">
          <h3 className="text-sm font-medium">{t("settings.appConfigDir")}</h3>
          <p className="text-xs text-muted-foreground">
            {t("settings.appConfigDirDescription")}
          </p>
        </header>

        <div className="flex items-center gap-2">
          <Input
            value={appConfigDir ?? resolvedDirs.appConfig ?? ""}
            placeholder={t("settings.browsePlaceholderApp")}
            className="font-mono text-xs"
            onChange={(event) => onAppConfigChange(event.target.value)}
            disabled={!canOverride}
          />
          {canBrowse ? (
            <Button
              type="button"
              variant="outline"
              size="icon"
              onClick={onBrowseAppConfig}
              title={t("settings.browseDirectory")}
            >
              <FolderSearch className="h-4 w-4" />
            </Button>
          ) : null}
          {canOverride ? (
            <Button
              type="button"
              variant="outline"
              size="icon"
              onClick={onResetAppConfig}
              title={t("settings.resetDefault")}
            >
              <Undo2 className="h-4 w-4" />
            </Button>
          ) : null}
        </div>
      </section>

      {/* Claude/Codex 配置目录 - 独立区块 */}
      <section className="space-y-4">
        <header className="space-y-1">
          <h3 className="text-sm font-medium">
            {t("settings.configDirectoryOverride")}
          </h3>
          <p className="text-xs text-muted-foreground">
            {t("settings.configDirectoryDescription")}
          </p>
        </header>

        <DirectoryInput
          label={t("settings.claudeConfigDir")}
          description={t("settings.claudeConfigDirDescription")}
          value={claudeDir}
          resolvedValue={resolvedDirs.claude}
          dirInfo={resolvedDirInfo.claude}
          placeholder={t("settings.browsePlaceholderClaude")}
          onChange={(val) => onDirectoryChange("claude", val)}
          onBrowse={() => onBrowseDirectory("claude")}
          onReset={() => onResetDirectory("claude")}
          canBrowse={canBrowse}
          canEdit={canOverride}
        />

        <DirectoryInput
          label={t("settings.codexConfigDir")}
          description={t("settings.codexConfigDirDescription")}
          value={codexDir}
          resolvedValue={resolvedDirs.codex}
          dirInfo={resolvedDirInfo.codex}
          placeholder={t("settings.browsePlaceholderCodex")}
          onChange={(val) => onDirectoryChange("codex", val)}
          onBrowse={() => onBrowseDirectory("codex")}
          onReset={() => onResetDirectory("codex")}
          canBrowse={canBrowse}
          canEdit={canOverride}
        />

        <DirectoryInput
          label={t("settings.geminiConfigDir")}
          description={t("settings.geminiConfigDirDescription")}
          value={geminiDir}
          resolvedValue={resolvedDirs.gemini}
          dirInfo={resolvedDirInfo.gemini}
          placeholder={t("settings.browsePlaceholderGemini")}
          onChange={(val) => onDirectoryChange("gemini", val)}
          onBrowse={() => onBrowseDirectory("gemini")}
          onReset={() => onResetDirectory("gemini")}
          canBrowse={canBrowse}
          canEdit={canOverride}
        />

        <DirectoryInput
          label={t("settings.opencodeConfigDir", {
            defaultValue: "OpenCode / OMO 配置目录",
          })}
          description={t("settings.opencodeConfigDirDescription", {
            defaultValue:
              "OpenCode 与 OMO 共用该目录，默认位于 ~/.config/opencode。",
          })}
          value={opencodeDir}
          resolvedValue={resolvedDirs.opencode}
          dirInfo={resolvedDirInfo.opencode}
          placeholder={t("settings.browsePlaceholderOpencode", {
            defaultValue: "/home/user/.config/opencode",
          })}
          onChange={(val) => onDirectoryChange("opencode", val)}
          onBrowse={() => onBrowseDirectory("opencode")}
          onReset={() => onResetDirectory("opencode")}
          canBrowse={canBrowse}
          canEdit={canOverride}
        />

        {canOverride ? (
          <>
            <p className="text-xs text-muted-foreground">
              {t("settings.wslShareHint")}
            </p>
            <div className="pt-1">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={handleApplyWslTemplate}
                className="text-xs"
              >
                {t("settings.fillWslTemplate", {
                  defaultValue: "填充 WSL 模板路径",
                })}
              </Button>
            </div>
          </>
        ) : null}
      </section>
    </>
  );
}

interface DirectoryInputProps {
  label: string;
  description?: string;
  value?: string;
  resolvedValue: string;
  dirInfo?: ConfigDirInfo;
  placeholder?: string;
  onChange: (value?: string) => void;
  onBrowse: () => Promise<void>;
  onReset: () => Promise<void>;
  canBrowse: boolean;
  canEdit: boolean;
}

function DirectoryInput({
  label,
  description,
  value,
  resolvedValue,
  dirInfo,
  placeholder,
  onChange,
  onBrowse,
  onReset,
  canBrowse,
  canEdit,
}: DirectoryInputProps) {
  const { t } = useTranslation();
  const displayValue = useMemo(
    () => value ?? resolvedValue ?? "",
    [value, resolvedValue],
  );

  return (
    <div className="space-y-1.5">
      <div className="space-y-1">
        <p className="text-xs font-medium text-foreground">{label}</p>
        {description ? (
          <p className="text-xs text-muted-foreground">{description}</p>
        ) : null}
      </div>
      <div className="flex items-center gap-2">
        <Input
          value={displayValue}
          placeholder={placeholder}
          className="font-mono text-xs"
          onChange={(event) => onChange(event.target.value)}
          disabled={!canEdit}
        />
        {canBrowse ? (
          <Button
            type="button"
            variant="outline"
            size="icon"
            onClick={onBrowse}
            title={t("settings.browseDirectory")}
          >
            <FolderSearch className="h-4 w-4" />
          </Button>
        ) : null}
        {canEdit ? (
          <Button
            type="button"
            variant="outline"
            size="icon"
            onClick={onReset}
            title={t("settings.resetDefault")}
          >
            <Undo2 className="h-4 w-4" />
          </Button>
        ) : null}
      </div>
      <p className="text-[11px] text-muted-foreground break-all">
        {t("settings.currentWriteTarget", {
          defaultValue: "当前实际写入路径：{{path}}",
          path: dirInfo?.dir || resolvedValue || "-",
        })}
      </p>
      {dirInfo?.homeMismatch && dirInfo.source !== "override" ? (
        <p className="text-[11px] text-amber-600 dark:text-amber-500 break-all">
          {t("settings.homeMismatchHint", {
            defaultValue:
              "检测到 Web 服务 HOME ({{serviceHome}}) 与账号 HOME ({{accountHome}}) 不一致；当前会写入 {{path}}。如需其他路径，请在这里显式覆盖。",
            serviceHome: dirInfo.serviceHome ?? "-",
            accountHome: dirInfo.accountHome ?? "-",
            path: dirInfo.dir,
          })}
        </p>
      ) : null}
    </div>
  );
}
