import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  Check,
  DatabaseBackup,
  Pencil,
  RotateCcw,
  Trash2,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useBackupManager } from "@/hooks/useBackupManager";

interface BackupListSectionProps {
  intervalHours?: number;
  retainCount?: number;
  onSettingsChange: (updates: {
    backupIntervalHours?: number;
    backupRetainCount?: number;
  }) => void;
}

const formatBytes = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
};

export function BackupListSection({
  intervalHours,
  retainCount,
  onSettingsChange,
}: BackupListSectionProps) {
  const { t } = useTranslation();
  const { backups, isLoading, create, restore, rename, remove, isBusy } =
    useBackupManager();
  const [restoreTarget, setRestoreTarget] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [name, setName] = useState("");

  const run = async (action: () => Promise<unknown>, success: string) => {
    try {
      await action();
      toast.success(success);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <section className="space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium">
            {t("settings.backupManager.title")}
          </h3>
          <p className="text-xs text-muted-foreground">
            {t("settings.backupManager.description")}
          </p>
        </div>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={isBusy}
          onClick={() =>
            void run(() => create(), t("settings.backupManager.createSuccess"))
          }
        >
          <DatabaseBackup className="h-4 w-4" />
          {t("settings.backupManager.create")}
        </Button>
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <div className="space-y-2">
          <Label>{t("settings.backupManager.interval")}</Label>
          <Select
            value={String(intervalHours ?? 24)}
            onValueChange={(value) =>
              onSettingsChange({ backupIntervalHours: Number(value) })
            }
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="0">
                {t("settings.backupManager.disabled")}
              </SelectItem>
              {[6, 12, 24, 48, 168].map((hours) => (
                <SelectItem key={hours} value={String(hours)}>
                  {t("settings.backupManager.hours", { hours })}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-2">
          <Label>{t("settings.backupManager.retention")}</Label>
          <Select
            value={String(retainCount ?? 10)}
            onValueChange={(value) =>
              onSettingsChange({ backupRetainCount: Number(value) })
            }
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {[3, 5, 10, 15, 20, 30, 50].map((count) => (
                <SelectItem key={count} value={String(count)}>
                  {count}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      <div className="divide-y rounded-md border border-border-default">
        {isLoading ? (
          <p className="px-3 py-4 text-sm text-muted-foreground">
            {t("common.loading")}
          </p>
        ) : backups.length === 0 ? (
          <p className="px-3 py-4 text-sm text-muted-foreground">
            {t("settings.backupManager.empty")}
          </p>
        ) : (
          backups.map((backup) => (
            <div
              key={backup.filename}
              className="flex items-center gap-3 px-3 py-2"
            >
              <div className="min-w-0 flex-1">
                {editing === backup.filename ? (
                  <Input
                    value={name}
                    onChange={(event) => setName(event.target.value)}
                  />
                ) : (
                  <p className="truncate text-sm font-medium">
                    {backup.filename}
                  </p>
                )}
                <p className="text-xs text-muted-foreground">
                  {new Date(backup.createdAt).toLocaleString()} ·{" "}
                  {formatBytes(backup.sizeBytes)}
                </p>
              </div>
              {editing === backup.filename ? (
                <>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    disabled={isBusy || !name.trim()}
                    title={t("common.save")}
                    onClick={() =>
                      void run(async () => {
                        await rename({
                          oldFilename: backup.filename,
                          newName: name,
                        });
                        setEditing(null);
                      }, t("settings.backupManager.renameSuccess"))
                    }
                  >
                    <Check className="h-4 w-4" />
                  </Button>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    title={t("common.cancel")}
                    onClick={() => setEditing(null)}
                  >
                    <X className="h-4 w-4" />
                  </Button>
                </>
              ) : (
                <>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    title={t("common.edit")}
                    onClick={() => {
                      setEditing(backup.filename);
                      setName(backup.filename.replace(/\.db$/, ""));
                    }}
                  >
                    <Pencil className="h-4 w-4" />
                  </Button>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    title={t("settings.backupManager.restore")}
                    onClick={() => setRestoreTarget(backup.filename)}
                  >
                    <RotateCcw className="h-4 w-4" />
                  </Button>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    title={t("common.delete")}
                    onClick={() => setDeleteTarget(backup.filename)}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </>
              )}
            </div>
          ))
        )}
      </div>

      <ConfirmDialog
        isOpen={restoreTarget !== null}
        title={t("settings.backupManager.restoreTitle")}
        message={t("settings.backupManager.restoreMessage", {
          filename: restoreTarget,
        })}
        confirmText={t("settings.backupManager.restore")}
        onCancel={() => setRestoreTarget(null)}
        onConfirm={() => {
          const target = restoreTarget;
          setRestoreTarget(null);
          if (target)
            void run(
              () => restore(target),
              t("settings.backupManager.restoreSuccess"),
            );
        }}
      />
      <ConfirmDialog
        isOpen={deleteTarget !== null}
        title={t("settings.backupManager.deleteTitle")}
        message={t("settings.backupManager.deleteMessage", {
          filename: deleteTarget,
        })}
        confirmText={t("common.delete")}
        onCancel={() => setDeleteTarget(null)}
        onConfirm={() => {
          const target = deleteTarget;
          setDeleteTarget(null);
          if (target)
            void run(
              () => remove(target),
              t("settings.backupManager.deleteSuccess"),
            );
        }}
      />
    </section>
  );
}
