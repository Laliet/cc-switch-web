import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ClaudeDesktopPanel } from "@/components/providers/ClaudeDesktopPanel";

const providerApiMocks = vi.hoisted(() => ({
  getClaudeDesktopStatus: vi.fn(),
  importClaudeDesktopProvidersFromClaude: vi.fn(),
}));

const toastMocks = vi.hoisted(() => ({
  error: vi.fn(),
  success: vi.fn(),
}));

vi.mock("@/lib/api/providers", () => ({
  providersApi: providerApiMocks,
}));

vi.mock("sonner", () => ({
  toast: toastMocks,
}));

describe("ClaudeDesktopPanel", () => {
  beforeEach(() => {
    providerApiMocks.getClaudeDesktopStatus.mockReset();
    providerApiMocks.importClaudeDesktopProvidersFromClaude.mockReset();
    toastMocks.error.mockReset();
    toastMocks.success.mockReset();
  });

  it("renders status issues for a proxy profile that needs attention", async () => {
    providerApiMocks.getClaudeDesktopStatus.mockResolvedValueOnce({
      supported: true,
      configured: true,
      appliedId: "provider-1",
      profilePath: "/profiles/cc-switch/profile.json",
      configLibraryPath: "/profiles/cc-switch",
      mode: "proxy",
      expectedBaseUrl: "http://127.0.0.1:3456/claude-desktop",
      actualBaseUrl: "https://old.example.com",
      proxyRunning: false,
      staleRawModels: true,
      missingRouteMappings: true,
      gatewayTokenConfigured: false,
    });

    render(<ClaudeDesktopPanel />);

    expect(await screen.findByText("Local routing")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Claude Desktop profile base URL does not match the selected provider.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Local proxy is not running, so proxy-mode Desktop routes will fail.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("MCP / Prompt unsupported")).toBeInTheDocument();
  });

  it("imports Claude Code providers and refreshes status", async () => {
    providerApiMocks.getClaudeDesktopStatus
      .mockResolvedValueOnce({
        supported: true,
        configured: false,
        appliedId: null,
        profilePath: "/profile.json",
        configLibraryPath: "/library",
        mode: "direct",
        expectedBaseUrl: null,
        actualBaseUrl: null,
        proxyRunning: true,
        staleRawModels: false,
        missingRouteMappings: false,
        gatewayTokenConfigured: true,
      })
      .mockResolvedValueOnce({
        supported: true,
        configured: true,
        appliedId: "provider-1",
        profilePath: "/profile.json",
        configLibraryPath: "/library",
        mode: "direct",
        expectedBaseUrl: null,
        actualBaseUrl: null,
        proxyRunning: true,
        staleRawModels: false,
        missingRouteMappings: false,
        gatewayTokenConfigured: true,
      });
    providerApiMocks.importClaudeDesktopProvidersFromClaude.mockResolvedValueOnce(
      2,
    );
    const onProvidersChanged = vi.fn();

    render(<ClaudeDesktopPanel onProvidersChanged={onProvidersChanged} />);
    fireEvent.click(
      await screen.findByRole("button", { name: /Import Claude Code/ }),
    );

    await waitFor(() =>
      expect(
        providerApiMocks.importClaudeDesktopProvidersFromClaude,
      ).toHaveBeenCalledTimes(1),
    );
    expect(providerApiMocks.getClaudeDesktopStatus).toHaveBeenCalledTimes(2);
    expect(onProvidersChanged).toHaveBeenCalledTimes(1);
    expect(toastMocks.success).toHaveBeenCalledWith(
      "Imported 2 Claude Code provider(s)",
    );
  });
});
