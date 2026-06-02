import { MutationCache, QueryCache, QueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

const ERROR_TOAST_DEDUPE_MS = 15_000;
const MAX_ERROR_DETAIL_CHARS = 220;

const recentErrorToasts = new Map<string, number>();

const truncateDetail = (value: string): string => {
  if (value.length <= MAX_ERROR_DETAIL_CHARS) return value;
  return `${value.slice(0, MAX_ERROR_DETAIL_CHARS).trimEnd()}...`;
};

const errorMessage = (error: unknown): string => {
  if (error instanceof Error && error.message.trim()) {
    return truncateDetail(error.message.trim());
  }
  if (typeof error === "string" && error.trim()) {
    return truncateDetail(error.trim());
  }
  return "Unknown API error";
};

const notifyApiError = (scope: string, error: unknown) => {
  const message = errorMessage(error);
  const key = `${scope}:${message}`;
  const now = Date.now();
  const lastShown = recentErrorToasts.get(key) ?? 0;
  if (now - lastShown < ERROR_TOAST_DEDUPE_MS) {
    return;
  }
  recentErrorToasts.set(key, now);
  toast.error("API request failed", {
    description: message,
  });
};

export function createAppQueryClient() {
  return new QueryClient({
    queryCache: new QueryCache({
      onError: (error, query) => {
        if (query.meta?.suppressErrorToast) {
          return;
        }
        notifyApiError(query.queryHash, error);
      },
    }),
    mutationCache: new MutationCache({
      onError: (error, _variables, _context, mutation) => {
        if (mutation.options.onError) {
          return;
        }
        notifyApiError(
          String(mutation.options.mutationKey ?? "mutation"),
          error,
        );
      },
    }),
    defaultOptions: {
      queries: {
        retry: 1,
        refetchOnWindowFocus: false,
        staleTime: 1000 * 60 * 5,
      },
      mutations: {
        retry: false,
      },
    },
  });
}

export const queryClient = createAppQueryClient();
