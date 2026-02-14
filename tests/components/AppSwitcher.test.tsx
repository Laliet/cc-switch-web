import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { AppId } from "@/lib/api";
import { AppSwitcher } from "@/components/AppSwitcher";

const renderAppSwitcher = (activeApp: AppId, onSwitch = vi.fn()) => {
  const renderResult = render(
    <AppSwitcher activeApp={activeApp} onSwitch={onSwitch} />,
  );

  return { onSwitch, ...renderResult };
};

const getButton = (name: string | RegExp) =>
  screen.getByRole("button", { name });

const getButtonIcon = (button: HTMLElement) => {
  const icon = button.querySelector("svg");
  if (!icon) {
    throw new Error("Expected button to contain an svg icon.");
  }
  return icon;
};

describe("AppSwitcher", () => {
  it("renders supported and upcoming app buttons", () => {
    renderAppSwitcher("claude");

    expect(getButton("apps.claude")).toBeInTheDocument();
    expect(getButton("apps.codex")).toBeInTheDocument();
    expect(getButton("apps.gemini")).toBeInTheDocument();
    expect(getButton(/apps\.opencode/i)).toBeDisabled();
    expect(getButton(/apps\.omo/i)).toBeDisabled();
    expect(screen.getAllByRole("button")).toHaveLength(5);
  });

  it("calls onSwitch when clicking different buttons", async () => {
    const user = userEvent.setup();
    const onSwitch = vi.fn();
    renderAppSwitcher("claude", onSwitch);

    await user.click(getButton("apps.codex"));
    await user.click(getButton("apps.gemini"));
    await user.click(getButton(/apps\.opencode/i));

    expect(onSwitch).toHaveBeenCalledTimes(2);
    expect(onSwitch).toHaveBeenNthCalledWith(1, "codex");
    expect(onSwitch).toHaveBeenNthCalledWith(2, "gemini");
  });

  it("does not call onSwitch when clicking active button", async () => {
    const user = userEvent.setup();
    const onSwitch = vi.fn();
    renderAppSwitcher("codex", onSwitch);

    await user.click(getButton("apps.codex"));

    expect(onSwitch).not.toHaveBeenCalled();
  });

  it("applies active styles based on activeApp", () => {
    const onSwitch = vi.fn();
    const { rerender } = render(
      <AppSwitcher activeApp="claude" onSwitch={onSwitch} />,
    );

    const claudeButton = getButton("apps.claude");
    const codexButton = getButton("apps.codex");
    const geminiButton = getButton("apps.gemini");

    expect(claudeButton).toHaveClass("bg-white");
    expect(claudeButton).toHaveClass("text-gray-900");
    expect(claudeButton).not.toHaveClass("text-gray-500");
    expect(codexButton).toHaveClass("text-gray-500");
    expect(codexButton).not.toHaveClass("bg-white");
    expect(geminiButton).toHaveClass("text-gray-500");

    const claudeIcon = getButtonIcon(claudeButton);
    const geminiIcon = getButtonIcon(geminiButton);

    expect(claudeIcon).toHaveClass("text-[#D97757]");
    expect(claudeIcon).not.toHaveClass("text-gray-500");
    expect(geminiIcon).toHaveClass("text-gray-500");

    rerender(<AppSwitcher activeApp="gemini" onSwitch={onSwitch} />);

    const claudeButtonAfter = getButton("apps.claude");
    const geminiButtonAfter = getButton("apps.gemini");

    expect(geminiButtonAfter).toHaveClass("bg-white");
    expect(geminiButtonAfter).toHaveClass("text-gray-900");
    expect(geminiButtonAfter).not.toHaveClass("text-gray-500");
    expect(claudeButtonAfter).toHaveClass("text-gray-500");
    expect(claudeButtonAfter).not.toHaveClass("bg-white");

    const claudeIconAfter = getButtonIcon(claudeButtonAfter);
    const geminiIconAfter = getButtonIcon(geminiButtonAfter);

    expect(claudeIconAfter).toHaveClass("text-gray-500");
    expect(geminiIconAfter).toHaveClass("text-[#4285F4]");
  });
});
