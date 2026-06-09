import { afterEach, describe, expect, it, vi } from "vitest";
import { generateUUID } from "@/utils/uuid";

const UUID_V4_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("generateUUID", () => {
  it("returns a UUID v4 in 8-4-4-4-12 hex format", () => {
    expect(generateUUID()).toMatch(UUID_V4_REGEX);
  });

  it("generates unique UUIDs", () => {
    const uuids = new Set<string>();
    for (let i = 0; i < 1000; i += 1) {
      uuids.add(generateUUID());
    }
    expect(uuids.size).toBe(1000);
  });

  it("falls back when crypto.randomUUID is unavailable", () => {
    const getRandomValues = vi.fn((buffer: Uint8Array) => {
      for (let i = 0; i < buffer.length; i += 1) {
        buffer[i] = i;
      }
      return buffer;
    });

    vi.stubGlobal("crypto", { getRandomValues, randomUUID: undefined });

    const uuid = generateUUID();

    expect(getRandomValues).toHaveBeenCalledTimes(1);
    expect(uuid).toMatch(UUID_V4_REGEX);
  });

  it("falls back when crypto.randomUUID exists but is not a function", () => {
    const getRandomValues = vi.fn((buffer: Uint8Array) => {
      for (let i = 0; i < buffer.length; i += 1) {
        buffer[i] = (i * 7) % 256;
      }
      return buffer;
    });

    vi.stubGlobal("crypto", {
      getRandomValues,
      randomUUID: "not-a-function",
      randomUUlD: () => "legacy-typo-method",
    });

    const uuid = generateUUID();

    expect(getRandomValues).toHaveBeenCalledTimes(1);
    expect(uuid).toMatch(UUID_V4_REGEX);
  });

  it("falls back to Math.random when crypto.getRandomValues is unavailable", () => {
    vi.stubGlobal("crypto", {
      randomUUID: undefined,
      getRandomValues: undefined,
    });

    const mathRandomSpy = vi.spyOn(Math, "random").mockReturnValue(0.123456);

    const uuid = generateUUID();

    expect(mathRandomSpy).toHaveBeenCalled();
    expect(uuid).toMatch(UUID_V4_REGEX);
  });

  it("falls back to Math.random when crypto is missing", () => {
    vi.stubGlobal("crypto", undefined);
    const mathRandomSpy = vi.spyOn(Math, "random").mockReturnValue(0.654321);

    const uuid = generateUUID();

    expect(mathRandomSpy).toHaveBeenCalled();
    expect(uuid).toMatch(UUID_V4_REGEX);
  });
});
