import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  openclawApi,
  type OpenClawAgentsDefaults,
  type OpenClawEnvConfig,
  type OpenClawToolsConfig,
} from "@/lib/api";

export const openclawKeys = {
  all: ["openclaw"] as const,
  status: ["openclaw", "status"] as const,
  raw: ["openclaw", "raw"] as const,
  agents: ["openclaw", "agents-defaults"] as const,
  env: ["openclaw", "env"] as const,
  tools: ["openclaw", "tools"] as const,
  reconciliation: ["openclaw", "reconciliation"] as const,
};

export function useOpenClawStatus(enabled = true) {
  return useQuery({
    queryKey: openclawKeys.status,
    queryFn: () => openclawApi.getStatus(),
    enabled,
  });
}

export function useOpenClawRawConfig(enabled = true) {
  return useQuery({
    queryKey: openclawKeys.raw,
    queryFn: () => openclawApi.getRawConfig(),
    enabled,
  });
}

export function useOpenClawAgents(enabled = true) {
  return useQuery({
    queryKey: openclawKeys.agents,
    queryFn: () => openclawApi.getAgentsDefaults(),
    enabled,
  });
}

export function useOpenClawEnv(enabled = true) {
  return useQuery({
    queryKey: openclawKeys.env,
    queryFn: () => openclawApi.getEnv(),
    enabled,
  });
}

export function useOpenClawTools(enabled = true) {
  return useQuery({
    queryKey: openclawKeys.tools,
    queryFn: () => openclawApi.getTools(),
    enabled,
  });
}

export function useOpenClawReconciliation(enabled = true) {
  return useQuery({
    queryKey: openclawKeys.reconciliation,
    queryFn: () => openclawApi.previewReconciliation(),
    enabled,
    staleTime: 0,
  });
}

function useInvalidateOpenClaw() {
  const queryClient = useQueryClient();
  return async (includeProviders = false) => {
    await queryClient.invalidateQueries({ queryKey: openclawKeys.all });
    if (includeProviders) {
      await queryClient.invalidateQueries({
        queryKey: ["providers", "openclaw"],
      });
    }
  };
}

export function useSaveOpenClawAgents() {
  const invalidate = useInvalidateOpenClaw();
  return useMutation({
    mutationFn: (input: {
      defaults: OpenClawAgentsDefaults;
      expectedEtag: string;
    }) => openclawApi.setAgentsDefaults(input.defaults, input.expectedEtag),
    onSuccess: () => invalidate(),
  });
}

export function useSaveOpenClawRawConfig() {
  const invalidate = useInvalidateOpenClaw();
  return useMutation({
    mutationFn: (input: { source: string; expectedEtag: string }) =>
      openclawApi.setRawConfig(input.source, input.expectedEtag),
    onSuccess: () => invalidate(true),
  });
}

export function useSaveOpenClawEnv() {
  const invalidate = useInvalidateOpenClaw();
  return useMutation({
    mutationFn: (input: { env: OpenClawEnvConfig; expectedEtag: string }) =>
      openclawApi.setEnv(input.env, input.expectedEtag),
    onSuccess: () => invalidate(),
  });
}

export function useSaveOpenClawTools() {
  const invalidate = useInvalidateOpenClaw();
  return useMutation({
    mutationFn: (input: { tools: OpenClawToolsConfig; expectedEtag: string }) =>
      openclawApi.setTools(input.tools, input.expectedEtag),
    onSuccess: () => invalidate(),
  });
}

export function useApplyOpenClawReconciliation() {
  const invalidate = useInvalidateOpenClaw();
  return useMutation({
    mutationFn: (input: { providerIds: string[]; expectedEtag: string }) =>
      openclawApi.applyReconciliation(
        input.providerIds,
        true,
        input.expectedEtag,
      ),
    onSuccess: () => invalidate(true),
  });
}
