import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import PromptToggle from "@/components/prompts/PromptToggle";

describe("PromptToggle", () => {
  it("shows blue background and aria-checked true when enabled", () => {
    render(<PromptToggle enabled onChange={vi.fn()} ariaLabel="toggle" />);

    const toggle = screen.getByRole("switch");
    expect(toggle).toHaveClass("bg-blue-500");
    expect(toggle).toHaveAttribute("aria-checked", "true");
  });

  it("shows gray background and aria-checked false when disabled", () => {
    render(
      <PromptToggle enabled={false} onChange={vi.fn()} ariaLabel="toggle" />,
    );

    const toggle = screen.getByRole("switch");
    expect(toggle).toHaveClass("bg-gray-300");
    expect(toggle).toHaveAttribute("aria-checked", "false");
  });

  it("calls onChange with toggled value on click", async () => {
    const onChange = vi.fn();
    render(
      <PromptToggle enabled={false} onChange={onChange} ariaLabel="toggle" />,
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole("switch"));

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("does not call onChange when disabled", async () => {
    const onChange = vi.fn();
    render(
      <PromptToggle
        enabled={false}
        onChange={onChange}
        disabled
        ariaLabel="toggle"
      />,
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole("switch"));

    expect(onChange).not.toHaveBeenCalled();
  });

  it("sets aria-label", () => {
    render(
      <PromptToggle
        enabled={false}
        onChange={vi.fn()}
        ariaLabel="prompt toggle"
      />,
    );

    expect(screen.getByRole("switch")).toHaveAttribute(
      "aria-label",
      "prompt toggle",
    );
  });
});
