import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { usePromptActions } from "@/hooks/usePromptActions";
import type { Prompt } from "@/lib/api";

const getPromptsMock = vi.hoisted(() => vi.fn());
const getCurrentFileContentMock = vi.hoisted(() => vi.fn());
const upsertPromptMock = vi.hoisted(() => vi.fn());
const deletePromptMock = vi.hoisted(() => vi.fn());
const enablePromptMock = vi.hoisted(() => vi.fn());
const importFromFileMock = vi.hoisted(() => vi.fn());

const toastSuccessMock = vi.hoisted(() => vi.fn());
const toastErrorMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", () => ({
  promptsApi: {
    getPrompts: (...args: unknown[]) => getPromptsMock(...args),
    getCurrentFileContent: (...args: unknown[]) =>
      getCurrentFileContentMock(...args),
    upsertPrompt: (...args: unknown[]) => upsertPromptMock(...args),
    deletePrompt: (...args: unknown[]) => deletePromptMock(...args),
    enablePrompt: (...args: unknown[]) => enablePromptMock(...args),
    importFromFile: (...args: unknown[]) => importFromFileMock(...args),
  },
}));

vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => toastSuccessMock(...args),
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

const createPrompt = (overrides: Partial<Prompt> = {}): Prompt => ({
  id: "prompt-1",
  name: "Prompt 1",
  content: "content",
  enabled: true,
  ...overrides,
});

