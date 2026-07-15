import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { useSettingsMetadata } from "@/hooks/useSettingsMetadata";

const isPortableMock = vi.hoisted(() => vi.fn());
const capabilitiesMock = vi.hoisted(() => ({
  features: { portableMode: true },
}));

vi.mock("@/lib/api", () => ({
  settingsApi: {
    isPortable: (...args: unknown[]) => isPortableMock(...args),
  },
}));

vi.mock("@/lib/query", () => ({
  useCapabilitiesQuery: () => ({
    data: capabilitiesMock,
  }),
}));

describe("useSettingsMetadata", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    vi.clearAllMocks();
  });

  it("loads portable flag and handles success path", async () => {
    isPortableMock.mockResolvedValue(true);

    const { result } = renderHook(() => useSettingsMetadata());

    expect(result.current.isLoading).toBe(true);
    expect(result.current.isPortable).toBe(false);

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.isPortable).toBe(true);
    expect(result.current.isLoading).toBe(false);
  });

  it("handles errors from settingsApi and proceeds", async () => {
    isPortableMock.mockRejectedValue(new Error("network failure"));

    const { result } = renderHook(() => useSettingsMetadata());

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.isPortable).toBe(false);
    expect(result.current.isLoading).toBe(false);
  });

  it("allows updating restart flag via setters", async () => {
    isPortableMock.mockResolvedValue(false);

    const { result } = renderHook(() => useSettingsMetadata());

    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      result.current.setRequiresRestart(true);
      await Promise.resolve();
    });

    expect(result.current.requiresRestart).toBe(true);

    await act(async () => {
      result.current.acknowledgeRestart();
      await Promise.resolve();
    });

    expect(result.current.requiresRestart).toBe(false);
  });
});
