import { beforeEach, describe, expect, it, vi } from "vitest";
import { vscodeApi } from "@/lib/api/vscode";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api/adapter", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("vscode API module", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("getLiveProviderSettings requests live settings", async () => {
    const settings = { model: "claude-3" };
    invokeMock.mockResolvedValueOnce(settings);

    const result = await vscodeApi.getLiveProviderSettings("claude");

    expect(result).toEqual(settings);
    expect(invokeMock).toHaveBeenCalledWith("read_live_provider_settings", {
      app: "claude",
    });
  });

  it("testApiEndpoints sends urls and timeout", async () => {
    const payload = [{ url: "https://api.example.com", latency: 123 }];
    invokeMock.mockResolvedValueOnce(payload);

    const result = await vscodeApi.testApiEndpoints(
      ["https://api.example.com"],
      { timeoutSecs: 10 },
    );

    expect(result).toEqual(payload);
    expect(invokeMock).toHaveBeenCalledWith("test_api_endpoints", {
      urls: ["https://api.example.com"],
      timeoutSecs: 10,
    });
  });

  it("custom endpoint CRUD delegates to invoke", async () => {
    invokeMock.mockResolvedValueOnce([{ url: "https://custom" }]);

    const list = await vscodeApi.getCustomEndpoints("codex", "provider-1");

    expect(list).toEqual([{ url: "https://custom" }]);
    expect(invokeMock).toHaveBeenCalledWith("get_custom_endpoints", {
      app: "codex",
      providerId: "provider-1",
    });

    invokeMock.mockResolvedValueOnce(undefined);
    await vscodeApi.addCustomEndpoint("codex", "provider-1", "https://new");
    expect(invokeMock).toHaveBeenCalledWith("add_custom_endpoint", {
      app: "codex",
      providerId: "provider-1",
      url: "https://new",
    });

    invokeMock.mockResolvedValueOnce(undefined);
    await vscodeApi.removeCustomEndpoint("codex", "provider-1", "https://new");
    expect(invokeMock).toHaveBeenCalledWith("remove_custom_endpoint", {
      app: "codex",
      providerId: "provider-1",
      url: "https://new",
    });

    invokeMock.mockResolvedValueOnce(undefined);
    await vscodeApi.updateEndpointLastUsed(
      "codex",
      "provider-1",
      "https://new",
    );
    expect(invokeMock).toHaveBeenCalledWith("update_endpoint_last_used", {
      app: "codex",
      providerId: "provider-1",
      url: "https://new",
    });
  });

  it("file dialog operations delegate to invoke", async () => {
    invokeMock.mockResolvedValueOnce(true);
    await vscodeApi.exportConfigToFile("/tmp/config.json");
    expect(invokeMock).toHaveBeenCalledWith("export_config_to_file", {
      filePath: "/tmp/config.json",
    });

    invokeMock.mockResolvedValueOnce(true);
    await vscodeApi.importConfigFromFile("/tmp/config.json");
    expect(invokeMock).toHaveBeenCalledWith("import_config_from_file", {
      filePath: "/tmp/config.json",
    });

    invokeMock.mockResolvedValueOnce("/tmp/saved.json");
    const savePath = await vscodeApi.saveFileDialog("config.json");
    expect(savePath).toBe("/tmp/saved.json");
    expect(invokeMock).toHaveBeenCalledWith("save_file_dialog", {
      defaultName: "config.json",
    });

    invokeMock.mockResolvedValueOnce("/tmp/opened.json");
    const openPath = await vscodeApi.openFileDialog();
    expect(openPath).toBe("/tmp/opened.json");
    expect(invokeMock).toHaveBeenCalledWith("open_file_dialog");
  });
});
