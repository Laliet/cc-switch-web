import { beforeEach, describe, expect, it, vi } from "vitest";

import { openclawApi } from "@/lib/api/openclaw";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api/adapter", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("openclawApi", () => {
  beforeEach(() => invokeMock.mockReset());

  it("reads the live configuration sections", async () => {
    invokeMock.mockResolvedValue(undefined);

    await openclawApi.getStatus();
    await openclawApi.getRawConfig();
    await openclawApi.getProviders();
    await openclawApi.getProvider("provider/a");
    await openclawApi.getDefaultModel();
    await openclawApi.getModelCatalog();
    await openclawApi.getAgentsDefaults();
    await openclawApi.getEnv();
    await openclawApi.getTools();
    await openclawApi.getHealth();

    expect(invokeMock.mock.calls).toEqual([
      ["get_openclaw_status"],
      ["get_openclaw_raw_config"],
      ["get_openclaw_live_providers"],
      ["get_openclaw_live_provider", { providerId: "provider/a" }],
      ["get_openclaw_default_model"],
      ["get_openclaw_model_catalog"],
      ["get_openclaw_agents_defaults"],
      ["get_openclaw_env"],
      ["get_openclaw_tools"],
      ["scan_openclaw_config_health"],
    ]);
  });

  it("passes the loaded ETag to every configuration write", async () => {
    invokeMock.mockResolvedValue(undefined);
    const expectedEtag = "etag-1";

    await openclawApi.setRawConfig("{ models: {} }", expectedEtag);
    await openclawApi.setDefaultModel(
      { primary: "openai/gpt-5", fallbacks: ["openai/gpt-4.1"] },
      expectedEtag,
    );
    await openclawApi.clearDefaultModel(expectedEtag);
    await openclawApi.setModelCatalog(
      { "openai/gpt-5": { alias: "main" } },
      expectedEtag,
    );
    await openclawApi.setAgentsDefaults(
      { workspace: "/srv/openclaw" },
      expectedEtag,
    );
    await openclawApi.setEnv({ OPENAI_API_KEY: "secret" }, expectedEtag);
    await openclawApi.setTools(
      { profile: "coding", deny: ["browser"] },
      expectedEtag,
    );

    expect(invokeMock.mock.calls).toEqual([
      ["set_openclaw_raw_config", { source: "{ models: {} }", expectedEtag }],
      [
        "set_openclaw_default_model",
        {
          model: {
            primary: "openai/gpt-5",
            fallbacks: ["openai/gpt-4.1"],
          },
          expectedEtag,
        },
      ],
      ["clear_openclaw_default_model", { expectedEtag }],
      [
        "set_openclaw_model_catalog",
        {
          catalog: { "openai/gpt-5": { alias: "main" } },
          expectedEtag,
        },
      ],
      [
        "set_openclaw_agents_defaults",
        { defaults: { workspace: "/srv/openclaw" }, expectedEtag },
      ],
      ["set_openclaw_env", { env: { OPENAI_API_KEY: "secret" }, expectedEtag }],
      [
        "set_openclaw_tools",
        { tools: { profile: "coding", deny: ["browser"] }, expectedEtag },
      ],
    ]);
  });

  it("previews and applies provider reconciliation against one snapshot", async () => {
    invokeMock.mockResolvedValue(undefined);

    await openclawApi.previewReconciliation();
    await openclawApi.applyReconciliation(
      ["external-a", "external-b"],
      true,
      "etag-2",
    );
    await openclawApi.importLiveProviders();

    expect(invokeMock.mock.calls).toEqual([
      ["preview_openclaw_provider_reconciliation"],
      [
        "apply_openclaw_provider_reconciliation",
        {
          providerIds: ["external-a", "external-b"],
          updateExisting: true,
          expectedEtag: "etag-2",
        },
      ],
      ["import_openclaw_providers_from_live"],
    ]);
  });
});
