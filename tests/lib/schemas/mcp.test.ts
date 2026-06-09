import { describe, expect, it } from "vitest";
import { mcpServerSchema } from "@/lib/schemas/mcp";

describe("mcpServerSchema", () => {
  const baseStdio = {
    id: "server-1",
    server: {
      command: "node",
    },
  };

  it("requires a non-empty id", () => {
    expect(mcpServerSchema.safeParse({ ...baseStdio, id: "" }).success).toBe(
      false,
    );
    expect(
      mcpServerSchema.safeParse({ server: baseStdio.server } as any).success,
    ).toBe(false);
  });

  it("accepts optional fields when omitted or provided", () => {
    expect(mcpServerSchema.safeParse(baseStdio).success).toBe(true);

    const withOptionalFields = {
      ...baseStdio,
      name: "My MCP",
      description: "Test server",
      tags: ["alpha", "beta"],
      homepage: "https://example.com",
      docs: "https://example.com/docs",
      enabled: true,
    };

    expect(mcpServerSchema.safeParse(withOptionalFields).success).toBe(true);
  });

  it("defaults server.type to stdio when omitted", () => {
    expect(mcpServerSchema.safeParse(baseStdio).success).toBe(true);
    expect(
      mcpServerSchema.safeParse({ id: "server-1", server: {} }).success,
    ).toBe(false);
  });

  it("requires command for stdio servers", () => {
    expect(
      mcpServerSchema.safeParse({
        id: "server-1",
        server: { type: "stdio" },
      }).success,
    ).toBe(false);

    expect(
      mcpServerSchema.safeParse({
        id: "server-1",
        server: { type: "stdio", command: "   " },
      }).success,
    ).toBe(false);
  });

  it("requires url for http/sse servers", () => {
    expect(
      mcpServerSchema.safeParse({
        id: "server-1",
        server: { type: "http" },
      }).success,
    ).toBe(false);

    expect(
      mcpServerSchema.safeParse({
        id: "server-1",
        server: { type: "sse" },
      }).success,
    ).toBe(false);

    expect(
      mcpServerSchema.safeParse({
        id: "server-1",
        server: { type: "http", url: "https://example.com" },
      }).success,
    ).toBe(true);
  });

  it("validates server url format", () => {
    expect(
      mcpServerSchema.safeParse({
        id: "server-1",
        server: { type: "http", url: "not-a-url" },
      }).success,
    ).toBe(false);

    expect(
      mcpServerSchema.safeParse({
        id: "server-1",
        server: { type: "sse", url: "https://example.com/stream" },
      }).success,
    ).toBe(true);
  });

  it("validates homepage/docs url format", () => {
    expect(
      mcpServerSchema.safeParse({
        ...baseStdio,
        homepage: "not-a-url",
      }).success,
    ).toBe(false);

    expect(
      mcpServerSchema.safeParse({
        ...baseStdio,
        docs: "not-a-url",
      }).success,
    ).toBe(false);

    expect(
      mcpServerSchema.safeParse({
        ...baseStdio,
        homepage: "https://example.com",
        docs: "https://example.com/docs",
      }).success,
    ).toBe(true);
  });
});
