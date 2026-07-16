import { invoke } from "./adapter";
import type { SessionMessage, SessionMeta } from "@/types";

export interface DeleteSessionOptions {
  providerId: string;
  sessionId: string;
  sourcePath: string;
}

export interface DeleteSessionResult extends DeleteSessionOptions {
  success: boolean;
  error?: string;
}

export interface SessionPage {
  sessions: SessionMeta[];
  nextCursor?: string;
  total: number;
  scannedAt: number;
}

export const sessionsApi = {
  list(refresh = false): Promise<SessionMeta[]> {
    return invoke("list_sessions", { refresh });
  },

  listPage(
    options: {
      cursor?: string;
      limit?: number;
      providerId?: string;
      query?: string;
      refresh?: boolean;
    } = {},
  ): Promise<SessionPage> {
    return invoke("list_sessions_page", options);
  },

  getMessages(
    providerId: string,
    sourcePath: string,
  ): Promise<SessionMessage[]> {
    return invoke("get_session_messages", { providerId, sourcePath });
  },

  delete(options: DeleteSessionOptions): Promise<boolean> {
    return invoke<boolean>("delete_session", { ...options });
  },

  deleteMany(items: DeleteSessionOptions[]): Promise<DeleteSessionResult[]> {
    return invoke("delete_sessions", { items });
  },
};
