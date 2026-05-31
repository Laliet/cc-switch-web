import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  DraggableAttributes,
  DraggableSyntheticListeners,
} from "@dnd-kit/core";
import type { Provider } from "@/types";
import type { ProviderHealth } from "@/lib/api";
import { ProviderCard } from "@/components/providers/ProviderCard";

type DragHandleProps = {
  attributes: DraggableAttributes & { "data-dnd-id": string };
  listeners: DraggableSyntheticListeners;
  isDragging: boolean;
};

const tMock = vi.fn((key: string) => key);

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

vi.mock("@/components/UsageFooter", () => ({
  default: () => <div data-testid="usage-footer" />,
}));

const providerActionsSpy = vi.fn();

vi.mock("@/components/providers/ProviderActions", () => ({
  ProviderActions: (props: any) => {
    providerActionsSpy(props);
    return <div data-testid="provider-actions" />;
  },
}));

const createProvider = (overrides: Partial<Provider> = {}): Provider => ({
  id: overrides.id ?? "provider-1",
  name: overrides.name ?? "Test Provider",
  settingsConfig: overrides.settingsConfig ?? {},
  category: overrides.category,
  createdAt: overrides.createdAt,
  sortIndex: overrides.sortIndex,
  meta: overrides.meta,
  websiteUrl: overrides.websiteUrl,
  notes: overrides.notes,
  isPartner: overrides.isPartner,
});

const createDragHandleProps = (
  overrides: Partial<DragHandleProps> = {},
): DragHandleProps => {
  const attributes: DraggableAttributes & { "data-dnd-id": string } = {
    role: "button",
    tabIndex: 0,
    "aria-pressed": undefined,
    "aria-disabled": false,
    "aria-roledescription": "sortable",
    "aria-describedby": "provider-1",
    "data-dnd-id": "provider-1",
    ...(overrides.attributes ?? {}),
  };

  return {
    attributes,
    listeners:
      overrides.listeners ??
      ({ onPointerDown: vi.fn() } as DraggableSyntheticListeners),
    isDragging: overrides.isDragging ?? false,
  };
};

const renderProviderCard = (
  providerOverrides: Partial<Provider> = {},
  options: {
    isCurrent?: boolean;
    isEditMode?: boolean;
    dragHandleProps?: DragHandleProps;
    healthStatus?: ProviderHealth;
    appId?: "claude" | "codex" | "gemini" | "opencode" | "omo";
  } = {},
) => {
  const provider = createProvider(providerOverrides);
  const onSwitch = vi.fn();
  const onEdit = vi.fn();
  const onDelete = vi.fn();
  const onConfigureUsage = vi.fn();
  const onOpenWebsite = vi.fn();
  const onDuplicate = vi.fn();

  const renderResult = render(
    <ProviderCard
      provider={provider}
      isCurrent={options.isCurrent ?? false}
      appId={options.appId ?? "claude"}
      isEditMode={options.isEditMode ?? false}
      onSwitch={onSwitch}
      onEdit={onEdit}
      onDelete={onDelete}
      onConfigureUsage={onConfigureUsage}
      onOpenWebsite={onOpenWebsite}
      onDuplicate={onDuplicate}
      dragHandleProps={options.dragHandleProps}
      healthStatus={options.healthStatus}
    />,
  );

  return {
    provider,
    onSwitch,
    onEdit,
    onDelete,
    onConfigureUsage,
    onOpenWebsite,
    onDuplicate,
    ...renderResult,
  };
};

beforeEach(() => {
  tMock.mockClear();
  providerActionsSpy.mockClear();
});

