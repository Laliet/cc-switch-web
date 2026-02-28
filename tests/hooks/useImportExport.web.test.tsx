import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { useImportExport } from "@/hooks/useImportExport";
import { WEB_AUTH_STORAGE_KEY } from "@/lib/api/adapter";

const toastSuccessMock = vi.fn();
const toastErrorMock = vi.fn();
const toastWarningMock = vi.fn();

vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => toastSuccessMock(...args),
    error: (...args: unknown[]) => toastErrorMock(...args),
    warning: (...args: unknown[]) => toastWarningMock(...args),
  },
}));

const importConfigMock = vi.fn();

vi.mock("@/lib/api", () => ({
  settingsApi: {
    importConfigFromFile: (...args: unknown[]) => importConfigMock(...args),
  },
}));

const syncCurrentProvidersLiveSafeMock = vi.fn();

vi.mock("@/utils/postChangeSync", () => ({
  syncCurrentProvidersLiveSafe: () => syncCurrentProvidersLiveSafeMock(),
}));

vi.mock("@/lib/api/adapter", () => ({
  isWeb: () => true,
  buildWebApiUrl: (path: string) => `/custom-api${path}`,
  buildWebAuthHeadersForUrl: (_url: string) => {
    const stored = window.sessionStorage.getItem("cc-switch-web-auth");
    if (!stored) return {};
    return { Authorization: `Basic ${stored}` };
  },
  WEB_AUTH_STORAGE_KEY: "cc-switch-web-auth",
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      (options?.defaultValue as string) ?? key,
  }),
}));

const createFileList = (file: File): FileList =>
  Object.assign(
    {
      item: (index: number) => (index === 0 ? file : null),
      length: 1,
    },
    {
      0: file,
      [Symbol.iterator]: function* () {
        yield file;
      },
    },
  ) as unknown as FileList;

describe("useImportExport (web mode)", () => {
  const originalCreateElement = document.createElement.bind(document);

  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    toastSuccessMock.mockReset();
    toastErrorMock.mockReset();
    toastWarningMock.mockReset();
    importConfigMock.mockReset();
    syncCurrentProvidersLiveSafeMock.mockReset();
    window.sessionStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("selectImportFile reads file content via input", async () => {
    const input = originalCreateElement("input") as HTMLInputElement;
    const file = {
      name: "config.json",
      text: vi.fn().mockResolvedValue("{\"ok\":true}"),
    } as unknown as File;
    const fileList = createFileList(file);

    Object.defineProperty(input, "files", { value: fileList, configurable: true });
    Object.defineProperty(input, "click", {
      value: () => input.onchange?.(new Event("change")),
    });

    vi.spyOn(document, "createElement").mockImplementation((tag) => {
      if (tag === "input") return input;
      return originalCreateElement(tag);
    });

    const { result } = renderHook(() => useImportExport());

    await act(async () => {
      await result.current.selectImportFile();
    });

    await waitFor(() => expect(result.current.selectedFile).toBe("config.json"));
    expect(result.current.status).toBe("idle");
    expect(result.current.errorMessage).toBeNull();
  });

  it("selectImportFile reports read errors", async () => {
    const input = originalCreateElement("input") as HTMLInputElement;
    const file = {
      name: "config.json",
      text: vi.fn().mockRejectedValue(new Error("read failed")),
    } as unknown as File;
    const fileList = createFileList(file);

    Object.defineProperty(input, "files", { value: fileList, configurable: true });
    Object.defineProperty(input, "click", {
      value: () => input.onchange?.(new Event("change")),
    });

    vi.spyOn(document, "createElement").mockImplementation((tag) => {
      if (tag === "input") return input;
      return originalCreateElement(tag);
    });

    const { result } = renderHook(() => useImportExport());

    await act(async () => {
      await result.current.selectImportFile();
    });

    expect(toastErrorMock).toHaveBeenCalled();
    expect(result.current.selectedFile).toBe("");
  });

  it("importConfig uses selected file content and reports partial success", async () => {
    const input = originalCreateElement("input") as HTMLInputElement;
    const file = {
      name: "config.json",
      text: vi.fn().mockResolvedValue("{\"ok\":true}"),
    } as unknown as File;
    const fileList = createFileList(file);

    Object.defineProperty(input, "files", { value: fileList, configurable: true });
    Object.defineProperty(input, "click", {
      value: () => input.onchange?.(new Event("change")),
    });

    vi.spyOn(document, "createElement").mockImplementation((tag) => {
      if (tag === "input") return input;
      return originalCreateElement(tag);
    });

    importConfigMock.mockResolvedValueOnce({ success: true, backupId: "b" });
    syncCurrentProvidersLiveSafeMock.mockResolvedValueOnce({
      ok: false,
      error: new Error("sync failed"),
    });

    const { result } = renderHook(() => useImportExport());

    await act(async () => {
      await result.current.selectImportFile();
    });

    await act(async () => {
      await result.current.importConfig();
    });

    expect(importConfigMock).toHaveBeenCalledWith(
      "config.json",
      expect.stringContaining("ok"),
    );
    expect(result.current.status).toBe("partial-success");
    expect(toastWarningMock).toHaveBeenCalled();
  });

  it("exportConfig uses fetch and auth header", async () => {
    vi.useFakeTimers();
    const anchor = originalCreateElement("a") as HTMLAnchorElement;
    Object.defineProperty(anchor, "click", { value: vi.fn() });

    vi.spyOn(document, "createElement").mockImplementation((tag) => {
      if (tag === "a") return anchor;
      return originalCreateElement(tag);
    });

    if (!URL.createObjectURL) {
      Object.defineProperty(URL, "createObjectURL", {
        value: () => "blob:mock",
        configurable: true,
      });
    }
    if (!URL.revokeObjectURL) {
      Object.defineProperty(URL, "revokeObjectURL", {
        value: () => undefined,
        configurable: true,
      });
    }

    const createObjectUrlMock = vi.spyOn(URL, "createObjectURL");
    createObjectUrlMock.mockReturnValue("blob:mock");
    const revokeObjectUrlMock = vi.spyOn(URL, "revokeObjectURL");

    window.sessionStorage.setItem(WEB_AUTH_STORAGE_KEY, "encoded");

    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => ({ config: true }),
      } as Response);

    try {
      const { result } = renderHook(() => useImportExport());

      await act(async () => {
        await result.current.exportConfig();
      });
      await vi.runAllTimersAsync();

      expect(fetchMock).toHaveBeenCalledWith(
        "/custom-api/config/export",
        expect.objectContaining({
          credentials: "include",
          headers: expect.objectContaining({ Authorization: "Basic encoded" }),
        }),
      );
      expect(anchor.click).toHaveBeenCalled();
      expect(createObjectUrlMock).toHaveBeenCalled();
      expect(revokeObjectUrlMock).toHaveBeenCalledWith("blob:mock");
      expect(toastSuccessMock).toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("exportConfig reports fetch errors", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce({
      ok: false,
      status: 500,
      text: async () => "boom",
    } as Response);

    const { result } = renderHook(() => useImportExport());

    await act(async () => {
      await result.current.exportConfig();
    });

    expect(toastErrorMock).toHaveBeenCalled();
  });
});
