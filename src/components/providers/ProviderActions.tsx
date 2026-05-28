import { Activity, BarChart3, Check, Edit, Play, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface ProviderActionsProps {
  isCurrent: boolean;
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
  onSwitch,
  onEdit,
  onConfigureUsage,
  onStreamCheck,
  isStreamChecking = false,
  onDelete,
  showUsageActions = true,
}: ProviderActionsProps) {
  const { t } = useTranslation();

  return (
    <div className="flex items-center gap-2">
      <Button
        size="sm"
        variant={isCurrent ? "secondary" : "default"}
        onClick={onSwitch}
        disabled={isCurrent}
        className={cn(
          "w-20",
          isCurrent &&
            "bg-gray-200 text-muted-foreground hover:bg-gray-200 hover:text-muted-foreground dark:bg-gray-700 dark:hover:bg-gray-700",
        )}
      >
        {isCurrent ? (
          <>
            <Check className="h-4 w-4" />
            {t("provider.inUse")}
          </>
        ) : (
          <>
            <Play className="h-4 w-4" />
            {t("provider.enable")}
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
          onClick={isCurrent ? undefined : onDelete}
          title={t("common.delete")}
          className={cn(
            !isCurrent && "hover:text-red-500 dark:hover:text-red-400",
            isCurrent && "opacity-40 cursor-not-allowed text-muted-foreground",
          )}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
