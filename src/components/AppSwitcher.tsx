import type { AppId } from "@/lib/api";
import { useTranslation } from "react-i18next";
import { Blocks } from "lucide-react";
import { SWITCHER_APPS } from "@/config/apps";
import { ClaudeIcon, CodexIcon, GeminiIcon } from "./BrandIcons";
import { ProviderIcon } from "./ProviderIcon";

interface AppSwitcherProps {
  activeApp: AppId;
  onSwitch: (app: AppId) => void;
  availableApps?: readonly AppId[];
}

export function AppSwitcher({
  activeApp,
  onSwitch,
  availableApps,
}: AppSwitcherProps) {
  const { t } = useTranslation();
  const apps = availableApps
    ? SWITCHER_APPS.filter((app) => availableApps.includes(app.id))
    : SWITCHER_APPS;

  const handleSwitch = (app: AppId) => {
    if (app === activeApp) return;
    onSwitch(app);
  };

  const renderIcon = (appId: string, isActive: boolean) => {
    if (appId === "claude" || appId === "claude-desktop") {
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
    if (appId === "opencode") {
      return (
        <ProviderIcon
          name="opencode"
          size={16}
          showFallback={false}
          className={
            isActive
              ? "opacity-100 transition-opacity duration-200"
              : "opacity-60 transition-opacity duration-200 group-hover:opacity-100"
          }
        />
      );
    }
    if (appId === "openclaw") {
      return (
        <ProviderIcon
          name="openclaw"
          size={16}
          showFallback
          className={
            isActive ? "opacity-100" : "opacity-60 group-hover:opacity-100"
          }
        />
      );
    }
    return (
      <Blocks
        size={16}
        className={
          isActive
            ? "text-[#166534] dark:text-[#22C55E] transition-colors duration-200"
            : "text-gray-500 dark:text-gray-400 group-hover:text-[#166534] dark:group-hover:text-[#22C55E] transition-colors duration-200"
        }
      />
    );
  };

  return (
    <div className="max-w-full overflow-x-auto rounded-lg">
      <div className="inline-flex min-w-max gap-1 rounded-lg border border-transparent bg-gray-100 p-1 dark:bg-gray-800">
        {apps.map((app) => {
          const isActive = activeApp === app.id;

          return (
            <button
              key={app.id}
              type="button"
              onClick={() => handleSwitch(app.id)}
              className={`group inline-flex shrink-0 items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-all duration-200 ${
                isActive
                  ? "bg-white text-gray-900 shadow-sm dark:bg-gray-900 dark:text-gray-100 dark:shadow-none"
                  : "text-gray-500 hover:bg-white/50 hover:text-gray-900 dark:text-gray-400 dark:hover:bg-gray-800/60 dark:hover:text-gray-100"
              }`}
            >
              {renderIcon(app.id, isActive)}
              <span>{t(app.labelKey)}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
