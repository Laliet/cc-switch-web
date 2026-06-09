import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, beforeEach } from "vitest";
import type { Prompt } from "@/lib/api";
import PromptListItem from "@/components/prompts/PromptListItem";

const tMock = vi.fn((key: string) => key);

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

const createPrompt = (overrides: Partial<Prompt> = {}): Prompt => ({
  id: overrides.id ?? "prompt-1",
  name: overrides.name ?? "Sample Prompt",
  content: overrides.content ?? "Prompt content",
  description: overrides.description ?? "Sample description",
  enabled: overrides.enabled ?? false,
  createdAt: overrides.createdAt,
  updatedAt: overrides.updatedAt,
});

const renderItem = (promptOverrides: Partial<Prompt> = {}) => {
  const prompt = createPrompt(promptOverrides);
  const onToggle = vi.fn();
  const onEdit = vi.fn();
  const onDelete = vi.fn();

  render(
    <PromptListItem
      id={prompt.id}
      prompt={prompt}
      onToggle={onToggle}
      onEdit={onEdit}
      onDelete={onDelete}
    />,
  );

  return { prompt, onToggle, onEdit, onDelete };
};

beforeEach(() => {
  tMock.mockClear();
});

describe("PromptListItem", () => {
  it("renders prompt name and description", () => {
    const prompt = createPrompt({
      name: "Project Brief",
      description: "Keep it concise",
    });

    render(
      <PromptListItem
        id={prompt.id}
        prompt={prompt}
        onToggle={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText("Project Brief")).toBeInTheDocument();
    expect(screen.getByText("Keep it concise")).toBeInTheDocument();
  });

  it("enabled toggle calls onToggle", async () => {
    const user = userEvent.setup();
    const { onToggle, prompt } = renderItem({ enabled: false });

    await user.click(screen.getByRole("switch"));

    expect(onToggle).toHaveBeenCalledTimes(1);
    expect(onToggle).toHaveBeenCalledWith(prompt.id, true);
  });

  it("edit button calls onEdit", async () => {
    const user = userEvent.setup();
    const { onEdit, prompt } = renderItem();

    await user.click(screen.getByRole("button", { name: "prompts.edit" }));

    expect(onEdit).toHaveBeenCalledTimes(1);
    expect(onEdit).toHaveBeenCalledWith(prompt.id);
  });

  it("delete button calls onDelete", async () => {
    const user = userEvent.setup();
    const { onDelete, prompt } = renderItem();

    await user.click(screen.getByRole("button", { name: "common.delete" }));

    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(onDelete).toHaveBeenCalledWith(prompt.id);
  });

  it("delete button disabled when enabled", async () => {
    const user = userEvent.setup();
    const { onDelete } = renderItem({ enabled: true });

    const deleteButton = screen.getByRole("button", {
      name: "prompts.deleteEnabledHint",
    });

    expect(deleteButton).toBeDisabled();

    await user.click(deleteButton);
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("sets aria-labels for toggle, edit, and delete", () => {
    const { prompt } = renderItem({ name: "Focus Mode", enabled: false });

    expect(screen.getByRole("switch")).toHaveAttribute(
      "aria-label",
      `prompts.enable: ${prompt.name}`,
    );
    expect(
      screen.getByRole("button", { name: "prompts.edit" }),
    ).toHaveAttribute("aria-label", "prompts.edit");
    expect(
      screen.getByRole("button", { name: "common.delete" }),
    ).toHaveAttribute("aria-label", "common.delete");
  });
});
