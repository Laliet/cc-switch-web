import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { backupsApi } from "@/lib/api";

export function useBackupManager() {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: ["database-backups"],
    queryFn: backupsApi.listDbBackups,
  });

  const create = useMutation({
    mutationFn: backupsApi.createDbBackup,
    onSuccess: () => query.refetch(),
  });
  const restore = useMutation({
    mutationFn: backupsApi.restoreDbBackup,
    onSuccess: async () => {
      await queryClient.invalidateQueries();
      await query.refetch();
    },
  });
  const rename = useMutation({
    mutationFn: ({
      oldFilename,
      newName,
    }: {
      oldFilename: string;
      newName: string;
    }) => backupsApi.renameDbBackup(oldFilename, newName),
    onSuccess: () => query.refetch(),
  });
  const remove = useMutation({
    mutationFn: backupsApi.deleteDbBackup,
    onSuccess: () => query.refetch(),
  });

  return {
    backups: query.data ?? [],
    isLoading: query.isLoading,
    create: create.mutateAsync,
    restore: restore.mutateAsync,
    rename: rename.mutateAsync,
    remove: remove.mutateAsync,
    isBusy:
      create.isPending ||
      restore.isPending ||
      rename.isPending ||
      remove.isPending,
  };
}
