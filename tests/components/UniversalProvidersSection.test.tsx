import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UniversalProvidersSection } from "@/components/settings/UniversalProvidersSection";
import type { UniversalProvider } from "@/types";

const api = vi.hoisted(() => ({
  getUniversalAll: vi.fn(),
  upsertUniversal: vi.fn(),
  syncUniversal: vi.fn(),
  deleteUniversal: vi.fn(),
  previewUniversal: vi.fn(),
}));

vi.mock("@/lib/api", () => ({ providersApi: api }));

const existing: UniversalProvider = {
  id: "gateway",
  name: "Gateway",
  providerType: "newapi",
  apps: { claude: true, codex: true, gemini: true },
  baseUrl: "https://gateway.example.com",
  apiKey: "secret",
  models: {},
};

function input(id: string): HTMLInputElement {
  const element = document.getElementById(id);
  if (!(element instanceof HTMLInputElement)) throw new Error(`Missing ${id}`);
  return element;
}

describe("UniversalProvidersSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.getUniversalAll.mockResolvedValue({});
    api.upsertUniversal.mockResolvedValue(true);
    api.syncUniversal.mockResolvedValue(true);
    api.previewUniversal.mockResolvedValue({});
  });

  it("creates a NewAPI preset and automatically syncs it", async () => {
    render(<UniversalProvidersSection />);
    fireEvent.click(await screen.findByRole("button", { name: "NewAPI" }));
    fireEvent.change(input("cc-switch-universal-id"), {
      target: { value: "newapi-main" },
    });
    fireEvent.change(input("cc-switch-universal-base-url"), {
      target: { value: "https://api.example.com" },
    });
    fireEvent.change(input("cc-switch-universal-api-key"), {
      target: { value: "sk-test" },
    });
    fireEvent.click(screen.getByRole("button", { name: /^保存$/ }));

    await waitFor(() => expect(api.upsertUniversal).toHaveBeenCalledTimes(1));
    expect(api.syncUniversal).toHaveBeenCalledWith("newapi-main");
    expect(api.upsertUniversal.mock.calls[0][0]).toMatchObject({
      providerType: "newapi",
      models: {
        claude: { model: "claude-sonnet-4-6" },
        codex: { model: "gpt-5.4", reasoningEffort: "high" },
        gemini: { model: "gemini-3.1-pro" },
      },
    });
  });

  it("requires confirmation before syncing an existing provider", async () => {
    api.getUniversalAll.mockResolvedValue({ gateway: existing });
    render(<UniversalProvidersSection />);
    fireEvent.click(
      await screen.findByRole("button", { name: /^Gateway gateway$/ }),
    );
    fireEvent.click(screen.getByRole("button", { name: /同步到应用/ }));
    expect(api.syncUniversal).not.toHaveBeenCalled();
    expect(screen.getByText(/将按当前保存配置更新/)).toBeInTheDocument();
    fireEvent.click(
      screen.getAllByRole("button", { name: /同步到应用/ }).at(-1)!,
    );
    await waitFor(() =>
      expect(api.syncUniversal).toHaveBeenCalledWith("gateway"),
    );
  });

  it("uses backend conversion for preview and masks credentials", async () => {
    api.getUniversalAll.mockResolvedValue({ gateway: existing });
    api.previewUniversal.mockResolvedValue({
      claude: { settingsConfig: { env: { ANTHROPIC_AUTH_TOKEN: "secret" } } },
    });
    render(<UniversalProvidersSection />);
    fireEvent.click(
      await screen.findByRole("button", { name: /^Gateway gateway$/ }),
    );
    fireEvent.click(screen.getByRole("button", { name: /配置预览/ }));
    await waitFor(() =>
      expect(api.previewUniversal).toHaveBeenCalledWith(existing),
    );
    expect(screen.getByText(/\*\*\*\*\*\*\*\*/)).toBeInTheDocument();
    expect(screen.queryByText(/"secret"/)).not.toBeInTheDocument();
  });
});
