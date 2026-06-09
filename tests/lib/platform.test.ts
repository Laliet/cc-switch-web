import { afterEach, describe, expect, it, vi } from "vitest";
import { isLinux, isMac, isWindows } from "@/lib/platform";

const setNavigator = (userAgent: string, platform = "") => {
  vi.stubGlobal("navigator", { userAgent, platform });
};

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("platform", () => {
  describe("isMac", () => {
    it("returns true for Mac OS X user agent", () => {
      setNavigator(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
      );

      expect(isMac()).toBe(true);
    });

    it("returns true for Mac platform", () => {
      setNavigator(
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        "MacIntel",
      );

      expect(isMac()).toBe(true);
    });

    it("returns false for Windows user agent", () => {
      setNavigator(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        "Win32",
      );

      expect(isMac()).toBe(false);
    });

    it("returns false when navigator is unavailable", () => {
      vi.stubGlobal("navigator", undefined);

      expect(isMac()).toBe(false);
    });
  });

  describe("isWindows", () => {
    it("returns true for Windows user agent", () => {
      setNavigator(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
      );

      expect(isWindows()).toBe(true);
    });

    it("returns false for Mac user agent", () => {
      setNavigator(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
      );

      expect(isWindows()).toBe(false);
    });

    it("returns false when navigator is unavailable", () => {
      vi.stubGlobal("navigator", undefined);

      expect(isWindows()).toBe(false);
    });
  });

  describe("isLinux", () => {
    it("returns true for Linux user agent", () => {
      setNavigator(
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        "Linux x86_64",
      );

      expect(isLinux()).toBe(true);
    });

    it("returns false for Android user agent", () => {
      setNavigator(
        "Mozilla/5.0 (Linux; Android 10; Pixel 3) AppleWebKit/537.36",
        "Linux armv8l",
      );

      expect(isLinux()).toBe(false);
    });

    it("returns false for Mac user agent", () => {
      setNavigator(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
      );

      expect(isLinux()).toBe(false);
    });

    it("returns false for Windows user agent", () => {
      setNavigator(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
      );

      expect(isLinux()).toBe(false);
    });

    it("returns false when navigator is unavailable", () => {
      vi.stubGlobal("navigator", undefined);

      expect(isLinux()).toBe(false);
    });
  });
});
