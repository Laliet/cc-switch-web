import { render, screen, waitFor } from "@testing-library/react";
import type { LabelHTMLAttributes } from "react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  OpenCodeFormFields,
  mergeFetchedModelsIntoConfig,
} from "@/components/providers/forms/OpenCodeFormFields";

const fetchModelsForConfigMock = vi.hoisted(() => vi.fn());
const toastMock = vi.hoisted(() => ({
  success: vi.fn(),
  info: vi.fn(),
  error: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      String(options?.defaultValue ?? key),
  }),
}));

vi.mock("sonner", () => ({
  toast: toastMock,
}));

vi.mock("@/lib/api/model-fetch", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api/model-fetch")>(
    "@/lib/api/model-fetch",
  );
  return {
    ...actual,
    fetchModelsForConfig: (...args: unknown[]) =>
      fetchModelsForConfigMock(...args),
  };
});

vi.mock("@/components/ui/form", () => ({
  FormLabel: ({
    children,
    ...props
  }: LabelHTMLAttributes<HTMLLabelElement>) => (
    <label {...props}>{children}</label>
  ),
}));

const defaultProps = {
  apiKey: "sk-test",
  onApiKeyChange: vi.fn(),
  shouldShowApiKeyLink: false,
  websiteUrl: "",
  baseUrl: "https://generativelanguage.googleapis.com",
  onBaseUrlChange: vi.fn(),
  isFullUrl: false,
  onIsFullUrlChange: vi.fn(),
  modelsUrl: "",
  onModelsUrlChange: vi.fn(),
  models: {},
  onModelsChange: vi.fn(),
  extraOptions: {},
  onExtraOptionsChange: vi.fn(),
};

describe("OpenCodeFormFields", () => {
  beforeEach(() => {
    fetchModelsForConfigMock.mockReset();
    toastMock.success.mockReset();
    toastMock.info.mockReset();
    toastMock.error.mockReset();
  });

  it("allows model fetch for provider npm packages supported by backend fetch protocols", async () => {
    const user = userEvent.setup();
    const onModelsChange = vi.fn();
    fetchModelsForConfigMock.mockResolvedValueOnce([
      { id: "gemini-3-pro-preview", ownedBy: "google" },
    ]);

    render(
      <OpenCodeFormFields
        {...defaultProps}
        npm="@ai-sdk/google"
        onNpmChange={vi.fn()}
        onModelsChange={onModelsChange}
      />,
    );

    const fetchButton = screen.getByRole("button", {
      name: "providerForm.fetchModels",
    });
    expect(fetchButton).toBeEnabled();

    await user.click(fetchButton);

    await waitFor(() => {
      expect(fetchModelsForConfigMock).toHaveBeenCalledWith(
        "https://generativelanguage.googleapis.com",
        "sk-test",
        "@ai-sdk/google",
        false,
        "",
      );
    });
    await waitFor(() => {
      expect(onModelsChange).toHaveBeenCalledWith({
        "gemini-3-pro-preview": expect.objectContaining({
          name: "Gemini 3 Pro Preview",
          limit: {
            context: 1048576,
            output: 65536,
          },
          modalities: {
            input: ["text", "image", "pdf", "video", "audio"],
            output: ["text"],
          },
        }),
      });
    });
  });

  it("imports preset models for Bedrock without calling backend model fetch", async () => {
    const user = userEvent.setup();
    const onModelsChange = vi.fn();

    render(
      <OpenCodeFormFields
        {...defaultProps}
        npm="@ai-sdk/amazon-bedrock"
        onNpmChange={vi.fn()}
        onModelsChange={onModelsChange}
      />,
    );

    const fetchButton = screen.getByRole("button", {
      name: "providerForm.fetchModels",
    });
    expect(fetchButton).toBeEnabled();

    await user.click(fetchButton);

    expect(fetchModelsForConfigMock).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(onModelsChange).toHaveBeenCalledWith(
        expect.objectContaining({
          "global.anthropic.claude-opus-4-7": {
            name: "Claude Opus 4.7",
            limit: {
              context: 1000000,
              output: 128000,
            },
            modalities: {
              input: ["text", "image", "pdf"],
              output: ["text"],
            },
          },
          "us.amazon.nova-pro-v1:0": {
            name: "Amazon Nova Pro",
            limit: {
              context: 300000,
              output: 5000,
            },
            modalities: {
              input: ["text", "image"],
              output: ["text"],
            },
          },
        }),
      );
    });
  });

  it("passes full URL and model URL override options to model fetch", async () => {
    const user = userEvent.setup();
    fetchModelsForConfigMock.mockResolvedValueOnce([]);

    render(
      <OpenCodeFormFields
        {...defaultProps}
        npm="@ai-sdk/openai-compatible"
        onNpmChange={vi.fn()}
        isFullUrl
        modelsUrl="https://api.example.com/v1/models"
      />,
    );

    await user.click(
      screen.getByRole("button", { name: "providerForm.fetchModels" }),
    );

    await waitFor(() => {
      expect(fetchModelsForConfigMock).toHaveBeenCalledWith(
        "https://generativelanguage.googleapis.com",
        "sk-test",
        "@ai-sdk/openai-compatible",
        true,
        "https://api.example.com/v1/models",
      );
    });
  });
});

describe("mergeFetchedModelsIntoConfig", () => {
  it("adds fetched models without overwriting existing model configuration", () => {
    const current = {
      "gpt-5.4": {
        name: "GPT 5.4",
        options: { reasoningEffort: "high" },
      },
    };

    expect(
      mergeFetchedModelsIntoConfig(
        current,
        [
          { id: "gpt-5.4", ownedBy: "openai" },
          { id: "gpt-5.4-mini", ownedBy: "openai" },
        ],
        "@ai-sdk/openai",
      ),
    ).toEqual({
      "gpt-5.4": {
        name: "GPT 5.4",
        options: { reasoningEffort: "high" },
      },
      "gpt-5.4-mini": { name: "gpt-5.4-mini" },
    });
  });
});
