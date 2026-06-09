import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  WEB_API_BASE_STORAGE_KEY,
  WEB_AUTH_STORAGE_KEY,
} from "@/lib/api/adapter";

const importHealthCheckWeb = async () => {
  vi.resetModules();
  delete (window as any).__TAURI__;
  delete (window as any).__TAURI_INTERNALS__;
  return import("@/lib/api/healthCheck");
};

const createRelayPulsePayload = () => ({
  meta: { period: "24h", count: 1 },
  data: [
    {
      provider: "88code",
      provider_url: "https://88code.com",
      service: "cc",
      category: "third_party",
      current_status: { status: 1, latency: 120, timestamp: 1_710_000_000 },
      timeline: [{ availability: 90 }],
    },
  ],
});

let originalTauri: unknown;
let originalTauriInternals: unknown;

beforeEach(() => {
  originalTauri = (window as any).__TAURI__;
  originalTauriInternals = (window as any).__TAURI_INTERNALS__;
  delete (window as any).__TAURI__;
  delete (window as any).__TAURI_INTERNALS__;
  window.localStorage.clear();
  window.sessionStorage.clear();
});

afterEach(() => {
  (window as any).__TAURI__ = originalTauri;
  (window as any).__TAURI_INTERNALS__ = originalTauriInternals;
  vi.restoreAllMocks();
});

describe("healthCheck API module (web mode)", () => {
  it("fetchAllHealthStatus uses web endpoint and caches results", async () => {
    const payload = createRelayPulsePayload();
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValueOnce({
      ok: true,
      status: 200,
      headers: new Headers({ "content-type": "application/json" }),
      json: async () => payload,
    } as Response);

    window.sessionStorage.setItem(WEB_AUTH_STORAGE_KEY, "encoded");

    const { fetchAllHealthStatus } = await importHealthCheckWeb();

    const result = await fetchAllHealthStatus();

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/health/status",
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Basic encoded" }),
      }),
    );
    expect(result.get("88code/cc")?.status).toBe("available");

    const cached = await fetchAllHealthStatus();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(cached).toBe(result);
  });

  it("returns cached map when response is not ok", async () => {
    const payload = createRelayPulsePayload();
    vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        headers: new Headers({ "content-type": "application/json" }),
        json: async () => payload,
      } as Response)
      .mockResolvedValueOnce({
        ok: false,
        status: 500,
        headers: new Headers({ "content-type": "application/json" }),
        text: async () => "boom",
      } as Response);

    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    const { fetchAllHealthStatus, refreshHealthCache } =
      await importHealthCheckWeb();

    const first = await fetchAllHealthStatus();
    const refreshed = await refreshHealthCache();

    expect(refreshed).toBe(first);
    expect(warnSpy).not.toHaveBeenCalledWith(
      expect.stringContaining("timed out"),
    );
  });

  it("returns cached map when fetch aborts", async () => {
    const payload = createRelayPulsePayload();
    vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        headers: new Headers({ "content-type": "application/json" }),
        json: async () => payload,
      } as Response)
      .mockRejectedValueOnce(
        Object.assign(new Error("timeout"), { name: "AbortError" }),
      );

    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    const { fetchAllHealthStatus, refreshHealthCache } =
      await importHealthCheckWeb();

    const first = await fetchAllHealthStatus();
    const refreshed = await refreshHealthCache();

    expect(refreshed).toBe(first);
    expect(warnSpy).toHaveBeenCalled();
  });

  it("uses the configured web API base for the relay-pulse proxy", async () => {
    const payload = createRelayPulsePayload();
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValueOnce({
      ok: true,
      status: 200,
      headers: new Headers({ "content-type": "application/json" }),
      json: async () => payload,
    } as Response);

    window.localStorage.setItem(
      WEB_API_BASE_STORAGE_KEY,
      "https://10.0.0.2:8080/api",
    );
    window.sessionStorage.setItem(
      WEB_AUTH_STORAGE_KEY,
      JSON.stringify({
        token: "encoded",
        apiBase: "https://10.0.0.2:8080/api",
      }),
    );

    const { fetchAllHealthStatus } = await importHealthCheckWeb();

    await fetchAllHealthStatus();

    expect(fetchMock).toHaveBeenCalledWith(
      "https://10.0.0.2:8080/api/health/status",
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Basic encoded" }),
      }),
    );
  });
});
