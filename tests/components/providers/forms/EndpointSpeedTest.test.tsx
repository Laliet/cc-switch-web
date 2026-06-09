import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import EndpointSpeedTest from "@/components/providers/forms/EndpointSpeedTest";

const tMock = vi.fn((key: string) => key);
const getCustomEndpointsMock = vi.hoisted(() => vi.fn());
const addCustomEndpointMock = vi.hoisted(() => vi.fn());
const removeCustomEndpointMock = vi.hoisted(() => vi.fn());
const testApiEndpointsMock = vi.hoisted(() => vi.fn());

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

vi.mock("@/lib/api/vscode", () => ({
  vscodeApi: {
    getCustomEndpoints: (...args: unknown[]) => getCustomEndpointsMock(...args),
    addCustomEndpoint: (...args: unknown[]) => addCustomEndpointMock(...args),
    removeCustomEndpoint: (...args: unknown[]) =>
      removeCustomEndpointMock(...args),
    testApiEndpoints: (...args: unknown[]) => testApiEndpointsMock(...args),
  },
}));

vi.mock("@/components/ui/dialog", () => ({
  Dialog: ({ children, open }: any) =>
    open ? <div data-testid="dialog">{children}</div> : null,
  DialogContent: ({ children }: any) => <div>{children}</div>,
  DialogHeader: ({ children }: any) => <div>{children}</div>,
  DialogTitle: ({ children }: any) => <h2>{children}</h2>,
  DialogDescription: ({ children }: any) => <p>{children}</p>,
  DialogFooter: ({ children }: any) => <div>{children}</div>,
}));

vi.mock("@/components/ui/button", () => ({
  Button: ({ children, onClick, disabled, type = "button", ...rest }: any) => (
    <button type={type} onClick={onClick} disabled={disabled} {...rest}>
      {children}
    </button>
  ),
}));

vi.mock("@/components/ui/input", () => ({
  Input: ({ value, onChange, onKeyDown, ...rest }: any) => (
    <input value={value} onChange={onChange} onKeyDown={onKeyDown} {...rest} />
  ),
}));

const defaultProps = {
  appId: "claude" as const,
  value: "",
  onChange: vi.fn(),
  initialEndpoints: [],
  visible: true,
  onClose: vi.fn(),
};

beforeEach(() => {
  tMock.mockClear();
  defaultProps.onChange.mockClear();
  defaultProps.onClose.mockClear();
  getCustomEndpointsMock.mockReset();
  addCustomEndpointMock.mockReset();
  removeCustomEndpointMock.mockReset();
  testApiEndpointsMock.mockReset();

  getCustomEndpointsMock.mockResolvedValue([]);
  addCustomEndpointMock.mockResolvedValue(undefined);
  removeCustomEndpointMock.mockResolvedValue(undefined);
  testApiEndpointsMock.mockResolvedValue([]);
});

