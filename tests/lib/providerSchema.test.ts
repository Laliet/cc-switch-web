import { describe, expect, it } from "vitest";
import { providerSchema } from "@/lib/schemas/provider";

describe("providerSchema.websiteUrl", () => {
  const baseInput = {
    name: "Test Provider",
    settingsConfig: "{}",
  };

  it("accepts empty string or omitted value", () => {
    expect(
      providerSchema.safeParse({ ...baseInput, websiteUrl: "" }).success,
    ).toBe(true);

    expect(providerSchema.safeParse({ ...baseInput }).success).toBe(true);
  });

  it("accepts http/https and rejects other schemes", () => {
    expect(
      providerSchema.safeParse({
        ...baseInput,
        websiteUrl: "https://provider.example.com",
      }).success,
    ).toBe(true);

    expect(
      providerSchema.safeParse({
        ...baseInput,
        websiteUrl: "http://provider.example.com",
      }).success,
    ).toBe(true);

    const javascriptResult = providerSchema.safeParse({
      ...baseInput,
      websiteUrl: "javascript:alert(1)",
    });
    expect(javascriptResult.success).toBe(false);

    const dataResult = providerSchema.safeParse({
      ...baseInput,
      websiteUrl: "data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==",
    });
    expect(dataResult.success).toBe(false);

    const ftpResult = providerSchema.safeParse({
      ...baseInput,
      websiteUrl: "ftp://example.com",
    });
    expect(ftpResult.success).toBe(false);
  });
});
