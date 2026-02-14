import type { AppId } from "@/lib/api";
import { useTranslation } from "react-i18next";
import { Clock3 } from "lucide-react";
import { SWITCHER_APPS } from "@/config/apps";
import { ClaudeIcon, CodexIcon, GeminiIcon } from "./BrandIcons";

interface AppSwitcherProps {
  activeApp: AppId;
  onSwitch: (app: AppId) => void;
}

export function AppSwitcher({ activeApp, onSwitch }: AppSwitcherProps) {
  const { t } = useTranslation();

  const handleSwitch = (app: AppId) => {
    if (app === activeApp) return;
    onSwitch(app);
  };

  const renderIcon = (appId: string, isActive: boolean) => {
    if (appId === "claude") {
      return (
        <ClaudeIcon
          size={16}
          className={
            isActive
              ? "text-[#D97757] dark:text-[#D97757] transition-colors duration-200"
              : "text-gray-500 dark:text-gray-400 group-hover:text-[#D97757] dark:group-hover:text-[#D97757] transition-colors duration-200"
          }
        />
      );
    }
    if (appId === "codex") {
      return <CodexIcon size={16} />;
    }
    if (appId === "gemini") {
      return (
        <GeminiIcon
          size={16}
          className={
            isActive
              ? "text-[#4285F4] dark:text-[#4285F4] transition-colors duration-200"
              : "text-gray-500 dark:text-gray-400 group-hover:text-[#4285F4] dark:group-hover:text-[#4285F4] transition-colors duration-200"
          }
        />
      );
    }
    return <Clock3 size={14} className="text-gray-400 dark:text-gray-500" />;
  };

  return (
    <div className="inline-flex bg-gray-100 dark:bg-gray-800 rounded-lg p-1 gap-1 border border-transparent ">
      {SWITCHER_APPS.map((app) => {
        const isSupported = app.kind === "supported";
        const isActive = isSupported && activeApp === app.id;

        return (
          <button
            key={app.id}
            type="button"
            disabled={!isSupported}
            onClick={() => {
              if (isSupported) {
                handleSwitch(app.id);
              }
            }}
            className={`group inline-flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-all duration-200 ${
              isSupported
                ? isActive
                  ? "bg-white text-gray-900 shadow-sm dark:bg-gray-900 dark:text-gray-100 dark:shadow-none"
                  : "text-gray-500 hover:text-gray-900 hover:bg-white/50 dark:text-gray-400 dark:hover:text-gray-100 dark:hover:bg-gray-800/60"
                : "cursor-not-allowed text-gray-400 dark:text-gray-500 opacity-80"
            }`}
            title={
              !isSupported
                ? t("apps.comingSoon", { defaultValue: "Coming soon" })
                : undefined
            }
          >
            {renderIcon(app.id, Boolean(isActive))}
            <span>{t(app.labelKey)}</span>
            {!isSupported && (
              <span className="rounded bg-gray-200 px-1.5 py-0.5 text-[10px] text-gray-600 dark:bg-gray-700 dark:text-gray-300">
                {t("apps.comingSoon", { defaultValue: "Coming soon" })}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
