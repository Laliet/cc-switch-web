import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import type { Provider } from "@/types";
import { ProviderList } from "@/components/providers/ProviderList";

const useDragSortMock = vi.fn();
const useSortableMock = vi.fn();
const providerCardRenderSpy = vi.fn();
const getOmoPluginStatusMock = vi.fn();
const getOmoSlimPluginStatusMock = vi.fn();
const disableCurrentOmoMock = vi.fn();
const disableCurrentOmoSlimMock = vi.fn();
const checkProviderMock = vi.fn();
const isCheckingMock = vi.fn();

vi.mock("@/hooks/useDragSort", () => ({
  useDragSort: (...args: unknown[]) => useDragSortMock(...args),
}));

vi.mock("@/components/providers/ProviderCard", () => ({
  ProviderCard: (props: any) => {
    providerCardRenderSpy(props);
    const {
      provider,
      onSwitch,
      onEdit,
      onDelete,
      onDuplicate,
      onConfigureUsage,
      onStreamCheck,
    } = props;

    return (
      <div data-testid={`provider-card-${provider.id}`}>
        <button
          data-testid={`switch-${provider.id}`}
          onClick={() => onSwitch(provider)}
        >
          switch
        </button>
        <button
          data-testid={`edit-${provider.id}`}
          onClick={() => onEdit(provider)}
        >
          edit
        </button>
        <button
          data-testid={`duplicate-${provider.id}`}
          onClick={() => onDuplicate(provider)}
        >
          duplicate
        </button>
        <button
          data-testid={`usage-${provider.id}`}
          onClick={() => onConfigureUsage(provider)}
        >
          usage
        </button>
        <button
          data-testid={`stream-check-${provider.id}`}
          data-enabled={onStreamCheck ? "true" : "false"}
          data-checking={props.isStreamChecking ? "true" : "false"}
          onClick={() => onStreamCheck?.(provider)}
        >
          stream-check
        </button>
        <button
          data-testid={`delete-${provider.id}`}
          onClick={() => onDelete(provider)}
        >
          delete
        </button>
        <span data-testid={`is-current-${provider.id}`}>
          {props.isCurrent ? "current" : "inactive"}
        </span>
        <span data-testid={`edit-mode-${provider.id}`}>
          {props.isEditMode ? "edit-mode" : "view-mode"}
        </span>
        <span data-testid={`drag-attr-${provider.id}`}>
          {props.dragHandleProps?.attributes?.["data-dnd-id"] ?? "none"}
        </span>
      </div>
    );
  },
}));

vi.mock("@/lib/api", () => ({
  providersApi: {
    getOmoPluginStatus: (...args: unknown[]) => getOmoPluginStatusMock(...args),
    getOmoSlimPluginStatus: (...args: unknown[]) =>
      getOmoSlimPluginStatusMock(...args),
    disableCurrentOmo: (...args: unknown[]) => disableCurrentOmoMock(...args),
    disableCurrentOmoSlim: (...args: unknown[]) =>
      disableCurrentOmoSlimMock(...args),
  },
}));

vi.mock("@/hooks/useStreamCheck", () => ({
  useStreamCheck: () => ({
    checkProvider: (...args: unknown[]) => checkProviderMock(...args),
    isChecking: (...args: unknown[]) => isCheckingMock(...args),
  }),
}));

vi.mock("@/components/UsageFooter", () => ({
  default: () => <div data-testid="usage-footer" />,
}));

vi.mock("@dnd-kit/sortable", async () => {
  const actual = await vi.importActual<any>("@dnd-kit/sortable");

  return {
    ...actual,
    useSortable: (...args: unknown[]) => useSortableMock(...args),
  };
});

function createProvider(overrides: Partial<Provider> = {}): Provider {
  return {
    id: overrides.id ?? "provider-1",
    name: overrides.name ?? "Test Provider",
    settingsConfig: overrides.settingsConfig ?? {},
    category: overrides.category,
    createdAt: overrides.createdAt,
    sortIndex: overrides.sortIndex,
    meta: overrides.meta,
    websiteUrl: overrides.websiteUrl,
  };
}

