import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ConfirmDialog } from "@/components/ConfirmDialog";

const tMock = vi.fn((key: string) => key);

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

vi.mock("@/components/ui/dialog", () => ({
  Dialog: ({ open, onOpenChange, children }: any) => (
    <div>
      {open ? <div data-testid="dialog-root">{children}</div> : null}
      <button
        type="button"
        data-testid="dialog-close"
        onClick={() => onOpenChange?.(false)}
      >
        close
      </button>
    </div>
  ),
  DialogContent: ({ children }: any) => <div>{children}</div>,
  DialogHeader: ({ children }: any) => <div>{children}</div>,
  DialogTitle: ({ children }: any) => <h2>{children}</h2>,
  DialogDescription: ({ children }: any) => <p>{children}</p>,
  DialogFooter: ({ children }: any) => <div>{children}</div>,
}));

vi.mock("@/components/ui/button", () => ({
  Button: ({ children, onClick, type = "button", ...rest }: any) => (
    <button type={type} onClick={onClick} {...rest}>
      {children}
    </button>
  ),
}));

const renderDialog = (
  overrides: Partial<ComponentProps<typeof ConfirmDialog>> = {},
) => {
  const onConfirm = overrides.onConfirm ?? vi.fn();
  const onCancel = overrides.onCancel ?? vi.fn();

  const props = {
    isOpen: true,
    title: "Confirm action",
    message: "Are you sure?",
    onConfirm,
    onCancel,
    ...overrides,
  };

  render(<ConfirmDialog {...props} />);

  return { onConfirm, onCancel };
};

beforeEach(() => {
  tMock.mockClear();
});

describe("ConfirmDialog", () => {
  it("renders title and message when open", () => {
    renderDialog({ title: "Delete file", message: "This cannot be undone." });

    expect(screen.getByText("Delete file")).toBeInTheDocument();
    expect(screen.getByText("This cannot be undone.")).toBeInTheDocument();
  });

  it("hides dialog content when closed", () => {
    renderDialog({ isOpen: false });

    expect(screen.queryByTestId("dialog-root")).not.toBeInTheDocument();
    expect(screen.queryByText("Confirm action")).not.toBeInTheDocument();
  });

  it("calls onConfirm when confirm button clicked", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    renderDialog({ onConfirm });

    await user.click(screen.getByRole("button", { name: "common.confirm" }));

    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel when cancel button clicked", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    renderDialog({ onCancel });

    await user.click(screen.getByRole("button", { name: "common.cancel" }));

    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("uses custom button text when provided", () => {
    renderDialog({ confirmText: "Proceed", cancelText: "Back" });

    expect(screen.getByRole("button", { name: "Proceed" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Back" })).toBeInTheDocument();
  });

  it("calls onCancel when dialog closes", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    renderDialog({ onCancel });

    await user.click(screen.getByTestId("dialog-close"));

    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
