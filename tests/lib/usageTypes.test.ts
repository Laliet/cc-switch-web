import { describe, expect, it } from "vitest";
import { KNOWN_USAGE_APP_TYPES, usageAppLabel } from "@/types/usage";

describe("usage app metadata", () => {
  it("keeps Claude Desktop as a first-class usage filter boundary", () => {
    expect(KNOWN_USAGE_APP_TYPES).toContain("claude-desktop");
    expect(usageAppLabel("claude-desktop")).toBe("Claude Desktop");
    expect(usageAppLabel("all")).toBe("All apps");
  });
});
