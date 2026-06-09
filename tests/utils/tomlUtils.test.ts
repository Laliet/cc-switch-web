import { describe, expect, it, vi } from "vitest";
import * as smolToml from "smol-toml";
import type { McpServerSpec } from "@/types";
import {
  extractIdFromToml,
  mcpServerToToml,
  tomlToMcpServer,
  validateToml,
} from "@/utils/tomlUtils";

const FORCE_ARRAY_TEXT = vi.hoisted(() => "__force_array__");

vi.mock("smol-toml", async () => {
  const actual = await vi.importActual<typeof import("smol-toml")>("smol-toml");
  return {
    ...actual,
    parse: (text: string) => {
      if (text === FORCE_ARRAY_TEXT) {
        return [];
      }
      return actual.parse(text);
    },
  };
});

describe("validateToml", () => {
  it("returns empty string for blank text", () => {
    expect(validateToml("")).toBe("");
    expect(validateToml("   \n  ")).toBe("");
  });

  it("returns empty string for valid TOML", () => {
    const result = validateToml('command = "node"\nargs = ["--help"]');
    expect(result).toBe("");
  });

  it("returns error message for invalid TOML", () => {
    const result = validateToml("command = ");
    expect(result).not.toBe("");
    expect(result).not.toBe("mustBeObject");
  });

  it("rejects non-object root values", () => {
    const result = validateToml(FORCE_ARRAY_TEXT);
    expect(result).toBe("mustBeObject");
  });
});

describe("mcpServerToToml", () => {
  it("converts stdio server with optional fields", () => {
    const server: McpServerSpec = {
      type: "stdio",
      command: "node",
      args: ["--foo", "bar"],
      env: {
        FOO: "bar",
      },
      cwd: "/work",
      timeout_ms: 1200,
    };

    const toml = mcpServerToToml(server);
    const parsed = smolToml.parse(toml);

    expect(parsed).toEqual({
      type: "stdio",
      command: "node",
      args: ["--foo", "bar"],
      env: {
        FOO: "bar",
      },
      cwd: "/work",
      timeout_ms: 1200,
    });
  });

  it("converts http server with headers", () => {
    const server: McpServerSpec = {
      type: "http",
      url: "https://example.com",
      headers: {
        Authorization: "Bearer token",
        Accept: "application/json",
      },
      timeout_ms: 5000,
    };

    const toml = mcpServerToToml(server);
    const parsed = smolToml.parse(toml);

    expect(parsed).toEqual({
      type: "http",
      url: "https://example.com",
      headers: {
        Authorization: "Bearer token",
        Accept: "application/json",
      },
      timeout_ms: 5000,
    });
  });

  it("removes undefined fields", () => {
    const server: McpServerSpec = {
      type: "stdio",
      command: "node",
      args: undefined,
      env: undefined,
      cwd: undefined,
      timeout_ms: undefined,
    };

    const toml = mcpServerToToml(server);
    const parsed = smolToml.parse(toml);

    expect(parsed).toEqual({
      type: "stdio",
      command: "node",
    });
    expect(toml).not.toContain("args");
    expect(toml).not.toContain("env");
    expect(toml).not.toContain("cwd");
    expect(toml).not.toContain("timeout_ms");
  });
});

describe("tomlToMcpServer", () => {
  it("parses direct server config", () => {
    const toml = [
      'command = "node"',
      'args = [1, "two"]',
      'env = { ONE = 1, TWO = "2" }',
      'cwd = "/work"',
      "timeout_ms = 2500",
    ].join("\n");

    expect(tomlToMcpServer(toml)).toEqual({
      type: "stdio",
      command: "node",
      args: ["1", "two"],
      env: {
        ONE: "1",
        TWO: "2",
      },
      cwd: "/work",
      timeout_ms: 2500,
    });
  });

  it("parses [mcp_servers.<id>] config", () => {
    const toml = [
      "[mcp_servers.alpha]",
      'type = "http"',
      'url = "https://example.com"',
      'headers = { Authorization = "Bearer token" }',
    ].join("\n");

    expect(tomlToMcpServer(toml)).toEqual({
      type: "http",
      url: "https://example.com",
      headers: {
        Authorization: "Bearer token",
      },
    });
  });

  it("parses [mcp.servers.<id>] config", () => {
    const toml = [
      "[mcp.servers.beta]",
      'type = "sse"',
      'url = "https://sse.example.com"',
    ].join("\n");

    expect(tomlToMcpServer(toml)).toEqual({
      type: "sse",
      url: "https://sse.example.com",
    });
  });

  it("throws on empty content", () => {
    expect(() => tomlToMcpServer("  \n\t")).toThrow();
  });
});

describe("extractIdFromToml", () => {
  it("extracts id from [mcp_servers.<id>] section", () => {
    const toml = ["[mcp_servers.sample-server]", 'command = "node"'].join("\n");

    expect(extractIdFromToml(toml)).toBe("sample-server");
  });

  it("infers id from command", () => {
    const toml = 'command = "/usr/local/bin/my-server.js"';

    expect(extractIdFromToml(toml)).toBe("my-server");
  });

  it("returns empty string on parse failure", () => {
    expect(extractIdFromToml("command = ")).toBe("");
  });
});
