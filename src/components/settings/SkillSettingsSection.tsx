import { useState } from "react";
import { Copy, Database, Link2, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { skillsApi } from "@/lib/api/skills";
import { cn } from "@/lib/utils";
import type { SkillStorageLocation, SkillSyncMethod } from "@/types";

interface SkillSettingsSectionProps {
  storageLocation?: SkillStorageLocation;
  syncMethod?: SkillSyncMethod;
  onChange: (updates: {
    skillStorageLocation?: SkillStorageLocation;
    skillSyncMethod?: SkillSyncMethod;
  }) => void;
}

const DEFAULT_STORAGE_LOCATION: SkillStorageLocation = "cc_switch";
const DEFAULT_SYNC_METHOD: SkillSyncMethod = "auto";

export function SkillSettingsSection({
  storageLocation = DEFAULT_STORAGE_LOCATION,
  syncMethod = DEFAULT_SYNC_METHOD,
  onChange,
}: SkillSettingsSectionProps) {
  const [pendingTarget, setPendingTarget] =
    useState<SkillStorageLocation | null>(null);
  const [isMigrating, setIsMigrating] = useState(false);

  const migrateStorage = async (target: SkillStorageLocation) => {
    setIsMigrating(true);
    setPendingTarget(null);
    try {
      const result = await skillsApi.migrateStorage(target);
      onChange({ skillStorageLocation: target });
      if (result.errors.length > 0) {
        toast.warning(
          `Skills 存储位置已切换，迁移 ${result.migratedCount} 个，${result.errors.length} 个需要手动检查。`,
        );
      } else {
        toast.success(
          `Skills 存储位置已切换，迁移 ${result.migratedCount} 个。`,
        );
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setIsMigrating(false);
    }
  };

  const selectStorage = (target: SkillStorageLocation) => {
    if (target === storageLocation || isMigrating) return;
    setPendingTarget(target);
  };

  return (
    <section className="space-y-5">
      <header className="space-y-1">
        <h3 className="text-sm font-medium">Skills</h3>
        <p className="text-xs text-muted-foreground">
          对齐上游 cc-switch 的 Skill 存储位置与应用同步方式。
        </p>
      </header>

      <div className="space-y-2">
        <div className="text-xs font-medium text-muted-foreground">
          存储位置
        </div>
        <div className="inline-flex gap-1 rounded-md border border-border-default bg-background p-1">
          <ChoiceButton
            active={storageLocation === "cc_switch"}
            disabled={isMigrating}
            onClick={() => selectStorage("cc_switch")}
          >
            <Database className="h-4 w-4" />
            CC Switch
          </ChoiceButton>
          <ChoiceButton
            active={storageLocation === "unified"}
            disabled={isMigrating}
            onClick={() => selectStorage("unified")}
          >
            {isMigrating && storageLocation !== "unified" ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Database className="h-4 w-4" />
            )}
            Unified
          </ChoiceButton>
        </div>
        <p className="text-xs text-muted-foreground">
          {storageLocation === "unified"
            ? "当前使用 ~/.agents/skills。"
            : "当前使用 ~/.cc-switch/skills。"}
        </p>
      </div>

      <div className="space-y-2">
        <div className="text-xs font-medium text-muted-foreground">
          同步方式
        </div>
        <div className="inline-flex gap-1 rounded-md border border-border-default bg-background p-1">
          <ChoiceButton
            active={syncMethod === "auto"}
            onClick={() => onChange({ skillSyncMethod: "auto" })}
          >
            <Link2 className="h-4 w-4" />
            Auto
          </ChoiceButton>
          <ChoiceButton
            active={syncMethod === "symlink"}
            onClick={() => onChange({ skillSyncMethod: "symlink" })}
          >
            <Link2 className="h-4 w-4" />
            Symlink
          </ChoiceButton>
          <ChoiceButton
            active={syncMethod === "copy"}
            onClick={() => onChange({ skillSyncMethod: "copy" })}
          >
            <Copy className="h-4 w-4" />
            Copy
          </ChoiceButton>
        </div>
      </div>

      <Dialog
        open={pendingTarget !== null}
        onOpenChange={(open) => {
          if (!open) setPendingTarget(null);
        }}
      >
        <DialogContent zIndex="alert" className="max-w-md">
          <DialogHeader>
            <DialogTitle>迁移 Skill 存储位置</DialogTitle>
            <DialogDescription>
              会先迁移 SSOT 文件，再写入设置，并刷新已安装客户端的同步目录。
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setPendingTarget(null)}
              disabled={isMigrating}
            >
              取消
            </Button>
            <Button
              onClick={() =>
                pendingTarget && void migrateStorage(pendingTarget)
              }
              disabled={isMigrating}
            >
              {isMigrating ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : null}
              迁移
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </section>
  );
}

function ChoiceButton({
  active,
  disabled,
  onClick,
  children,
}: {
  active: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Button
      type="button"
      size="sm"
      variant={active ? "default" : "ghost"}
      className={cn(
        "min-w-[104px] gap-2",
        active
          ? "shadow-sm"
          : "text-muted-foreground hover:bg-muted hover:text-foreground",
      )}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </Button>
  );
}
