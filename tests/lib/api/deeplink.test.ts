import { beforeEach, describe, expect, it, vi } from "vitest";
import { deeplinkApi } from "@/lib/api/deeplink";
import type { DeepLinkImportRequest } from "@/lib/api/deeplink";
import type { AppId } from "@/lib/api/types";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api/adapter", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("deeplink API module", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("parseDeeplink delegates to invoke", async () => {
    const url = "ccswitch://import";
    const payload = {
      version: "1",
      resource: "provider",
      app: "claude",
      name: "Example",
      homepage: "https://example.com",
      endpoint: "https://api.example.com",
      apiKey: "token",
    };
    invokeMock.mockResolvedValueOnce(payload);

    const result = await deeplinkApi.parseDeeplink(url);

    expect(result).toEqual(payload);
    expect(invokeMock).toHaveBeenCalledWith("parse_deeplink", { url });
  });

  it("importFromDeeplink delegates to invoke", async () => {
    const appId: AppId = "codex";
    const request: DeepLinkImportRequest = {
      version: "1",
      resource: "provider",
      app: appId,
      name: "Codex Provider",
      homepage: "https://codex.example",
      endpoint: "https://api.codex.example",
      apiKey: "secret",
      model: "gpt-4o",
    };
    invokeMock.mockResolvedValueOnce({
      type: "provider",
      id: "provider-id",
      result: { id: "provider-id" },
    });

    const result = await deeplinkApi.importFromDeeplink(request);

    expect(result).toEqual({
      type: "provider",
      id: "provider-id",
      result: { id: "provider-id" },
    });
    expect(invokeMock).toHaveBeenCalledWith("import_from_deeplink_unified", {
      request,
    });
  });

  it("mergeDeeplinkConfig delegates to invoke", async () => {
    const request: DeepLinkImportRequest = {
      version: "v1",
      resource: "provider",
      app: "claude",
      name: "Claude Provider",
      config: "eyJlbnYiOnsiQU5USFJPUElDX0JBU0VfVVJMIjoiaHR0cHM6Ly9hcGkuZXhhbXBsZSJ9fQ",
      configFormat: "json",
    };
    const merged = {
      ...request,
      endpoint: "https://api.example",
    };
    invokeMock.mockResolvedValueOnce(merged);

    const result = await deeplinkApi.mergeDeeplinkConfig(request);

    expect(result).toEqual(merged);
    expect(invokeMock).toHaveBeenCalledWith("merge_deeplink_config", {
      request,
    });
  });
});
