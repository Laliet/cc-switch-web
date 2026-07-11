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

export const sessionsApi = {
  list(): Promise<SessionMeta[]> {
    return invoke("list_sessions");
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