beforeEach(() => {
  useDragSortMock.mockReset();
  useSortableMock.mockReset();
  providerCardRenderSpy.mockClear();
  getOmoPluginStatusMock.mockReset();
  getOmoPluginStatusMock.mockResolvedValue(false);
  getOmoSlimPluginStatusMock.mockReset();
  getOmoSlimPluginStatusMock.mockResolvedValue(false);
  disableCurrentOmoMock.mockReset();
  disableCurrentOmoMock.mockResolvedValue(true);
  disableCurrentOmoSlimMock.mockReset();
  disableCurrentOmoSlimMock.mockResolvedValue(true);
  checkProviderMock.mockReset();
  checkProviderMock.mockResolvedValue(null);
  isCheckingMock.mockReset();
  isCheckingMock.mockReturnValue(false);

  useSortableMock.mockImplementation(({ id }: { id: string }) => ({
    setNodeRef: vi.fn(),
    attributes: { "data-dnd-id": id },
    listeners: { onPointerDown: vi.fn() },
    transform: null,
    transition: null,
    isDragging: false,
  }));

  useDragSortMock.mockReturnValue({
    sortedProviders: [],
    sensors: [],
    handleDragEnd: vi.fn(),
  });
});

describe("ProviderList Component", () => {
  it("should render skeleton placeholders when loading", () => {
    const { container } = render(
      <ProviderList
        providers={{}}
        currentProviderId=""
        appId="claude"
        onSwitch={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
        onOpenWebsite={vi.fn()}
        isLoading
      />,
    );

    const placeholders = container.querySelectorAll(
      ".border-dashed.border-muted-foreground\\/40",
    );
    expect(placeholders).toHaveLength(3);
  });

  it("should show empty state and trigger create callback when no providers exist", () => {
    const handleCreate = vi.fn();
    useDragSortMock.mockReturnValueOnce({
      sortedProviders: [],
      sensors: [],
      handleDragEnd: vi.fn(),
    });

    render(
      <ProviderList
        providers={{}}
        currentProviderId=""
        appId="claude"
        onSwitch={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
        onOpenWebsite={vi.fn()}
        onCreate={handleCreate}
      />,
    );

    const addButton = screen.getByRole("button", {
      name: "provider.addProvider",
    });
    fireEvent.click(addButton);

    expect(handleCreate).toHaveBeenCalledTimes(1);
  });

  it("should render in order returned by useDragSort and pass through action callbacks", () => {
    const providerA = createProvider({ id: "a", name: "A" });
    const providerB = createProvider({ id: "b", name: "B" });

    const handleSwitch = vi.fn();
    const handleEdit = vi.fn();
    const handleDelete = vi.fn();
    const handleDuplicate = vi.fn();
    const handleUsage = vi.fn();
    const handleOpenWebsite = vi.fn();

    useDragSortMock.mockReturnValue({
      sortedProviders: [providerB, providerA],
      sensors: [],
      handleDragEnd: vi.fn(),
    });

    render(
      <ProviderList
        providers={{ a: providerA, b: providerB }}
        currentProviderId="b"
        appId="claude"
        isEditMode
        onSwitch={handleSwitch}
        onEdit={handleEdit}
        onDelete={handleDelete}
        onDuplicate={handleDuplicate}
        onConfigureUsage={handleUsage}
        onOpenWebsite={handleOpenWebsite}
      />,
    );

    // Verify sort order
    expect(providerCardRenderSpy).toHaveBeenCalledTimes(2);
    expect(providerCardRenderSpy.mock.calls[0][0].provider.id).toBe("b");
    expect(providerCardRenderSpy.mock.calls[1][0].provider.id).toBe("a");

    // Verify current provider marker and edit mode pass-through
    expect(providerCardRenderSpy.mock.calls[0][0].isCurrent).toBe(true);
    expect(providerCardRenderSpy.mock.calls[0][0].isEditMode).toBe(true);

    // Drag attributes from useSortable
    expect(
      providerCardRenderSpy.mock.calls[0][0].dragHandleProps?.attributes[
        "data-dnd-id"
      ],
    ).toBe("b");
    expect(
      providerCardRenderSpy.mock.calls[1][0].dragHandleProps?.attributes[
        "data-dnd-id"
      ],
    ).toBe("a");

    // Trigger action buttons
    fireEvent.click(screen.getByTestId("switch-b"));
    fireEvent.click(screen.getByTestId("edit-b"));
    fireEvent.click(screen.getByTestId("duplicate-b"));
    fireEvent.click(screen.getByTestId("usage-b"));
    fireEvent.click(screen.getByTestId("delete-a"));

    expect(handleSwitch).toHaveBeenCalledWith(providerB);
    expect(handleEdit).toHaveBeenCalledWith(providerB);
    expect(handleDuplicate).toHaveBeenCalledWith(providerB);
    expect(handleUsage).toHaveBeenCalledWith(providerB);
    expect(handleDelete).toHaveBeenCalledWith(providerA);

    // Verify useDragSort call parameters
    expect(useDragSortMock).toHaveBeenCalledWith(
      { a: providerA, b: providerB },
      "claude",
    );
  });

  it("shows OMO plugin status from opencode.json", async () => {
    const provider = createProvider({ id: "omo-1", name: "OMO" });
    getOmoPluginStatusMock.mockResolvedValueOnce(true);
    useDragSortMock.mockReturnValue({
      sortedProviders: [provider],
      sensors: [],
      handleDragEnd: vi.fn(),
    });

    render(
      <ProviderList
        providers={{ "omo-1": provider }}
        currentProviderId="omo-1"
        appId="omo"
        onSwitch={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
        onOpenWebsite={vi.fn()}
      />,
    );

    expect(getOmoPluginStatusMock).toHaveBeenCalledTimes(1);
    expect(
      await screen.findByText("已在 opencode.json plugin 中启用"),
    ).toBeInTheDocument();
  });

  it("disables current OMO and refreshes the parent state", async () => {
    const provider = createProvider({ id: "omo-1", name: "OMO" });
    const handleOmoDisabled = vi.fn();
    getOmoPluginStatusMock.mockResolvedValueOnce(true);
    useDragSortMock.mockReturnValue({
      sortedProviders: [provider],
      sensors: [],
      handleDragEnd: vi.fn(),
    });

    render(
      <ProviderList
        providers={{ "omo-1": provider }}
        currentProviderId="omo-1"
        appId="omo"
        onSwitch={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
        onOpenWebsite={vi.fn()}
        onOmoDisabled={handleOmoDisabled}
      />,
    );

    fireEvent.click(
      await screen.findByRole("button", { name: /禁用当前 OMO/ }),
    );

    await waitFor(() => {
      expect(disableCurrentOmoMock).toHaveBeenCalledTimes(1);
      expect(handleOmoDisabled).toHaveBeenCalledTimes(1);
    });
    expect(
      await screen.findByText("未在 opencode.json plugin 中启用"),
    ).toBeInTheDocument();
  });

  it("disables current OMO Slim", async () => {
    const provider = createProvider({ id: "omo-slim-1", name: "OMO Slim" });
    getOmoSlimPluginStatusMock.mockResolvedValueOnce(true);
    useDragSortMock.mockReturnValue({
      sortedProviders: [provider],
      sensors: [],
      handleDragEnd: vi.fn(),
    });

    render(
      <ProviderList
        providers={{ "omo-slim-1": provider }}
        currentProviderId="omo-slim-1"
        appId="omo-slim"
        onSwitch={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
        onOpenWebsite={vi.fn()}
      />,
    );

    expect(
      await screen.findByText("oh-my-opencode-slim@latest"),
    ).toBeInTheDocument();
    expect(getOmoPluginStatusMock).not.toHaveBeenCalled();
    expect(getOmoSlimPluginStatusMock).toHaveBeenCalledTimes(1);

    fireEvent.click(
      await screen.findByRole("button", { name: /禁用当前 OMO Slim/ }),
    );

    await waitFor(() => {
      expect(disableCurrentOmoSlimMock).toHaveBeenCalledTimes(1);
    });
    expect(disableCurrentOmoMock).not.toHaveBeenCalled();
  });

  it("passes stream check action for direct provider apps", () => {
    const provider = createProvider({ id: "opencode-1", name: "OpenCode" });
    isCheckingMock.mockImplementation((id: string) => id === "opencode-1");
    useDragSortMock.mockReturnValue({
      sortedProviders: [provider],
      sensors: [],
      handleDragEnd: vi.fn(),
    });

    render(
      <ProviderList
        providers={{ "opencode-1": provider }}
        currentProviderId="opencode-1"
        appId="opencode"
        onSwitch={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
        onOpenWebsite={vi.fn()}
      />,
    );

    const button = screen.getByTestId("stream-check-opencode-1");
    expect(button).toHaveAttribute("data-enabled", "true");
    expect(button).toHaveAttribute("data-checking", "true");

    fireEvent.click(button);
    expect(checkProviderMock).toHaveBeenCalledWith("opencode-1", "OpenCode");
  });

  it("does not expose stream check action for OMO profile apps", async () => {
    const provider = createProvider({ id: "omo-slim-1", name: "OMO Slim" });
    useDragSortMock.mockReturnValue({
      sortedProviders: [provider],
      sensors: [],
      handleDragEnd: vi.fn(),
    });

    render(
      <ProviderList
        providers={{ "omo-slim-1": provider }}
        currentProviderId="omo-slim-1"
        appId="omo-slim"
        onSwitch={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
        onOpenWebsite={vi.fn()}
      />,
    );

    expect(
      await screen.findByText("oh-my-opencode-slim@latest"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("stream-check-omo-slim-1")).toHaveAttribute(
      "data-enabled",
      "false",
    );
  });
});
