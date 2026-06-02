import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { AppId } from "@/lib/api";
import { AppSwitcher } from "@/components/AppSwitcher";
import { SWITCHER_APPS } from "@/config/apps";

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

    for (const app of SWITCHER_APPS) {
      expect(getButton(app.labelKey)).toBeInTheDocument();
    }
    expect(screen.getAllByRole("button")).toHaveLength(SWITCHER_APPS.length);
  });

  it("calls onSwitch when clicking different buttons", async () => {
    const user = userEvent.setup();
    const onSwitch = vi.fn();
    renderAppSwitcher("claude", onSwitch);

    const inactiveApps = SWITCHER_APPS.filter((app) => app.id !== "claude");

    for (const app of inactiveApps) {
      await user.click(getButton(app.labelKey));
    }

    expect(onSwitch).toHaveBeenCalledTimes(inactiveApps.length);
    inactiveApps.forEach((app, index) => {
      expect(onSwitch).toHaveBeenNthCalledWith(index + 1, app.id);
    });
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
