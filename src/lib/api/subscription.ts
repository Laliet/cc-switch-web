import { invoke } from "./adapter";

export type SubscriptionProvider = "claude" | "codex" | "gemini";

export interface QuotaWindow {
  name: string;
  used?: number | null;
  remaining?: number | null;
  total?: number | null;
  resetAt?: string | null;
}

export interface SubscriptionQuota {
  provider: SubscriptionProvider;
  accountId?: string | null;
  accountLabel?: string | null;
  source: string;
  status: string;
  plan?: string | null;
  windows: QuotaWindow[];
  fetchedAt: number;
  expiresAt?: string | null;
  error?: string | null;
}

export const subscriptionApi = {
  query(
    provider: SubscriptionProvider,
    options: { accountId?: string; force?: boolean } = {},
  ): Promise<SubscriptionQuota> {
    return invoke("query_subscription_quota", {
      provider,
      accountId: options.accountId ?? null,
      force: options.force ?? false,
    });
  },
};
