import { describe, expect, it } from "vitest";
import { settingsSchema } from "@/lib/schemas/settings";

const baseSettings = {
  showInTray: true,
  minimizeToTrayOnClose: true,
};

describe("settingsSchema", () => {
  it("accepts WebDAV settings and preserves password spacing", () => {
    const result = settingsSchema.safeParse({
      ...baseSettings,
      webDav: {
        enabled: true,
        baseUrl: " https://dav.example.com/ ",
        username: " user ",
        password: " secret ",
        remoteDir: " cc-switch-web ",
        profile: " default ",
      },
    });

    expect(result.success).toBe(true);
    if (!result.success) {
      return;
    }
    expect(result.data.webDav).toEqual({
      enabled: true,
      baseUrl: "https://dav.example.com/",
      username: "user",
      password: " secret ",
      remoteDir: "cc-switch-web",
      profile: "default",
    });
  });

  it("defaults omitted WebDAV fields and rejects empty remote identifiers", () => {
    const defaulted = settingsSchema.safeParse({
      ...baseSettings,
      webDav: {},
    });
    expect(defaulted.success).toBe(true);
    if (defaulted.success) {
      expect(defaulted.data.webDav).toEqual({
        enabled: false,
        baseUrl: "",
        username: "",
        password: "",
        remoteDir: "cc-switch-web",
        profile: "default",
      });
    }

    expect(
      settingsSchema.safeParse({
        ...baseSettings,
        webDav: {
          remoteDir: " ",
        },
      }).success,
    ).toBe(false);
  });
});