describe("EndpointSpeedTest", () => {
  it("renders dialog when visible", () => {
    render(<EndpointSpeedTest {...defaultProps} />);

    expect(screen.getByTestId("dialog")).toBeInTheDocument();
    expect(screen.getByText("endpointTest.title")).toBeInTheDocument();
  });

  it("does not render when not visible", () => {
    render(<EndpointSpeedTest {...defaultProps} visible={false} />);

    expect(screen.queryByTestId("dialog")).not.toBeInTheDocument();
  });

  it("shows empty state when no endpoints", () => {
    render(<EndpointSpeedTest {...defaultProps} />);

    expect(screen.getByText("endpointTest.noEndpoints")).toBeInTheDocument();
  });

  it("renders initial endpoints", () => {
    const endpoints = [
      { url: "https://api.example.com", isCustom: false },
      { url: "https://api2.example.com", isCustom: true },
    ];

    render(
      <EndpointSpeedTest {...defaultProps} initialEndpoints={endpoints} />,
    );

    expect(screen.getByText("https://api.example.com")).toBeInTheDocument();
    expect(screen.getByText("https://api2.example.com")).toBeInTheDocument();
  });

  it("adds custom endpoint via input", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    render(<EndpointSpeedTest {...defaultProps} onChange={onChange} />);

    const input = screen.getByPlaceholderText(
      "endpointTest.addEndpointPlaceholder",
    );
    await user.type(input, "https://custom.api.com");

    // Find the add button (icon-only button next to input)
    const buttons = screen.getAllByRole("button");
    const addButton = buttons.find(
      (btn) =>
        btn.getAttribute("variant") === "outline" && btn.querySelector("svg"),
    );
    expect(addButton).toBeTruthy();
    await user.click(addButton!);

    await waitFor(() => {
      expect(screen.getByText("https://custom.api.com")).toBeInTheDocument();
    });
    expect(onChange).toHaveBeenCalledWith("https://custom.api.com");
  });

  it("shows error for invalid URL", async () => {
    const user = userEvent.setup();

    render(<EndpointSpeedTest {...defaultProps} />);

    const input = screen.getByPlaceholderText(
      "endpointTest.addEndpointPlaceholder",
    );
    await user.type(input, "not-a-valid-url");

    const buttons = screen.getAllByRole("button");
    const addButton = buttons.find(
      (btn) =>
        btn.getAttribute("variant") === "outline" && btn.querySelector("svg"),
    );
    await user.click(addButton!);

    await waitFor(() => {
      expect(
        screen.getByText("endpointTest.invalidUrlFormat"),
      ).toBeInTheDocument();
    });
  });

  it("shows error for empty URL", async () => {
    const user = userEvent.setup();

    render(<EndpointSpeedTest {...defaultProps} />);

    const buttons = screen.getAllByRole("button");
    const addButton = buttons.find(
      (btn) =>
        btn.getAttribute("variant") === "outline" && btn.querySelector("svg"),
    );
    await user.click(addButton!);

    await waitFor(() => {
      expect(
        screen.getByText("endpointTest.enterValidUrl"),
      ).toBeInTheDocument();
    });
  });

  it("shows error for duplicate URL", async () => {
    const user = userEvent.setup();
    const endpoints = [{ url: "https://api.example.com", isCustom: false }];

    render(
      <EndpointSpeedTest {...defaultProps} initialEndpoints={endpoints} />,
    );

    const input = screen.getByPlaceholderText(
      "endpointTest.addEndpointPlaceholder",
    );
    await user.type(input, "https://api.example.com");

    const buttons = screen.getAllByRole("button");
    const addButton = buttons.find(
      (btn) =>
        btn.getAttribute("variant") === "outline" && btn.querySelector("svg"),
    );
    await user.click(addButton!);

    await waitFor(() => {
      expect(screen.getByText("endpointTest.urlExists")).toBeInTheDocument();
    });
  });

  it("shows error for non-http protocol", async () => {
    const user = userEvent.setup();

    render(<EndpointSpeedTest {...defaultProps} />);

    const input = screen.getByPlaceholderText(
      "endpointTest.addEndpointPlaceholder",
    );
    await user.type(input, "ftp://api.example.com");

    const buttons = screen.getAllByRole("button");
    const addButton = buttons.find(
      (btn) =>
        btn.getAttribute("variant") === "outline" && btn.querySelector("svg"),
    );
    await user.click(addButton!);

    await waitFor(() => {
      expect(screen.getByText("endpointTest.onlyHttps")).toBeInTheDocument();
    });
  });

  it("selects endpoint on click", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const endpoints = [
      { url: "https://api1.example.com", isCustom: false },
      { url: "https://api2.example.com", isCustom: false },
    ];

    render(
      <EndpointSpeedTest
        {...defaultProps}
        initialEndpoints={endpoints}
        onChange={onChange}
        value="https://api1.example.com"
      />,
    );

    await user.click(screen.getByText("https://api2.example.com"));

    expect(onChange).toHaveBeenCalledWith("https://api2.example.com");
  });

  it("runs speed test and updates latencies", async () => {
    const user = userEvent.setup();
    const endpoints = [
      { url: "https://api1.example.com", isCustom: false },
      { url: "https://api2.example.com", isCustom: false },
    ];

    testApiEndpointsMock.mockResolvedValueOnce([
      { url: "https://api1.example.com", latency: 150, status: 200 },
      { url: "https://api2.example.com", latency: 300, status: 200 },
    ]);

    render(
      <EndpointSpeedTest {...defaultProps} initialEndpoints={endpoints} />,
    );

    await user.click(screen.getByText("endpointTest.testSpeed"));

    await waitFor(() => {
      expect(screen.getByText("150ms")).toBeInTheDocument();
      expect(screen.getByText("300ms")).toBeInTheDocument();
    });
  });

  it("auto-selects fastest endpoint after speed test", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const endpoints = [
      { url: "https://slow.example.com", isCustom: false },
      { url: "https://fast.example.com", isCustom: false },
    ];

    testApiEndpointsMock.mockResolvedValueOnce([
      { url: "https://slow.example.com", latency: 500, status: 200 },
      { url: "https://fast.example.com", latency: 100, status: 200 },
    ]);

    render(
      <EndpointSpeedTest
        {...defaultProps}
        initialEndpoints={endpoints}
        onChange={onChange}
        value="https://slow.example.com"
      />,
    );

    await user.click(screen.getByText("endpointTest.testSpeed"));

    await waitFor(() => {
      expect(onChange).toHaveBeenCalledWith("https://fast.example.com");
    });
  });

  it("handles speed test error", async () => {
    const user = userEvent.setup();
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const endpoints = [{ url: "https://api.example.com", isCustom: false }];

    testApiEndpointsMock.mockRejectedValueOnce(new Error("Network error"));

    render(
      <EndpointSpeedTest {...defaultProps} initialEndpoints={endpoints} />,
    );

    await user.click(screen.getByText("endpointTest.testSpeed"));

    await waitFor(() => {
      expect(screen.getByText("Network error")).toBeInTheDocument();
    });

    consoleSpy.mockRestore();
  });

  it("removes endpoint", async () => {
    const user = userEvent.setup();
    const endpoints = [{ url: "https://api.example.com", isCustom: true }];

    render(
      <EndpointSpeedTest {...defaultProps} initialEndpoints={endpoints} />,
    );

    expect(screen.getByText("https://api.example.com")).toBeInTheDocument();

    // Find the X button inside the endpoint row
    const endpointRow = screen
      .getByText("https://api.example.com")
      .closest("div[class*='cursor-pointer']");
    const removeButton = endpointRow?.querySelector(
      "button[class*='opacity-0']",
    );

    if (removeButton) {
      await user.click(removeButton);
    }

    await waitFor(() => {
      expect(
        screen.queryByText("https://api.example.com"),
      ).not.toBeInTheDocument();
    });
  });

  it("calls onClose when cancel button clicked", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();

    render(<EndpointSpeedTest {...defaultProps} onClose={onClose} />);

    await user.click(screen.getByText("common.cancel"));

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("saves and closes on save button click", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();

    render(<EndpointSpeedTest {...defaultProps} onClose={onClose} />);

    await user.click(screen.getByText("common.save"));

    await waitFor(() => {
      expect(onClose).toHaveBeenCalledTimes(1);
    });
  });

  it("loads custom endpoints in edit mode", async () => {
    getCustomEndpointsMock.mockResolvedValueOnce([
      { url: "https://saved.example.com", addedAt: Date.now() },
    ]);

    render(
      <EndpointSpeedTest
        {...defaultProps}
        providerId="provider-123"
        initialEndpoints={[
          { url: "https://preset.example.com", isCustom: false },
        ]}
      />,
    );

    await waitFor(() => {
      expect(getCustomEndpointsMock).toHaveBeenCalledWith(
        "claude",
        "provider-123",
      );
    });

    await waitFor(() => {
      expect(screen.getByText("https://saved.example.com")).toBeInTheDocument();
    });
  });

  it("saves endpoint changes in edit mode", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();

    getCustomEndpointsMock.mockResolvedValueOnce([]);

    render(
      <EndpointSpeedTest
        {...defaultProps}
        providerId="provider-123"
        onClose={onClose}
      />,
    );

    // Add a new endpoint
    const input = screen.getByPlaceholderText(
      "endpointTest.addEndpointPlaceholder",
    );
    await user.type(input, "https://new.example.com");

    const buttons = screen.getAllByRole("button");
    const addButton = buttons.find(
      (btn) =>
        btn.getAttribute("variant") === "outline" && btn.querySelector("svg"),
    );
    await user.click(addButton!);

    await waitFor(() => {
      expect(screen.getByText("https://new.example.com")).toBeInTheDocument();
    });

    // Save
    await user.click(screen.getByText("common.save"));

    await waitFor(() => {
      expect(addCustomEndpointMock).toHaveBeenCalledWith(
        "claude",
        "provider-123",
        "https://new.example.com",
      );
    });
  });

  it("calls onCustomEndpointsChange in create mode", async () => {
    const user = userEvent.setup();
    const onCustomEndpointsChange = vi.fn();

    render(
      <EndpointSpeedTest
        {...defaultProps}
        onCustomEndpointsChange={onCustomEndpointsChange}
      />,
    );

    const input = screen.getByPlaceholderText(
      "endpointTest.addEndpointPlaceholder",
    );
    await user.type(input, "https://custom.example.com");

    const buttons = screen.getAllByRole("button");
    const addButton = buttons.find(
      (btn) =>
        btn.getAttribute("variant") === "outline" && btn.querySelector("svg"),
    );
    await user.click(addButton!);

    await waitFor(() => {
      expect(onCustomEndpointsChange).toHaveBeenCalledWith([
        "https://custom.example.com",
      ]);
    });
  });

  it("disables test button when no endpoints", () => {
    render(<EndpointSpeedTest {...defaultProps} />);

    const testButton = screen
      .getByText("endpointTest.testSpeed")
      .closest("button");
    expect(testButton).toBeDisabled();
  });

  it("toggles auto-select checkbox", async () => {
    const user = userEvent.setup();
    const endpoints = [{ url: "https://api.example.com", isCustom: false }];

    render(
      <EndpointSpeedTest {...defaultProps} initialEndpoints={endpoints} />,
    );

    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).toBeChecked();

    await user.click(checkbox);

    expect(checkbox).not.toBeChecked();
  });

  it("adds endpoint on Enter key press", async () => {
    const user = userEvent.setup();

    render(<EndpointSpeedTest {...defaultProps} />);

    const input = screen.getByPlaceholderText(
      "endpointTest.addEndpointPlaceholder",
    );
    await user.type(input, "https://enter.example.com{enter}");

    await waitFor(() => {
      expect(screen.getByText("https://enter.example.com")).toBeInTheDocument();
    });
  });

  it("normalizes URL by removing trailing slashes", async () => {
    const user = userEvent.setup();

    render(<EndpointSpeedTest {...defaultProps} />);

    const input = screen.getByPlaceholderText(
      "endpointTest.addEndpointPlaceholder",
    );
    await user.type(input, "https://api.example.com///");

    const buttons = screen.getAllByRole("button");
    const addButton = buttons.find(
      (btn) =>
        btn.getAttribute("variant") === "outline" && btn.querySelector("svg"),
    );
    await user.click(addButton!);

    await waitFor(() => {
      expect(screen.getByText("https://api.example.com")).toBeInTheDocument();
    });
  });

  it("shows endpoint count", () => {
    const endpoints = [
      { url: "https://api1.example.com", isCustom: false },
      { url: "https://api2.example.com", isCustom: false },
    ];

    render(
      <EndpointSpeedTest {...defaultProps} initialEndpoints={endpoints} />,
    );

    // The count and label are in the same div: "2 endpointTest.endpoints"
    expect(screen.getByText(/2\s+endpointTest\.endpoints/)).toBeInTheDocument();
  });
});
