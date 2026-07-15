import { Activity, BarChart3, Check, Edit, Play, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface ProviderActionsProps {
  isCurrent: boolean;
  canDeleteCurrent?: boolean;
  switchMode?: "provider" | "default-model";
  onSwitch: () => void;
  onEdit: () => void;
  onConfigureUsage: () => void;
  onStreamCheck?: () => void;
  isStreamChecking?: boolean;
  onDelete: () => void;
  showUsageActions?: boolean;
}

export function ProviderActions({
  isCurrent,
  canDeleteCurrent = false,
  switchMode = "provider",
  onSwitch,
  onEdit,
  onConfigureUsage,
  onStreamCheck,
  isStreamChecking = false,
  onDelete,
  showUsageActions = true,
}: ProviderActionsProps) {
  const { t } = useTranslation();
  const deleteDisabled = isCurrent && !canDeleteCurrent;
  const currentLabel =
    switchMode === "default-model"
      ? t("openclaw.default", { defaultValue: "默认" })
      : t("provider.inUse");
  const switchLabel =
    switchMode === "default-model"
      ? t("openclaw.setDefault", { defaultValue: "设为默认" })
      : t("provider.enable");

  return (
    <div className="flex items-center gap-2">
      <Button
        size="sm"
        variant={isCurrent ? "secondary" : "default"}
        onClick={onSwitch}
        disabled={isCurrent}
        className={cn(
          switchMode === "default-model" ? "min-w-24" : "w-20",
          isCurrent &&
            "bg-gray-200 text-muted-foreground hover:bg-gray-200 hover:text-muted-foreground dark:bg-gray-700 dark:hover:bg-gray-700",
        )}
      >
        {isCurrent ? (
          <>
            <Check className="h-4 w-4" />
            {currentLabel}
          </>
        ) : (
          <>
            <Play className="h-4 w-4" />
            {switchLabel}
          </>
        )}
      </Button>

      <div className="flex items-center gap-1">
        <Button
          size="icon"
          variant="ghost"
          onClick={onEdit}
          title={t("common.edit")}
        >
          <Edit className="h-4 w-4" />
        </Button>

        {showUsageActions ? (
          <Button
            size="icon"
            variant="ghost"
            onClick={onConfigureUsage}
            title={t("provider.configureUsage")}
          >
            <BarChart3 className="h-4 w-4" />
          </Button>
        ) : null}

        {onStreamCheck ? (
          <Button
            size="icon"
            variant="ghost"
            onClick={onStreamCheck}
            disabled={isStreamChecking}
            aria-label={t("streamCheck.action", {
              defaultValue: "流式健康检查",
            })}
            title={t("streamCheck.action", {
              defaultValue: "流式健康检查",
            })}
          >
            <Activity
              className={cn("h-4 w-4", isStreamChecking && "animate-pulse")}
            />
          </Button>
        ) : null}

        <Button
          size="icon"
          variant="ghost"
          onClick={deleteDisabled ? undefined : onDelete}
          disabled={deleteDisabled}
          title={t("common.delete")}
          className={cn(
            !deleteDisabled && "hover:text-red-500 dark:hover:text-red-400",
            deleteDisabled &&
              "opacity-40 cursor-not-allowed text-muted-foreground",
          )}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
