import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { ProviderActions } from "@/components/providers/ProviderActions";

const tMock = vi.fn((key: string) => {
  if (key === "provider.inUse") {
    return "使用中";
  }
  if (key === "provider.enable") {
    return "启用";
  }
  if (key === "streamCheck.action") {
    return "流式健康检查";
  }
  return key;
});

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

const renderActions = (
  overrides: Partial<ComponentProps<typeof ProviderActions>> = {},
) => {
  const props = {
    isCurrent: false,
    onSwitch: vi.fn(),
    onEdit: vi.fn(),
    onConfigureUsage: vi.fn(),
    onDelete: vi.fn(),
    ...overrides,
  };

  render(<ProviderActions {...props} />);

  return props;
};

beforeEach(() => {
  tMock.mockClear();
});

describe("ProviderActions", () => {
  it("renders all action buttons", () => {
    renderActions();

    expect(screen.getByRole("button", { name: "启用" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "common.edit" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "provider.configureUsage" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "common.delete" }),
    ).toBeInTheDocument();
  });

  it("shows 使用中 and disables switch button when current", () => {
    renderActions({ isCurrent: true });

    const switchButton = screen.getByRole("button", { name: "使用中" });
    expect(switchButton).toBeDisabled();
  });

  it("shows 启用 when not current", () => {
    renderActions({ isCurrent: false });

    const switchButton = screen.getByRole("button", { name: "启用" });
    expect(switchButton).toBeEnabled();
  });

  it("calls callbacks when buttons are clicked", async () => {
    const user = userEvent.setup();
    const props = renderActions({ onStreamCheck: vi.fn() });

    await user.click(screen.getByRole("button", { name: "启用" }));
    await user.click(screen.getByRole("button", { name: "common.edit" }));
    await user.click(
      screen.getByRole("button", { name: "provider.configureUsage" }),
    );
    await user.click(screen.getByRole("button", { name: "流式健康检查" }));
    await user.click(screen.getByRole("button", { name: "common.delete" }));

    expect(props.onSwitch).toHaveBeenCalledTimes(1);
    expect(props.onEdit).toHaveBeenCalledTimes(1);
    expect(props.onConfigureUsage).toHaveBeenCalledTimes(1);
    expect(props.onStreamCheck).toHaveBeenCalledTimes(1);
    expect(props.onDelete).toHaveBeenCalledTimes(1);
  });

  it("hides and disables stream check action according to props", () => {
    renderActions();
    expect(
      screen.queryByRole("button", { name: "流式健康检查" }),
    ).not.toBeInTheDocument();

    renderActions({ onStreamCheck: vi.fn(), isStreamChecking: true });
    expect(screen.getByRole("button", { name: "流式健康检查" })).toBeDisabled();
  });

  it("hides usage action when usage is unsupported", () => {
    renderActions({ showUsageActions: false });

    expect(
      screen.queryByRole("button", { name: "provider.configureUsage" }),
    ).not.toBeInTheDocument();
  });

  it("disables delete styling and ignores clicks when current", async () => {
    const user = userEvent.setup();
    const props = renderActions({ isCurrent: true });

    const deleteButton = screen.getByRole("button", { name: "common.delete" });
    expect(deleteButton).toHaveClass("cursor-not-allowed");
    expect(deleteButton).toHaveClass("opacity-40");

    await user.click(deleteButton);
    expect(props.onDelete).not.toHaveBeenCalled();
  });
});
