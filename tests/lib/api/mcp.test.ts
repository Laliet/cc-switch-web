import { beforeEach, describe, expect, it, vi } from "vitest";
import { mcpApi } from "@/lib/api/mcp";
import type {
  McpConfigResponse,
  McpServer,
  McpServerSpec,
  McpServersMap,
  McpStatus,
} from "@/types";

const adapterMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock("@/lib/api/adapter", () => adapterMocks);

describe("mcpApi", () => {
  const sampleStatus: McpStatus = {
    userConfigPath: "/config/path",
    userConfigExists: true,
    serverCount: 2,
  };
  const sampleSpec: McpServerSpec = {
    type: "stdio",
    command: "node",
    args: ["-v"],
    env: { NODE_ENV: "test" },
  };
  const sampleServer: McpServer = {
    id: "server-1",
    name: "Test Server",
    server: sampleSpec,
    apps: { claude: true, codex: false, gemini: true, opencode: false },
  };
  const sampleServers: McpServersMap = {
    [sampleServer.id]: sampleServer,
  };

  beforeEach(() => {
    adapterMocks.invoke.mockReset();
  });

  it("getStatus calls invoke with get_claude_mcp_status", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(sampleStatus);

    const result = await mcpApi.getStatus();

    expect(result).toBe(sampleStatus);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("get_claude_mcp_status");
  });

  it("imports MCP definitions from server-host applications", async () => {
    const response = {
      imported: 2,
      merged: 1,
      conflicts: 1,
      sources: [],
    };
    adapterMocks.invoke.mockResolvedValueOnce(response);

    await expect(mcpApi.importFromApps()).resolves.toBe(response);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("import_mcp_from_apps");
  });

  it("readConfig calls invoke with read_claude_mcp_config", async () => {
    adapterMocks.invoke.mockResolvedValueOnce("config-content");

    const result = await mcpApi.readConfig();

    expect(result).toBe("config-content");
    expect(adapterMocks.invoke).toHaveBeenCalledWith("read_claude_mcp_config");
  });

  it("upsertServer sends id and spec", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await mcpApi.upsertServer("server-1", sampleSpec);

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith(
      "upsert_claude_mcp_server",
      {
        id: "server-1",
        spec: sampleSpec,
      },
    );
  });

  it("deleteServer sends id", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(false);

    const result = await mcpApi.deleteServer("server-2");

    expect(result).toBe(false);
    expect(adapterMocks.invoke).toHaveBeenCalledWith(
      "delete_claude_mcp_server",
      { id: "server-2" },
    );
  });

  it("validateCommand sends cmd", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await mcpApi.validateCommand("echo hi");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("validate_mcp_command", {
      cmd: "echo hi",
    });
  });

  it("getConfig defaults to claude app", async () => {
    const response: McpConfigResponse = {
      configPath: "/config.json",
      servers: sampleServers,
    };
    adapterMocks.invoke.mockResolvedValueOnce(response);

    const result = await mcpApi.getConfig();

    expect(result).toBe(response);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("get_mcp_config", {
      app: "claude",
    });
  });

  it("upsertServerInConfig omits syncOtherSide when undefined", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await mcpApi.upsertServerInConfig(
      "claude",
      "server-1",
      sampleServer,
    );

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith(
      "upsert_mcp_server_in_config",
      {
        app: "claude",
        id: "server-1",
        spec: sampleServer,
      },
    );
    const [, payload] = adapterMocks.invoke.mock.calls[0];
    expect(payload).not.toHaveProperty("syncOtherSide");
  });

  it("upsertServerInConfig includes syncOtherSide when provided", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(false);

    const result = await mcpApi.upsertServerInConfig(
      "codex",
      "server-3",
      sampleServer,
      { syncOtherSide: false },
    );

    expect(result).toBe(false);
    expect(adapterMocks.invoke).toHaveBeenCalledWith(
      "upsert_mcp_server_in_config",
      {
        app: "codex",
        id: "server-3",
        spec: sampleServer,
        syncOtherSide: false,
      },
    );
  });

  it("deleteServerInConfig omits syncOtherSide when undefined", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await mcpApi.deleteServerInConfig("claude", "server-4");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith(
      "delete_mcp_server_in_config",
      {
        app: "claude",
        id: "server-4",
      },
    );
    const [, payload] = adapterMocks.invoke.mock.calls[0];
    expect(payload).not.toHaveProperty("syncOtherSide");
  });

  it("deleteServerInConfig includes syncOtherSide when provided", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(false);

    const result = await mcpApi.deleteServerInConfig("codex", "server-5", {
      syncOtherSide: true,
    });

    expect(result).toBe(false);
    expect(adapterMocks.invoke).toHaveBeenCalledWith(
      "delete_mcp_server_in_config",
      {
        app: "codex",
        id: "server-5",
        syncOtherSide: true,
      },
    );
  });

  it("setEnabled sends app, id, and enabled", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await mcpApi.setEnabled("gemini", "server-6", false);

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("set_mcp_enabled", {
      app: "gemini",
      id: "server-6",
      enabled: false,
    });
  });

  it("getAllServers returns unified server map", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(sampleServers);

    const result = await mcpApi.getAllServers();

    expect(result).toBe(sampleServers);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("get_mcp_servers");
  });

  it("upsertUnifiedServer sends server payload", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(undefined);

    const result = await mcpApi.upsertUnifiedServer(sampleServer);

    expect(result).toBeUndefined();
    expect(adapterMocks.invoke).toHaveBeenCalledWith("upsert_mcp_server", {
      server: sampleServer,
    });
  });

  it("deleteUnifiedServer sends id", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(true);

    const result = await mcpApi.deleteUnifiedServer("server-7");

    expect(result).toBe(true);
    expect(adapterMocks.invoke).toHaveBeenCalledWith("delete_mcp_server", {
      id: "server-7",
    });
  });

  it("toggleApp sends serverId, app, and enabled", async () => {
    adapterMocks.invoke.mockResolvedValueOnce(undefined);

    const result = await mcpApi.toggleApp("server-8", "claude", true);

    expect(result).toBeUndefined();
    expect(adapterMocks.invoke).toHaveBeenCalledWith("toggle_mcp_app", {
      serverId: "server-8",
      app: "claude",
      enabled: true,
    });
  });
});
