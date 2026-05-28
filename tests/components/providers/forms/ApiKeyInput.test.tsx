import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import ApiKeyInput from "@/components/providers/forms/ApiKeyInput";

const tMock = vi.fn((key: string) => key);

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

const renderApiKeyInput = (
  overrides: Partial<ComponentProps<typeof ApiKeyInput>> = {},
) => {
  const props = {
    value: "",
    onChange: vi.fn(),
    ...overrides,
  };

  render(<ApiKeyInput {...props} />);

  return props;
};

beforeEach(() => {
  tMock.mockClear();
});

describe("ApiKeyInput", () => {
  it("renders label and input", () => {
    renderApiKeyInput({ label: "Secret Key", id: "secret-key" });

    expect(screen.getByText("Secret Key")).toBeInTheDocument();
    expect(screen.getByLabelText("Secret Key")).toBeInTheDocument();
  });

  it("masks the key by default without using a password input", () => {
    renderApiKeyInput({ value: "token" });

    const input = screen.getByLabelText("API Key") as HTMLInputElement;
    expect(input.type).toBe("text");
    expect(input).toHaveClass("secret-text-security");
  });

  it("toggles visibility when the eye button is clicked", async () => {
    const user = userEvent.setup();
    renderApiKeyInput({ value: "token" });

    const input = screen.getByLabelText("API Key") as HTMLInputElement;
    const toggleButton = screen.getByRole("button", {
      name: "apiKeyInput.show",
    });

    await user.click(toggleButton);

    expect(input.type).toBe("text");
    expect(input).not.toHaveClass("secret-text-security");
    expect(
      screen.getByRole("button", { name: "apiKeyInput.hide" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "apiKeyInput.hide" }));

    expect(input.type).toBe("text");
    expect(input).toHaveClass("secret-text-security");
    expect(
      screen.getByRole("button", { name: "apiKeyInput.show" }),
    ).toBeInTheDocument();
  });

  it("does not render the toggle button when disabled", () => {
    renderApiKeyInput({ value: "token", disabled: true });

    expect(
      screen.queryByRole("button", { name: "apiKeyInput.show" }),
    ).toBeNull();
  });

  it("calls onChange with the new value", () => {
    const { onChange } = renderApiKeyInput();

    fireEvent.change(screen.getByLabelText("API Key"), {
      target: { value: "new-key" },
    });

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("new-key");
  });

  it("shows a required asterisk when required", () => {
    renderApiKeyInput({ label: "Secret Key", required: true });

    expect(screen.getByText(/Secret Key\s*\*/)).toBeInTheDocument();
  });
});
