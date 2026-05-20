import { beforeEach, describe, expect, it, vi } from "vitest";
import { providersApi } from "@/lib/api/providers";

const adapterMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  isWeb: vi.fn(),
}));

const listenMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api/adapter", () => ({
  invoke: (...args: unknown[]) => adapterMocks.invoke(...args),
  isWeb: () => adapterMocks.isWeb(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

describe("providers API module", () => {
  beforeEach(() => {
    adapterMocks.invoke.mockReset();
    adapterMocks.isWeb.mockReset();
    listenMock.mockReset();
  });

  it("getAll returns providers map", async () => {
    const providers = {
      p1: { id: "p1", name: "Primary", settingsConfig: {} },
    };
    adapterMocks.invoke.mockResolvedValueOnce(providers);

    const result = await providersApi.getAll("claude");

    expect(result).toEqual(providers);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("get_providers", {
      app: "claude",
    });
  });

  it("getCurrent returns current provider id", async () => {
    adapterMocks.invoke.mockResolvedValueOnce("p2");

    const result = await providersApi.getCurrent("codex");

    expect(result).toBe("p2");
    expect(adapterMocks.invoke).toHaveBeenCalledWith("get_current_provider", {
      app: "codex",
    });
  });

  it("getBackup returns backup provider id", async () => {
    adapterMocks.invoke.mockResolvedValueOnce("p3");

    const result = await providersApi.getBackup("gemini");

    expect(result).toBe("p3");
    expect(adapterMocks.invoke).toHaveBeenCalledWith("get_backup_provider", {
      app: "gemini",
    });
  });

  it("setBackup sends backup id", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await providersApi.setBackup("p4", "claude");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("set_backup_provider", {
      id: "p4",
      app: "claude",
    });
  });

  it("add sends provider payload", async () => {
    const provider = { id: "p5", name: "Added", settingsConfig: {} };
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await providersApi.add(provider, "codex");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("add_provider", {
      provider,
      app: "codex",
    });
  });

  it("update sends provider payload", async () => {
    const provider = { id: "p6", name: "Updated", settingsConfig: {} };
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await providersApi.update(provider, "gemini");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("update_provider", {
      provider,
      app: "gemini",
    });
  });

  it("delete sends provider id", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await providersApi.delete("p7", "claude");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("delete_provider", {
      id: "p7",
      app: "claude",
    });
  });

  it("importDefault sends app id", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await providersApi.importDefault("codex");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("import_default_config", {
      app: "codex",
    });
  });

  it("updateTrayMenu invokes without args", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await providersApi.updateTrayMenu();

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("update_tray_menu");
  });

  it("updateSortOrder sends updates", async () => {
    const updates = [{ id: "p8", sortIndex: 2 }];
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await providersApi.updateSortOrder(updates, "gemini");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith(
      "update_providers_sort_order",
      {
        updates,
        app: "gemini",
      },
    );
  });

  it("getOmoPluginStatus invokes status command", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await providersApi.getOmoPluginStatus();

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("get_omo_plugin_status");
  });

  it("switch triggers web sync when in web mode", async () => {
    adapterMocks.isWeb.mockReturnValue(true);
    adapterMocks.invoke.mockResolvedValueOnce(true);
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await providersApi.switch("p9", "claude");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenNthCalledWith(1, "switch_provider", {
      id: "p9",
      app: "claude",
    });
    expect(adapterMocks.invoke).toHaveBeenNthCalledWith(
      2,
      "sync_current_providers_live",
    );
  });

  it("switch throws when web sync fails", async () => {
    adapterMocks.isWeb.mockReturnValue(true);
    adapterMocks.invoke.mockResolvedValueOnce(true);
    adapterMocks.invoke.mockRejectedValueOnce(new Error("sync failed"));

    await expect(providersApi.switch("p10", "codex")).rejects.toThrow(
      "切换成功，但同步配置失败：sync failed",
    );
  });

  it("switch skips web sync when not in web mode", async () => {
    adapterMocks.isWeb.mockReturnValue(false);
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await providersApi.switch("p11", "gemini");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledTimes(1);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("switch_provider", {
      id: "p11",
      app: "gemini",
    });
  });

  it("onSwitched returns noop in web mode", async () => {
    adapterMocks.isWeb.mockReturnValue(true);
    const handler = vi.fn();

    const unlisten = await providersApi.onSwitched(handler);

    expect(listenMock).not.toHaveBeenCalled();
    expect(unlisten).toEqual(expect.any(Function));
  });

  it("onSwitched wires tauri event listener in non-web mode", async () => {
    adapterMocks.isWeb.mockReturnValue(false);
    const handler = vi.fn();
    const unlisten = vi.fn();
    listenMock.mockResolvedValueOnce(unlisten);

    const result = await providersApi.onSwitched(handler);

    expect(listenMock).toHaveBeenCalledWith(
      "provider-switched",
      expect.any(Function),
    );
    expect(result).toBe(unlisten);
  });
});