describe("ProviderCard", () => {
  it("renders provider name and current status", () => {
    renderProviderCard({ name: "Acme Provider" }, { isCurrent: true });

    expect(screen.getByText("Acme Provider")).toBeInTheDocument();
    const badge = screen.getByText("provider.currentlyUsing");
    expect(badge).toHaveClass("opacity-100");
  });

  it("prefers notes for display url and disables click", async () => {
    const user = userEvent.setup();
    const { onOpenWebsite } = renderProviderCard({
      notes: "note-url",
      websiteUrl: "https://example.com",
      settingsConfig: { env: { ANTHROPIC_BASE_URL: "https://api.example" } },
    });

    const urlButton = screen.getByRole("button", { name: "note-url" });
    expect(urlButton).toBeDisabled();

    await user.click(urlButton);

    expect(onOpenWebsite).not.toHaveBeenCalled();
  });

  it("uses websiteUrl when available and opens on click", async () => {
    const user = userEvent.setup();
    const { onOpenWebsite } = renderProviderCard({
      websiteUrl: "https://example.com",
    });

    const urlButton = screen.getByRole("button", {
      name: "https://example.com",
    });
    expect(urlButton).toBeEnabled();

    await user.click(urlButton);

    expect(onOpenWebsite).toHaveBeenCalledTimes(1);
    expect(onOpenWebsite).toHaveBeenCalledWith("https://example.com");
  });

  it("extracts base url from env config", async () => {
    const user = userEvent.setup();
    const baseUrl = "https://api.anthropic.test";
    const { onOpenWebsite } = renderProviderCard({
      settingsConfig: { env: { ANTHROPIC_BASE_URL: baseUrl } },
    });

    const urlButton = screen.getByRole("button", { name: baseUrl });
    expect(urlButton).toBeEnabled();

    await user.click(urlButton);

    expect(onOpenWebsite).toHaveBeenCalledWith(baseUrl);
  });

  it("extracts base_url from config string", async () => {
    const user = userEvent.setup();
    const baseUrl = "https://config.example";
    const { onOpenWebsite } = renderProviderCard({
      settingsConfig: { config: `base_url='${baseUrl}'` },
    });

    const urlButton = screen.getByRole("button", { name: baseUrl });
    expect(urlButton).toBeEnabled();

    await user.click(urlButton);

    expect(onOpenWebsite).toHaveBeenCalledWith(baseUrl);
  });

  it("shows fallback text when url not configured", async () => {
    const user = userEvent.setup();
    const { onOpenWebsite } = renderProviderCard();

    const urlButton = screen.getByRole("button", {
      name: "provider.notConfigured",
    });
    expect(urlButton).toBeDisabled();

    await user.click(urlButton);

    expect(onOpenWebsite).not.toHaveBeenCalled();
  });

  it("hides usage UI for omo providers", () => {
    renderProviderCard({}, { appId: "omo" });

    expect(screen.queryByTestId("usage-footer")).not.toBeInTheDocument();
    expect(providerActionsSpy).toHaveBeenCalledWith(
      expect.objectContaining({ showUsageActions: false }),
    );
  });

  it("shows routing support badge for Local Routing compatible providers", () => {
    renderProviderCard({}, { appId: "claude" });

    expect(screen.getByText("provider.routingSupport")).toBeInTheDocument();
  });

  it("does not show routing support badge for Gemini OAuth providers", () => {
    renderProviderCard({ category: "official" }, { appId: "gemini" });

    expect(screen.queryByText("provider.routingSupport")).not.toBeInTheDocument();
  });

  it("renders health indicator tooltip and availability", () => {
    const healthStatus: ProviderHealth = {
      isHealthy: true,
      status: "available",
      latency: 123.4,
      lastChecked: 1000,
      availability: 98.76,
    };

    renderProviderCard({}, { healthStatus });

    const tooltip =
      "provider.health.statusLabel: provider.health.available · " +
      "provider.health.latency: 123ms · " +
      "provider.health.availability24h: 98.8%";
    const indicator = screen.getByLabelText(tooltip);

    expect(indicator).toBeInTheDocument();
    expect(indicator).toHaveTextContent("98.8%");
    expect(
      indicator.querySelector("span[aria-hidden='true']"),
    ).toHaveClass("bg-green-500");
  });

  it("shows placeholder availability when none provided", () => {
    const healthStatus: ProviderHealth = {
      isHealthy: false,
      status: "degraded",
      latency: 45,
      lastChecked: 1000,
    };

    renderProviderCard({}, { healthStatus });

    const tooltip =
      "provider.health.statusLabel: provider.health.degraded · " +
      "provider.health.latency: 45ms · " +
      "provider.health.availability24h: provider.health.availabilityUnknown";
    const indicator = screen.getByLabelText(tooltip);

    expect(indicator).toBeInTheDocument();
    expect(indicator).toHaveTextContent("--%");
    expect(
      indicator.querySelector("span[aria-hidden='true']"),
    ).toHaveClass("bg-yellow-500");
  });

  it("shows drag handle and duplicate button in edit mode", async () => {
    const user = userEvent.setup();
    const dragHandleProps = createDragHandleProps();

    const { onDuplicate, provider } = renderProviderCard(
      {},
      { isEditMode: true, dragHandleProps },
    );

    const dragButton = screen.getByRole("button", {
      name: "provider.dragHandle",
    });
    const duplicateButton = screen.getByRole("button", {
      name: "provider.duplicate",
    });

    expect(dragButton).toBeEnabled();
    expect(dragButton).toHaveAttribute("data-dnd-id", "provider-1");
    expect(duplicateButton).toBeEnabled();

    await user.click(duplicateButton);

    expect(onDuplicate).toHaveBeenCalledTimes(1);
    expect(onDuplicate).toHaveBeenCalledWith(provider);
  });

  it("hides drag handle and duplicate button when not in edit mode", () => {
    renderProviderCard({}, { isEditMode: false });

    expect(
      screen.queryByRole("button", { name: "provider.dragHandle" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "provider.duplicate" }),
    ).not.toBeInTheDocument();
  });
});
