import { invoke } from "./adapter";

export interface WorkspaceFileInfo {
  name: string;
  exists: boolean;
  sizeBytes: number;
  modifiedAt?: number;
  etag?: string;
}

export interface WorkspaceFileContent {
  name: string;
  content: string;
  sizeBytes: number;
  modifiedAt: number;
  etag: string;
}

export interface WorkspaceWriteOutcome extends WorkspaceFileContent {
  backupId?: string;
}

export interface WorkspaceBackupInfo {
  id: string;
  sizeBytes: number;
  createdAt: number;
}

export interface DailyMemoryInfo {
  date: string;
  sizeBytes: number;
  modifiedAt: number;
  etag: string;
  preview: string;
}

export interface DailyMemorySearchResult {
  date: string;
  sizeBytes: number;
  modifiedAt: number;
  etag: string;
  snippet: string;
  matchCount: number;
}

export interface DailyMemoryDeleteOutcome {
  date: string;
  deleted: boolean;
  backupId?: string;
}

export const workspaceApi = {
  listFiles(): Promise<WorkspaceFileInfo[]> {
    return invoke("list_workspace_files");
  },
  readFile(name: string): Promise<WorkspaceFileContent> {
    return invoke("read_workspace_file", { filename: name });
  },
  writeFile(
    name: string,
    content: string,
    expectedEtag?: string,
  ): Promise<WorkspaceWriteOutcome> {
    return invoke("write_workspace_file", {
      filename: name,
      content,
      expectedEtag: expectedEtag ?? null,
    });
  },
  listBackups(name: string): Promise<WorkspaceBackupInfo[]> {
    return invoke("list_workspace_backups", { filename: name });
  },
  restoreBackup(
    name: string,
    backupId: string,
    expectedEtag?: string,
  ): Promise<WorkspaceWriteOutcome> {
    return invoke("restore_workspace_backup", {
      filename: name,
      backupId,
      expectedEtag: expectedEtag ?? null,
    });
  },
  listDailyMemory(): Promise<DailyMemoryInfo[]> {
    return invoke("list_daily_memory_files");
  },
  readDailyMemory(date: string): Promise<WorkspaceFileContent> {
    return invoke("read_daily_memory_file", { date });
  },
  writeDailyMemory(
    date: string,
    content: string,
    expectedEtag?: string,
  ): Promise<WorkspaceWriteOutcome> {
    return invoke("write_daily_memory_file", {
      date,
      content,
      expectedEtag: expectedEtag ?? null,
    });
  },
  searchDailyMemory(query: string): Promise<DailyMemorySearchResult[]> {
    return invoke("search_daily_memory_files", { query });
  },
  deleteDailyMemory(
    date: string,
    expectedEtag: string,
  ): Promise<DailyMemoryDeleteOutcome> {
    return invoke("delete_daily_memory_file", { date, expectedEtag });
  },
};
