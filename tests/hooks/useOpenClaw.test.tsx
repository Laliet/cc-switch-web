import type { ReactNode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  openclawKeys,
  useApplyOpenClawReconciliation,
  useOpenClawAgents,
  useOpenClawEnv,
  useOpenClawReconciliation,
  useOpenClawRawConfig,
  useOpenClawStatus,
  useOpenClawTools,
  useSaveOpenClawAgents,
  useSaveOpenClawEnv,
  useSaveOpenClawRawConfig,
  useSaveOpenClawTools,
} from "@/hooks/useOpenClaw";

const apiMocks = vi.hoisted(() => ({
  getStatus: vi.fn(),
  getRawConfig: vi.fn(),
  getAgentsDefaults: vi.fn(),
  getEnv: vi.fn(),
  getTools: vi.fn(),
  previewReconciliation: vi.fn(),
  setAgentsDefaults: vi.fn(),
  setRawConfig: vi.fn(),
  setEnv: vi.fn(),
  setTools: vi.fn(),
  applyReconciliation: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  openclawApi: apiMocks,
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { queryClient, wrapper };
}

describe("useOpenClaw", () => {
  beforeEach(() => {
    Object.values(apiMocks).forEach((mock) => mock.mockReset());
  });

  it("loads each configuration-center query with a stable key", async () => {
    apiMocks.getStatus.mockResolvedValue({
      providers: [],
      warnings: [],
      etag: "1",
    });
    apiMocks.getRawConfig.mockResolvedValue({ value: "{}", etag: "1" });
    apiMocks.getAgentsDefaults.mockResolvedValue({ value: {}, etag: "1" });
    apiMocks.getEnv.mockResolvedValue({ value: {}, etag: "1" });
    apiMocks.getTools.mockResolvedValue({ value: {}, etag: "1" });
    apiMocks.previewReconciliation.mockResolvedValue({
      etag: "1",
      liveCount: 0,
      storedCount: 0,
      items: [],
    });
    const { wrapper } = createWrapper();

    const status = renderHook(() => useOpenClawStatus(), { wrapper });
    const raw = renderHook(() => useOpenClawRawConfig(), { wrapper });
    const agents = renderHook(() => useOpenClawAgents(), { wrapper });
    const env = renderHook(() => useOpenClawEnv(), { wrapper });
    const tools = renderHook(() => useOpenClawTools(), { wrapper });
    const reconciliation = renderHook(() => useOpenClawReconciliation(), {
      wrapper,
    });

    await waitFor(() => {
      expect(status.result.current.isSuccess).toBe(true);
      expect(raw.result.current.isSuccess).toBe(true);
      expect(agents.result.current.isSuccess).toBe(true);
      expect(env.result.current.isSuccess).toBe(true);
      expect(tools.result.current.isSuccess).toBe(true);
      expect(reconciliation.result.current.isSuccess).toBe(true);
    });
    expect(apiMocks.getStatus).toHaveBeenCalledTimes(1);
    expect(apiMocks.getRawConfig).toHaveBeenCalledTimes(1);
    expect(apiMocks.getAgentsDefaults).toHaveBeenCalledTimes(1);
    expect(apiMocks.getEnv).toHaveBeenCalledTimes(1);
    expect(apiMocks.getTools).toHaveBeenCalledTimes(1);
    expect(apiMocks.previewReconciliation).toHaveBeenCalledTimes(1);
  });

  it("saves all editable sections and invalidates OpenClaw queries", async () => {
    apiMocks.setRawConfig.mockResolvedValue({ warnings: [], etag: "2" });
    apiMocks.setAgentsDefaults.mockResolvedValue({ warnings: [], etag: "2" });
    apiMocks.setEnv.mockResolvedValue({ warnings: [], etag: "3" });
    apiMocks.setTools.mockResolvedValue({ warnings: [], etag: "4" });
    const { wrapper, queryClient } = createWrapper();
    const invalidate = vi
      .spyOn(queryClient, "invalidateQueries")
      .mockResolvedValue(undefined);
    const agents = renderHook(() => useSaveOpenClawAgents(), { wrapper });
    const raw = renderHook(() => useSaveOpenClawRawConfig(), { wrapper });
    const env = renderHook(() => useSaveOpenClawEnv(), { wrapper });
    const tools = renderHook(() => useSaveOpenClawTools(), { wrapper });

    await act(async () => {
      await raw.result.current.mutateAsync({
        source: "{ models: {} }",
        expectedEtag: "1",
      });
      await agents.result.current.mutateAsync({
        defaults: { workspace: "/workspace" },
        expectedEtag: "1",
      });
      await env.result.current.mutateAsync({
        env: { TOKEN: "value" },
        expectedEtag: "2",
      });
      await tools.result.current.mutateAsync({
        tools: { profile: "coding" },
        expectedEtag: "3",
      });
    });

    expect(apiMocks.setRawConfig).toHaveBeenCalledWith("{ models: {} }", "1");
    expect(apiMocks.setAgentsDefaults).toHaveBeenCalledWith(
      { workspace: "/workspace" },
      "1",
    );
    expect(apiMocks.setEnv).toHaveBeenCalledWith({ TOKEN: "value" }, "2");
    expect(apiMocks.setTools).toHaveBeenCalledWith({ profile: "coding" }, "3");
    expect(invalidate).toHaveBeenCalledTimes(5);
    expect(invalidate).toHaveBeenCalledWith({ queryKey: openclawKeys.all });
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["providers", "openclaw"],
    });
  });

  it("applies reconciliation and refreshes provider state", async () => {
    apiMocks.applyReconciliation.mockResolvedValue({
      imported: 1,
      updated: 0,
      unchanged: 0,
      ignored: 0,
      invalid: 0,
      etag: "2",
    });
    const { wrapper, queryClient } = createWrapper();
    const invalidate = vi
      .spyOn(queryClient, "invalidateQueries")
      .mockResolvedValue(undefined);
    const mutation = renderHook(() => useApplyOpenClawReconciliation(), {
      wrapper,
    });

    await act(async () => {
      await mutation.result.current.mutateAsync({
        providerIds: ["external"],
        expectedEtag: "1",
      });
    });

    expect(apiMocks.applyReconciliation).toHaveBeenCalledWith(
      ["external"],
      true,
      "1",
    );
    expect(invalidate).toHaveBeenCalledWith({ queryKey: openclawKeys.all });
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["providers", "openclaw"],
    });
  });
});
