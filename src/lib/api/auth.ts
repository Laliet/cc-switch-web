import { invoke } from "./adapter";

export type ManagedAuthProvider = "github_copilot" | "codex_oauth";

export interface ManagedAuthTokenSet {
  accessToken: string;
  refreshToken?: string | null;
  expiresAt?: string | null;
  scope?: string | null;
  tokenType?: string | null;
}

export interface ManagedAuthAccount {
  id: string;
  provider: ManagedAuthProvider;
  label: string;
  username?: string | null;
  avatarUrl?: string | null;
  plan?: string | null;
  isDefault: boolean;
  createdAt: string;
  updatedAt: string;
  lastUsedAt?: string | null;
  expiresAt?: string | null;
  scopes?: string | null;
  status?: string | null;
}

export interface ManagedAuthAccountInput {
  provider: ManagedAuthProvider;
  id?: string | null;
  label: string;
  username?: string | null;
  avatarUrl?: string | null;
  plan?: string | null;
  makeDefault?: boolean;
  tokens: ManagedAuthTokenSet;
}

export interface ManagedAuthDeviceStart {
  provider: ManagedAuthProvider;
}

export interface ManagedAuthDeviceSession {
  provider: ManagedAuthProvider;
  sessionId: string;
  userCode: string;
  verificationUri: string;
  verificationUriComplete?: string | null;
  intervalSeconds: number;
  expiresAt: string;
}

export interface ManagedAuthDevicePoll {
  provider: ManagedAuthProvider;
  sessionId: string;
}

export interface ManagedAuthDevicePollResult {
  status: string;
  account?: ManagedAuthAccount | null;
  message?: string | null;
}

export interface ManagedAuthUsage {
  provider: ManagedAuthProvider;
  accountId?: string | null;
  plan?: string | null;
  remaining?: number | null;
  used?: number | null;
  total?: number | null;
  resetAt?: string | null;
  raw?: unknown;
}

export const authApi = {
  async listAccounts(
    provider?: ManagedAuthProvider | null,
  ): Promise<ManagedAuthAccount[]> {
    return await invoke("list_managed_auth_accounts", {
      provider: provider || null,
    });
  },

  async importAccount(
    input: ManagedAuthAccountInput,
  ): Promise<ManagedAuthAccount> {
    return await invoke("import_managed_auth_account", { input });
  },

  async setDefault(
    provider: ManagedAuthProvider,
    accountId: string,
  ): Promise<boolean> {
    return await invoke("set_default_managed_auth_account", {
      provider,
      accountId,
    });
  },

  async deleteAccount(
    provider: ManagedAuthProvider,
    accountId: string,
  ): Promise<boolean> {
    return await invoke("delete_managed_auth_account", {
      provider,
      accountId,
    });
  },

  async logout(
    provider: ManagedAuthProvider,
    accountId: string,
  ): Promise<boolean> {
    return await invoke("logout_managed_auth_account", {
      provider,
      accountId,
    });
  },

  async startDeviceLogin(
    request: ManagedAuthDeviceStart,
  ): Promise<ManagedAuthDeviceSession> {
    return await invoke("start_managed_auth_device_login", { request });
  },

  async pollDeviceLogin(
    request: ManagedAuthDevicePoll,
  ): Promise<ManagedAuthDevicePollResult> {
    return await invoke("poll_managed_auth_device_login", { request });
  },

  async queryUsage(
    provider: ManagedAuthProvider,
    accountId?: string | null,
  ): Promise<ManagedAuthUsage> {
    return await invoke("query_managed_auth_usage", {
      provider,
      accountId: accountId || null,
    });
  },
};
