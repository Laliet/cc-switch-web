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

  it("trims network mirror settings", () => {
    const result = settingsSchema.safeParse({
      ...baseSettings,
      network: {
        githubMirrorBaseUrl: " https://ghproxy.net/ ",
      },
    });

    expect(result.success).toBe(true);
    if (!result.success) {
      return;
    }
    expect(result.data.network).toEqual({
      githubMirrorBaseUrl: "https://ghproxy.net/",
    });
  });

  it("rejects non-http GitHub mirror settings", () => {
    expect(
      settingsSchema.safeParse({
        ...baseSettings,
        network: {
          githubMirrorBaseUrl: "ghproxy.net",
        },
      }).success,
    ).toBe(false);

    expect(
      settingsSchema.safeParse({
        ...baseSettings,
        network: {
          githubMirrorBaseUrl: "file:///tmp/mirror",
        },
      }).success,
    ).toBe(false);
  });

  it("accepts Claude Desktop as a proxy bind app", () => {
    const result = settingsSchema.safeParse({
      ...baseSettings,
      proxy: {
        enabled: false,
        host: "127.0.0.1",
        port: 3456,
        bindApp: "claude-desktop",
        autoStart: false,
        enableLogging: false,
        liveTakeoverActive: false,
        apps: {},
      },
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.proxy?.bindApp).toBe("claude-desktop");
    }
  });
});