describe("usePromptActions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getCurrentFileContentMock.mockResolvedValue(null);
  });

  it("reload loads prompts and current file content", async () => {
    const prompts = {
      "prompt-1": createPrompt({ enabled: true }),
    };
    getPromptsMock.mockResolvedValueOnce(prompts);
    getCurrentFileContentMock.mockResolvedValueOnce("file content");

    const { result } = renderHook(() => usePromptActions("claude"));

    await act(async () => {
      await result.current.reload();
    });

    expect(getPromptsMock).toHaveBeenCalledWith("claude");
    expect(getCurrentFileContentMock).toHaveBeenCalledWith("claude");
    expect(result.current.prompts).toEqual(prompts);
    expect(result.current.currentFileContent).toBe("file content");
  });

  it("savePrompt persists changes and shows success toast", async () => {
    const prompt = createPrompt({ id: "prompt-1", content: "updated" });
    const prompts = { "prompt-1": prompt };

    upsertPromptMock.mockResolvedValueOnce(undefined);
    getPromptsMock.mockResolvedValueOnce(prompts);

    const { result } = renderHook(() => usePromptActions("claude"));

    await act(async () => {
      await result.current.savePrompt("prompt-1", prompt);
    });

    expect(upsertPromptMock).toHaveBeenCalledWith("claude", "prompt-1", prompt);
    expect(getPromptsMock).toHaveBeenCalledTimes(1);
    expect(result.current.prompts).toEqual(prompts);
    expect(toastSuccessMock).toHaveBeenCalledWith("prompts.saveSuccess");
  });

  it("deletePrompt deletes prompt and shows success toast", async () => {
    const prompts = {};

    deletePromptMock.mockResolvedValueOnce(undefined);
    getPromptsMock.mockResolvedValueOnce(prompts);

    const { result } = renderHook(() => usePromptActions("claude"));

    await act(async () => {
      await result.current.deletePrompt("prompt-1");
    });

    expect(deletePromptMock).toHaveBeenCalledWith("claude", "prompt-1");
    expect(getPromptsMock).toHaveBeenCalledTimes(1);
    expect(result.current.prompts).toEqual(prompts);
    expect(toastSuccessMock).toHaveBeenCalledWith("prompts.deleteSuccess");
  });

  it("enablePrompt enables prompt and shows success toast", async () => {
    const prompts = { "prompt-1": createPrompt({ enabled: true }) };

    enablePromptMock.mockResolvedValueOnce(undefined);
    getPromptsMock.mockResolvedValueOnce(prompts);

    const { result } = renderHook(() => usePromptActions("claude"));

    await act(async () => {
      await result.current.enablePrompt("prompt-1");
    });

    expect(enablePromptMock).toHaveBeenCalledWith("claude", "prompt-1");
    expect(result.current.prompts).toEqual(prompts);
    expect(toastSuccessMock).toHaveBeenCalledWith("prompts.enableSuccess");
  });

  it("toggleEnabled enables target prompt and disables others", async () => {
    const initialPrompts = {
      "prompt-1": createPrompt({ id: "prompt-1", enabled: true }),
      "prompt-2": createPrompt({
        id: "prompt-2",
        name: "Prompt 2",
        enabled: false,
      }),
    };
    const updatedPrompts = {
      "prompt-1": createPrompt({ id: "prompt-1", enabled: false }),
      "prompt-2": createPrompt({
        id: "prompt-2",
        name: "Prompt 2",
        enabled: true,
      }),
    };

    getPromptsMock
      .mockResolvedValueOnce(initialPrompts)
      .mockResolvedValueOnce(updatedPrompts);
    enablePromptMock.mockResolvedValueOnce(undefined);

    const { result } = renderHook(() => usePromptActions("claude"));

    await act(async () => {
      await result.current.reload();
    });

    await act(async () => {
      await result.current.toggleEnabled("prompt-2", true);
    });

    expect(enablePromptMock).toHaveBeenCalledWith("claude", "prompt-2");
    expect(result.current.prompts).toEqual(updatedPrompts);
    expect(result.current.prompts["prompt-1"].enabled).toBe(false);
    expect(result.current.prompts["prompt-2"].enabled).toBe(true);
    expect(toastSuccessMock).toHaveBeenCalledWith("prompts.enableSuccess");
  });

  it("toggleEnabled disables target prompt and persists it", async () => {
    const initialPrompts = {
      "prompt-1": createPrompt({ id: "prompt-1", enabled: true }),
    };
    const updatedPrompts = {
      "prompt-1": createPrompt({ id: "prompt-1", enabled: false }),
    };

    getPromptsMock
      .mockResolvedValueOnce(initialPrompts)
      .mockResolvedValueOnce(updatedPrompts);
    upsertPromptMock.mockResolvedValueOnce(undefined);

    const { result } = renderHook(() => usePromptActions("claude"));

    await act(async () => {
      await result.current.reload();
    });

    await act(async () => {
      await result.current.toggleEnabled("prompt-1", false);
    });

    expect(upsertPromptMock).toHaveBeenCalledWith("claude", "prompt-1", {
      ...initialPrompts["prompt-1"],
      enabled: false,
    });
    expect(result.current.prompts).toEqual(updatedPrompts);
    expect(toastSuccessMock).toHaveBeenCalledWith("prompts.disableSuccess");
  });

  it("toggleEnabled rolls back on failure", async () => {
    const initialPrompts = {
      "prompt-1": createPrompt({ id: "prompt-1", enabled: true }),
      "prompt-2": createPrompt({
        id: "prompt-2",
        name: "Prompt 2",
        enabled: false,
      }),
    };

    getPromptsMock.mockResolvedValueOnce(initialPrompts);
    enablePromptMock.mockRejectedValueOnce(new Error("enable failed"));

    const { result } = renderHook(() => usePromptActions("claude"));

    await act(async () => {
      await result.current.reload();
    });

    await expect(
      act(async () => {
        await result.current.toggleEnabled("prompt-2", true);
      }),
    ).rejects.toThrow("enable failed");

    await waitFor(() => expect(result.current.prompts).toEqual(initialPrompts));
    expect(toastErrorMock).toHaveBeenCalledWith("prompts.enableFailed");
  });

  it("importFromFile imports and returns id", async () => {
    const prompts = { "prompt-1": createPrompt({ enabled: true }) };

    importFromFileMock.mockResolvedValueOnce("imported-id");
    getPromptsMock.mockResolvedValueOnce(prompts);

    const { result } = renderHook(() => usePromptActions("claude"));

    let returnedId: string | undefined;
    await act(async () => {
      returnedId = await result.current.importFromFile();
    });

    expect(importFromFileMock).toHaveBeenCalledWith("claude");
    expect(returnedId).toBe("imported-id");
    expect(result.current.prompts).toEqual(prompts);
    expect(toastSuccessMock).toHaveBeenCalledWith("prompts.importSuccess");
  });

  it("shows error toast when reload fails", async () => {
    getPromptsMock.mockRejectedValueOnce(new Error("load failed"));

    const { result } = renderHook(() => usePromptActions("claude"));

    await act(async () => {
      await result.current.reload();
    });

    expect(toastErrorMock).toHaveBeenCalledWith("prompts.loadFailed");
  });
});
