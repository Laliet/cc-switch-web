import type { ReactNode } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { OpenClawConfigCenter } from "@/components/openclaw/OpenClawConfigCenter";
import {
  getOpenClawAgentsState,
  getOpenClawEnvState,
  getOpenClawRawState,
  getOpenClawToolsState,
  setOpenClawAgentsState,
  setOpenClawEnvState,
  setOpenClawToolsState,
} from "../../msw/state";

const toastSuccessMock = vi.hoisted(() => vi.fn());
const toastErrorMock = vi.hoisted(() => vi.fn());

vi.mock("react-i18next", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react-i18next")>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) => key,
    }),
  };
});

vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => toastSuccessMock(...args),
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

vi.mock("@/components/JsonEditor", () => ({
  default: ({
    value,
    onChange,
    language,
  }: {
    value: string;
    onChange: (value: string) => void;
    language: "json" | "javascript";
  }) => (
    <textarea
      aria-label={
        language === "javascript" ? "openclaw-raw-config" : "openclaw-env-json"
      }
      value={value}
      onChange={(event) => onChange(event.target.value)}
    />
  ),
}));

function renderCenter() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return render(<OpenClawConfigCenter open onOpenChange={vi.fn()} />, {
    wrapper,
  });
}

describe("OpenClawConfigCenter", () => {
  beforeEach(() => {
    toastSuccessMock.mockClear();
    toastErrorMock.mockClear();
  });

  it("edits agents defaults while preserving unknown fields", async () => {
    setOpenClawAgentsState({
      workspace: "/old/workspace",
      futureSetting: { enabled: true },
    });
    const user = userEvent.setup();
    renderCenter();

    const workspace = await screen.findByLabelText("openclaw.config.workspace");
    await user.clear(workspace);
    await user.type(workspace, "/new/workspace");
    await user.click(screen.getByRole("button", { name: "common.save" }));

    await waitFor(() =>
      expect(getOpenClawAgentsState().value).toMatchObject({
        workspace: "/new/workspace",
        futureSetting: { enabled: true },
      }),
    );
    expect(toastSuccessMock).toHaveBeenCalledWith("openclaw.config.saved");
    expect(toastErrorMock).not.toHaveBeenCalled();
  });

  it("migrates the legacy agents timeout and keeps catalog-only models editable", async () => {
    setOpenClawAgentsState({
      timeout: 30,
      models: {
        "retired/model": { alias: "Legacy" },
      },
    });
    const user = userEvent.setup();
    renderCenter();

    const timeout = await screen.findByLabelText("openclaw.config.timeout");
    expect(timeout).toHaveValue(30);
    expect(
      screen.getByText("openclaw.config.legacyTimeoutTitle"),
    ).toBeInTheDocument();
    const retiredModel = screen.getByRole("checkbox", {
      name: "retired/model",
    });
    expect(retiredModel).toBeChecked();
    await user.click(retiredModel);

    await user.click(screen.getByRole("button", { name: "common.save" }));

    await waitFor(() => {
      expect(getOpenClawAgentsState().value).toMatchObject({
        timeoutSeconds: 30,
      });
      expect(getOpenClawAgentsState().value).not.toHaveProperty("timeout");
      expect(getOpenClawAgentsState().value).not.toHaveProperty("models");
    });
  });

  it("edits environment JSON and tools without discarding unknown fields", async () => {
    setOpenClawEnvState({ EXISTING_TOKEN: "old" });
    setOpenClawToolsState({
      profile: "coding",
      allow: ["shell"],
      extensionSetting: "preserve-me",
    });
    const user = userEvent.setup();
    renderCenter();

    await user.click(
      await screen.findByRole("tab", { name: "openclaw.config.tabs.env" }),
    );
    const editor = await screen.findByLabelText("openclaw-env-json");
    fireEvent.change(editor, {
      target: {
        value: '{"EXISTING_TOKEN":"new","NEW_TOKEN":"value"}',
      },
    });
    await user.click(screen.getByRole("button", { name: "common.save" }));
    await waitFor(() =>
      expect(getOpenClawEnvState().value).toEqual({
        EXISTING_TOKEN: "new",
        NEW_TOKEN: "value",
      }),
    );

    await user.click(
      screen.getByRole("tab", { name: "openclaw.config.tabs.tools" }),
    );
    const allowInput = await screen.findByDisplayValue("shell");
    await user.clear(allowInput);
    await user.type(allowInput, "filesystem");
    await user.click(screen.getByRole("button", { name: "common.save" }));
    await waitFor(() =>
      expect(getOpenClawToolsState().value).toMatchObject({
        profile: "coding",
        allow: ["filesystem"],
        extensionSetting: "preserve-me",
      }),
    );
  });

  it("applies a selected provider reconciliation using the preview ETag", async () => {
    const user = userEvent.setup();
    renderCenter();

    await user.click(
      await screen.findByRole("tab", {
        name: "openclaw.config.tabs.reconcile",
      }),
    );
    await user.click(
      await screen.findByRole("checkbox", { name: "openclaw-1" }),
    );
    await user.click(
      screen.getByRole("button", { name: "openclaw.reconcile.apply" }),
    );

    await waitFor(() =>
      expect(toastSuccessMock).toHaveBeenCalledWith(
        "openclaw.reconcile.completed",
        expect.any(Object),
      ),
    );
  });

  it("edits the advanced JSON5 source as one ETag-protected document", async () => {
    const user = userEvent.setup();
    renderCenter();

    await user.click(
      await screen.findByRole("tab", { name: "openclaw.config.tabs.raw" }),
    );
    const editor = await screen.findByLabelText("openclaw-raw-config");
    const source = "{\n  // custom\n  models: { providers: {} },\n}\n";
    fireEvent.change(editor, { target: { value: source } });
    await user.click(screen.getByRole("button", { name: "common.save" }));

    await waitFor(() => expect(getOpenClawRawState().value).toBe(source));
    expect(toastSuccessMock).toHaveBeenCalledWith("openclaw.config.saved");
  });
});
