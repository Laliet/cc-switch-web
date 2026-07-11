import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProviderForm } from "@/components/providers/forms/ProviderForm";

const authApiMock = vi.hoisted(() => ({
  listAccounts: vi.fn(),
}));

const modelFetchMock = vi.hoisted(() => ({
  fetchCodexOauthModels: vi.fn(),
  fetchGithubCopilotModels: vi.fn(),
  showFetchModelsError: vi.fn(),
}));

let omoDraftMock = {
  omoAgents: {} as Record<string, Record<string, unknown>>,
  omoCategories: {} as Record<string, Record<string, unknown>>,
  omoOtherFieldsStr: "",
  mergedOmoJsonPreview: "{}",
};

const tMock = vi.fn((key: string, options?: Record<string, unknown>) => {
  if (options?.defaultValue) {
    return String(options.defaultValue);
  }
  return key;
});

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

vi.mock("@/lib/api", () => ({
  authApi: authApiMock,
}));

vi.mock("@/lib/api/model-fetch", () => ({
  fetchCodexOauthModels: modelFetchMock.fetchCodexOauthModels,
  fetchGithubCopilotModels: modelFetchMock.fetchGithubCopilotModels,
  showFetchModelsError: modelFetchMock.showFetchModelsError,
}));

// Mock all child components to simplify testing
vi.mock("@/components/providers/forms/ProviderPresetSelector", () => ({
  ProviderPresetSelector: ({ onPresetChange, selectedPresetId }: any) => (
    <div data-testid="preset-selector">
      <button type="button" onClick={() => onPresetChange("custom")}>
        Select Custom
      </button>
      <button type="button" onClick={() => onPresetChange("claude-0")}>
        Select Preset
      </button>
      <button type="button" onClick={() => onPresetChange("codex-0")}>
        Select Codex OAuth
      </button>
      <button type="button" onClick={() => onPresetChange("codex-1")}>
        Select Codex API Key
      </button>
      <span data-testid="selected-preset">{selectedPresetId}</span>
    </div>
  ),
}));

vi.mock("@/components/providers/forms/BasicFormFields", () => ({
  BasicFormFields: ({ form }: any) => (
    <div data-testid="basic-fields">
      <input
        data-testid="name-input"
        value={form.watch("name")}
        onChange={(e) => form.setValue("name", e.target.value)}
        placeholder="Name"
      />
      <input
        data-testid="website-input"
        value={form.watch("websiteUrl")}
        onChange={(e) => form.setValue("websiteUrl", e.target.value)}
        placeholder="Website"
      />
    </div>
  ),
}));

vi.mock("@/components/providers/forms/ClaudeFormFields", () => ({
  ClaudeFormFields: ({ canFetchModels, fetchedModels, onFetchModels }: any) => (
    <div data-testid="claude-fields">
      <button
        disabled={!canFetchModels}
        onClick={() => onFetchModels?.()}
        type="button"
      >
        Fetch Claude Models
      </button>
      <span data-testid="claude-model-count">{fetchedModels?.length ?? 0}</span>
    </div>
  ),
}));

vi.mock("@/components/providers/forms/CodexFormFields", () => ({
  CodexFormFields: ({ canFetchModels, fetchedModels, onFetchModels }: any) => (
    <div data-testid="codex-fields">
      <button
        disabled={!canFetchModels}
        onClick={() => onFetchModels?.()}
        type="button"
      >
        Fetch Codex Models
      </button>
      <span data-testid="codex-model-count">{fetchedModels?.length ?? 0}</span>
    </div>
  ),
}));

vi.mock("@/components/providers/forms/GeminiFormFields", () => ({
  GeminiFormFields: () => <div data-testid="gemini-fields">Gemini Fields</div>,
}));

vi.mock("@/components/providers/forms/OpenCodeFormFields", () => ({
  OpenCodeFormFields: () => (
    <div data-testid="opencode-fields">OpenCode Fields</div>
  ),
}));

vi.mock("@/components/providers/forms/OmoFormFields", () => ({
  OmoFormFields: ({ isSlim }: any) => (
    <div data-testid="omo-fields" data-slim={String(Boolean(isSlim))}>
      OMO Fields
    </div>
  ),
}));

vi.mock("@/components/providers/forms/CommonConfigEditor", () => ({
  CommonConfigEditor: ({ value, onChange, showCommonConfigControls }: any) => (
    <div
      data-testid="common-config-editor"
      data-show-common-config-controls={String(
        showCommonConfigControls ?? true,
      )}
    >
      <textarea
        data-testid="config-textarea"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  ),
}));

vi.mock("@/components/providers/forms/CodexConfigEditor", () => ({
  default: () => <div data-testid="codex-config-editor">Codex Config</div>,
}));

vi.mock("@/components/providers/forms/GeminiConfigEditor", () => ({
  default: () => <div data-testid="gemini-config-editor">Gemini Config</div>,
}));

// Mock hooks
vi.mock("@/components/providers/forms/hooks", () => ({
  useProviderCategory: () => ({ category: "third_party" }),
  useApiKeyState: () => ({
    apiKey: "",
    handleApiKeyChange: vi.fn(),
    showApiKey: () => true,
  }),
  useBaseUrlState: () => ({
    baseUrl: "",
    handleClaudeBaseUrlChange: vi.fn(),
  }),
  useModelState: () => ({
    claudeModel: "",
    defaultHaikuModel: "",
    defaultSonnetModel: "",
    defaultOpusModel: "",
    handleModelChange: vi.fn(),
  }),
  useCodexConfigState: () => ({
    codexAuth: "{}",
    codexConfig: "",
    codexApiKey: "",
    codexBaseUrl: "",
    codexModelName: "",
    codexAuthError: null,
    setCodexAuth: vi.fn(),
    handleCodexApiKeyChange: vi.fn(),
    handleCodexBaseUrlChange: vi.fn(),
    handleCodexModelNameChange: vi.fn(),
    handleCodexConfigChange: vi.fn(),
    resetCodexConfig: vi.fn(),
  }),
  useCodexTomlValidation: () => ({
    configError: null,
    debouncedValidate: vi.fn(),
  }),
  useTemplateValues: () => ({
    templateValues: {},
    templateValueEntries: [],
    selectedPreset: null,
    handleTemplateValueChange: vi.fn(),
    validateTemplateValues: () => ({ isValid: true }),
  }),
  useCommonConfigSnippet: () => ({
    useCommonConfig: false,
    commonConfigSnippet: "",
    commonConfigError: null,
    handleCommonConfigToggle: vi.fn(),
    handleCommonConfigSnippetChange: vi.fn(),
  }),
  useCodexCommonConfig: () => ({
    useCommonConfig: false,
    commonConfigSnippet: "",
    commonConfigError: null,
    handleCommonConfigToggle: vi.fn(),
    handleCommonConfigSnippetChange: vi.fn(),
  }),
  useApiKeyLink: () => ({
    shouldShowApiKeyLink: false,
    websiteUrl: "",
    isPartner: false,
    partnerPromotionKey: undefined,
  }),
  useSpeedTestEndpoints: () => [],
  useGeminiConfigState: () => ({
    geminiEnv: "",
    geminiConfig: "",
    geminiApiKey: "",
    geminiBaseUrl: "",
    geminiModel: "",
    envError: null,
    configError: null,
    handleGeminiApiKeyChange: vi.fn(),
    handleGeminiBaseUrlChange: vi.fn(),
    handleGeminiEnvChange: vi.fn(),
    handleGeminiConfigChange: vi.fn(),
    resetGeminiConfig: vi.fn(),
    envStringToObj: () => ({}),
    envObjToString: () => "",
  }),
  useGeminiCommonConfig: () => ({
    useCommonConfig: false,
    commonConfigSnippet: "",
    commonConfigError: null,
    handleCommonConfigToggle: vi.fn(),
    handleCommonConfigSnippetChange: vi.fn(),
  }),
}));

vi.mock("@/components/providers/forms/hooks/useOmoDraftState", () => ({
  useOmoDraftState: () => ({
    ...omoDraftMock,
    setOmoAgents: vi.fn(),
    setOmoCategories: vi.fn(),
    setOmoOtherFieldsStr: vi.fn(),
    resetOmoDraftState: vi.fn(),
  }),
}));

vi.mock("@/components/providers/forms/hooks/useOmoModelSource", () => ({
  useOmoModelSource: () => ({
    omoModelOptions: [],
    omoModelVariantsMap: {},
    omoPresetMetaMap: {},
    existingOpencodeKeys: [],
  }),
}));

// Mock presets
vi.mock("@/config/claudeProviderPresets", () => ({
  providerPresets: [
    {
      name: "Test Preset",
      category: "third_party",
      websiteUrl: "https://test.com",
      providerType: "github_copilot",
      settingsConfig: { env: {} },
    },
  ],
}));

vi.mock("@/config/codexProviderPresets", () => ({
  codexProviderPresets: [
    {
      name: "Codex OAuth",
      category: "official",
      websiteUrl: "https://codex.com",
      providerType: "codex_oauth",
      auth: {},
      config: "",
    },
    {
      name: "Codex Preset",
      category: "third_party",
      websiteUrl: "https://codex.com",
      auth: {},
      config: "",
    },
  ],
}));

vi.mock("@/config/geminiProviderPresets", () => ({
  geminiProviderPresets: [
    {
      name: "Gemini Preset",
      category: "official",
      websiteUrl: "https://gemini.com",
      settingsConfig: { env: {} },
    },
  ],
}));

vi.mock("@/config/opencodeProviderPresets", () => ({
  OPENCODE_PRESET_MODEL_VARIANTS: {},
  opencodeNpmPackages: [
    { value: "@ai-sdk/openai-compatible", label: "OpenAI Compatible" },
  ],
  opencodeProviderPresets: [
    {
      name: "OpenCode Preset",
      category: "third_party",
      websiteUrl: "https://opencode.com",
      settingsConfig: {
        npm: "@ai-sdk/openai-compatible",
        options: { baseURL: "https://api.example.com", apiKey: "" },
        models: {},
      },
    },
  ],
}));

vi.mock("@/config/codexTemplates", () => ({
  getCodexCustomTemplate: () => ({ auth: {}, config: "" }),
}));

vi.mock("@/utils/providerConfigUtils", () => ({
  applyTemplateValues: (config: any) => config,
}));

vi.mock("@/utils/providerMetaUtils", () => ({
  mergeProviderMeta: (existing: any, custom: any) => ({
    ...existing,
    ...custom,
  }),
}));

const defaultProps = {
  appId: "claude" as const,
  submitLabel: "Save",
  onSubmit: vi.fn(),
  onCancel: vi.fn(),
};

beforeEach(() => {
  tMock.mockClear();
  defaultProps.onSubmit.mockClear();
  defaultProps.onCancel.mockClear();
  authApiMock.listAccounts.mockReset();
  authApiMock.listAccounts.mockResolvedValue([
    {
      id: "github-1",
      provider: "github_copilot",
      label: "GitHub One",
      isDefault: false,
      createdAt: "2026-06-08T00:00:00Z",
      updatedAt: "2026-06-08T00:00:00Z",
    },
  ]);
  modelFetchMock.fetchCodexOauthModels.mockReset();
  modelFetchMock.fetchGithubCopilotModels.mockReset();
  modelFetchMock.showFetchModelsError.mockReset();
  modelFetchMock.fetchCodexOauthModels.mockResolvedValue([]);
  modelFetchMock.fetchGithubCopilotModels.mockResolvedValue([]);
  omoDraftMock = {
    omoAgents: {},
    omoCategories: {},
    omoOtherFieldsStr: "",
    mergedOmoJsonPreview: "{}",
  };
});

describe("ProviderForm", () => {
  it("renders form with basic fields", () => {
    render(<ProviderForm {...defaultProps} />);

    expect(screen.getByTestId("basic-fields")).toBeInTheDocument();
  });

  it("renders preset selector in create mode", () => {
    render(<ProviderForm {...defaultProps} />);

    expect(screen.getByTestId("preset-selector")).toBeInTheDocument();
  });

  it("renders preset selector and OpenCode fields for opencode", () => {
    render(<ProviderForm {...defaultProps} appId="opencode" />);

    expect(screen.getByTestId("preset-selector")).toBeInTheDocument();
    expect(screen.getByTestId("opencode-fields")).toBeInTheDocument();
  });

  it("renders OMO form without preset selector", () => {
    render(<ProviderForm {...defaultProps} appId="omo" />);

    expect(screen.queryByTestId("preset-selector")).not.toBeInTheDocument();
    expect(screen.getByTestId("omo-fields")).toHaveAttribute(
      "data-slim",
      "false",
    );
  });

  it("renders OMO Slim form", () => {
    render(<ProviderForm {...defaultProps} appId="omo-slim" />);

    expect(screen.getByTestId("omo-fields")).toHaveAttribute(
      "data-slim",
      "true",
    );
  });

  it("hides preset selector in edit mode", () => {
    render(
      <ProviderForm
        {...defaultProps}
        initialData={{ name: "Existing Provider" }}
      />,
    );

    expect(screen.queryByTestId("preset-selector")).not.toBeInTheDocument();
  });

  it("renders Claude fields for claude appId", () => {
    render(<ProviderForm {...defaultProps} appId="claude" />);

    expect(screen.getByTestId("claude-fields")).toBeInTheDocument();
    expect(screen.queryByTestId("codex-fields")).not.toBeInTheDocument();
    expect(screen.queryByTestId("gemini-fields")).not.toBeInTheDocument();
  });

  it("renders Codex fields for codex appId", () => {
    render(<ProviderForm {...defaultProps} appId="codex" />);

    expect(screen.getByTestId("codex-fields")).toBeInTheDocument();
    expect(screen.queryByTestId("claude-fields")).not.toBeInTheDocument();
  });

  it("renders Gemini fields for gemini appId", () => {
    render(<ProviderForm {...defaultProps} appId="gemini" />);

    expect(screen.getByTestId("gemini-fields")).toBeInTheDocument();
    expect(screen.queryByTestId("claude-fields")).not.toBeInTheDocument();
  });

  it("renders config editor based on appId", () => {
    const { rerender } = render(
      <ProviderForm {...defaultProps} appId="claude" />,
    );
    expect(screen.getByTestId("common-config-editor")).toBeInTheDocument();
    expect(screen.getByTestId("common-config-editor")).toHaveAttribute(
      "data-show-common-config-controls",
      "true",
    );

    rerender(<ProviderForm {...defaultProps} appId="codex" />);
    expect(screen.getByTestId("codex-config-editor")).toBeInTheDocument();

    rerender(<ProviderForm {...defaultProps} appId="gemini" />);
    expect(screen.getByTestId("gemini-config-editor")).toBeInTheDocument();
  });

  it("shows buttons by default", () => {
    render(<ProviderForm {...defaultProps} />);

    expect(
      screen.getByRole("button", { name: "common.cancel" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeInTheDocument();
  });

  it("hides buttons when showButtons is false", () => {
    render(<ProviderForm {...defaultProps} showButtons={false} />);

    expect(
      screen.queryByRole("button", { name: "common.cancel" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Save" }),
    ).not.toBeInTheDocument();
  });

  it("calls onCancel when cancel button clicked", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();

    render(<ProviderForm {...defaultProps} onCancel={onCancel} />);

    await user.click(screen.getByRole("button", { name: "common.cancel" }));

    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("changes preset selection", async () => {
    const user = userEvent.setup();

    render(<ProviderForm {...defaultProps} />);

    expect(screen.getByTestId("selected-preset")).toHaveTextContent("custom");

    await user.click(screen.getByText("Select Preset"));

    await waitFor(() => {
      expect(screen.getByTestId("selected-preset")).toHaveTextContent(
        "claude-0",
      );
    });
  });

  it("uses initial data in edit mode", () => {
    render(
      <ProviderForm
        {...defaultProps}
        initialData={{
          name: "My Provider",
          websiteUrl: "https://example.com",
          notes: "Some notes",
          settingsConfig: { env: { API_KEY: "test" } },
        }}
      />,
    );

    expect(screen.getByTestId("name-input")).toHaveValue("My Provider");
    expect(screen.getByTestId("website-input")).toHaveValue(
      "https://example.com",
    );
  });

  it("submits form with correct values", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(<ProviderForm {...defaultProps} onSubmit={onSubmit} />);

    const nameInput = screen.getByTestId("name-input");
    await user.clear(nameInput);
    await user.type(nameInput, "New Provider");

    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).not.toHaveBeenCalled();
    await user.click(await screen.findByRole("button", { name: "仍要保存" }));
    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });

    const submittedData = onSubmit.mock.calls[0][0];
    expect(submittedData.name).toBe("New Provider");
  });

  it("preserves managed auth binding when editing an OAuth provider", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(
      <ProviderForm
        {...defaultProps}
        onSubmit={onSubmit}
        initialData={{
          name: "Copilot",
          settingsConfig: {
            env: {
              ANTHROPIC_BASE_URL: "https://api.githubcopilot.com",
              ANTHROPIC_AUTH_TOKEN: "placeholder",
            },
          },
          meta: {
            providerType: "github_copilot",
            authBinding: {
              mode: "managed",
              providerType: "github_copilot",
              accountId: "github-1",
              useDefault: false,
            },
          },
        }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    expect(submittedData.meta).toMatchObject({
      providerType: "github_copilot",
      githubAccountId: "github-1",
      authBinding: {
        mode: "managed",
        providerType: "github_copilot",
        accountId: "github-1",
        useDefault: false,
      },
    });
  });

  it("clears stale manual Claude API keys when saving managed Copilot auth", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(
      <ProviderForm
        {...defaultProps}
        onSubmit={onSubmit}
        initialData={{
          name: "Copilot",
          settingsConfig: {
            env: {
              ANTHROPIC_BASE_URL: "https://api.githubcopilot.com",
              ANTHROPIC_AUTH_TOKEN: "old-manual-token",
              OPENAI_API_KEY: "old-openai-key",
            },
          },
          meta: {
            providerType: "github_copilot",
            authBinding: {
              mode: "managed",
              providerType: "github_copilot",
              accountId: "github-1",
              useDefault: false,
            },
          },
        }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    const settingsConfig = JSON.parse(submittedData.settingsConfig);
    expect(settingsConfig.env.ANTHROPIC_BASE_URL).toBe(
      "https://api.githubcopilot.com",
    );
    expect(settingsConfig.env.ANTHROPIC_AUTH_TOKEN).toBe("");
    expect(settingsConfig.env.OPENAI_API_KEY).toBe("");
  });

  it("submits selected managed account binding for OAuth presets", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(<ProviderForm {...defaultProps} onSubmit={onSubmit} />);

    await user.click(screen.getByText("Select Preset"));
    await user.click(screen.getByRole("combobox", { name: "账号" }));
    await user.click(await screen.findByRole("option", { name: "GitHub One" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    expect(submittedData.meta).toMatchObject({
      providerType: "github_copilot",
      githubAccountId: "github-1",
      authBinding: {
        mode: "managed",
        providerType: "github_copilot",
        accountId: "github-1",
        useDefault: false,
      },
    });
  });

  it("preserves manual API key mode for OAuth-compatible providers", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(
      <ProviderForm
        {...defaultProps}
        onSubmit={onSubmit}
        initialData={{
          name: "Manual Copilot",
          settingsConfig: {
            env: {
              ANTHROPIC_BASE_URL: "https://api.githubcopilot.com",
              ANTHROPIC_AUTH_TOKEN: "manual-token",
            },
          },
          meta: {
            providerType: " GitHub-Copilot ",
            githubAccountId: "github-1",
            authBinding: {
              mode: " API-KEY ",
              providerType: " GitHub-Copilot ",
            },
          },
        }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    expect(submittedData.meta).toMatchObject({
      providerType: "github_copilot",
      authBinding: {
        mode: "api_key",
        providerType: "github_copilot",
      },
    });
    expect(submittedData.meta.githubAccountId).toBeUndefined();
  });

  it("treats legacy OAuth provider metadata with a real token as manual API key mode", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(
      <ProviderForm
        {...defaultProps}
        onSubmit={onSubmit}
        initialData={{
          name: "Legacy Manual Copilot",
          settingsConfig: {
            env: {
              ANTHROPIC_BASE_URL: "https://api.githubcopilot.com",
              ANTHROPIC_AUTH_TOKEN: "manual-token",
            },
          },
          meta: {
            providerType: "github_copilot",
          },
        }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    expect(submittedData.meta).toMatchObject({
      providerType: "github_copilot",
      authBinding: {
        mode: "api_key",
        providerType: "github_copilot",
      },
    });
    expect(submittedData.meta.githubAccountId).toBeUndefined();
  });

  it("submits Codex OAuth proxy cache and fast mode options", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(
      <ProviderForm
        {...defaultProps}
        appId="codex"
        onSubmit={onSubmit}
        initialData={{
          name: "Codex OAuth",
          settingsConfig: {
            auth: { OPENAI_API_KEY: "" },
            config: 'model_provider = "codex_oauth"',
          },
          meta: {
            providerType: "codex_oauth",
            authBinding: {
              mode: "managed",
              providerType: "codex_oauth",
              useDefault: true,
            },
          },
        }}
      />,
    );

    await user.type(screen.getByLabelText("Prompt cache key"), "cache-1");
    await user.click(screen.getByRole("switch", { name: "FAST mode" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    expect(submittedData.meta).toMatchObject({
      providerType: "codex_oauth",
      promptCacheKey: "cache-1",
      codexFastMode: true,
      authBinding: {
        mode: "managed",
        providerType: "codex_oauth",
        useDefault: true,
      },
    });
  });

  it("clears stale manual Codex API key when saving managed Codex OAuth", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(
      <ProviderForm
        {...defaultProps}
        appId="codex"
        onSubmit={onSubmit}
        initialData={{
          name: "Codex OAuth",
          settingsConfig: {
            auth: { OPENAI_API_KEY: "old-manual-key" },
            config: 'model_provider = "codex_oauth"',
          },
          meta: {
            providerType: "codex_oauth",
            authBinding: {
              mode: "managed",
              providerType: "codex_oauth",
              useDefault: true,
            },
          },
        }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    const settingsConfig = JSON.parse(submittedData.settingsConfig);
    expect(settingsConfig.auth.OPENAI_API_KEY).toBe("");
    expect(submittedData.meta).toMatchObject({
      providerType: "codex_oauth",
      authBinding: {
        mode: "managed",
        providerType: "codex_oauth",
        useDefault: true,
      },
    });
  });

  it("resets a managed account binding when the account belongs to another provider type", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(
      <ProviderForm
        {...defaultProps}
        appId="codex"
        onSubmit={onSubmit}
        initialData={{
          name: "Codex OAuth",
          settingsConfig: {
            auth: { OPENAI_API_KEY: "" },
            config: 'model_provider = "codex_oauth"',
          },
          meta: {
            providerType: "codex_oauth",
            authBinding: {
              mode: "managed",
              providerType: "codex_oauth",
              accountId: "github-1",
              useDefault: false,
            },
          },
        }}
      />,
    );

    await waitFor(() => {
      expect(authApiMock.listAccounts).toHaveBeenCalled();
    });
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    expect(submittedData.meta).toMatchObject({
      providerType: "codex_oauth",
      authBinding: {
        mode: "managed",
        providerType: "codex_oauth",
        useDefault: true,
      },
    });
    expect(submittedData.meta?.authBinding?.accountId).toBeUndefined();
  });

  it("resets a managed account binding when the selected account is logged out", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    authApiMock.listAccounts.mockResolvedValueOnce([
      {
        id: "codex-logged-out",
        provider: "codex_oauth",
        label: "Logged Out",
        isDefault: false,
        status: "logged_out",
        createdAt: "2026-06-08T00:00:00Z",
        updatedAt: "2026-06-08T00:00:00Z",
      },
    ]);

    render(
      <ProviderForm
        {...defaultProps}
        appId="codex"
        onSubmit={onSubmit}
        initialData={{
          name: "Codex OAuth",
          settingsConfig: {
            auth: { OPENAI_API_KEY: "" },
            config: 'model_provider = "codex_oauth"',
          },
          meta: {
            providerType: "codex_oauth",
            authBinding: {
              mode: "managed",
              providerType: "codex_oauth",
              accountId: "codex-logged-out",
              useDefault: false,
            },
          },
        }}
      />,
    );

    await waitFor(() => {
      expect(authApiMock.listAccounts).toHaveBeenCalled();
    });
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    expect(submittedData.meta).toMatchObject({
      providerType: "codex_oauth",
      authBinding: {
        mode: "managed",
        providerType: "codex_oauth",
        useDefault: true,
      },
    });
    expect(submittedData.meta?.authBinding?.accountId).toBeUndefined();
  });

  it("fetches Codex OAuth live models through the managed account API", async () => {
    const user = userEvent.setup();
    modelFetchMock.fetchCodexOauthModels.mockResolvedValueOnce([
      { id: "gpt-5.1-codex", ownedBy: null },
    ]);

    render(<ProviderForm {...defaultProps} appId="codex" />);

    await user.click(screen.getByText("Select Codex OAuth"));
    await user.click(
      screen.getByRole("button", { name: "Fetch Codex Models" }),
    );

    await waitFor(() => {
      expect(modelFetchMock.fetchCodexOauthModels).toHaveBeenCalledWith(null);
    });
    expect(await screen.findByTestId("codex-model-count")).toHaveTextContent(
      "1",
    );
  });

  it("fetches Claude Copilot live models with the selected managed account", async () => {
    const user = userEvent.setup();
    modelFetchMock.fetchGithubCopilotModels.mockResolvedValueOnce([
      { id: "gpt-4.1", ownedBy: null },
    ]);

    render(<ProviderForm {...defaultProps} />);

    await user.click(screen.getByText("Select Preset"));
    await user.click(screen.getByRole("combobox", { name: "账号" }));
    await user.click(await screen.findByRole("option", { name: "GitHub One" }));
    await user.click(
      screen.getByRole("button", { name: "Fetch Claude Models" }),
    );

    await waitFor(() => {
      expect(modelFetchMock.fetchGithubCopilotModels).toHaveBeenCalledWith(
        "github-1",
      );
    });
    expect(await screen.findByTestId("claude-model-count")).toHaveTextContent(
      "1",
    );
  });

  it("clears Codex OAuth proxy options after switching to a non-OAuth Codex preset", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(
      <ProviderForm {...defaultProps} appId="codex" onSubmit={onSubmit} />,
    );

    await user.click(screen.getByText("Select Codex OAuth"));
    await user.type(screen.getByLabelText("Prompt cache key"), "old-cache");
    await user.click(screen.getByRole("switch", { name: "FAST mode" }));
    await user.click(screen.getByText("Select Codex API Key"));
    await user.click(screen.getByRole("button", { name: "Save" }));
    await user.click(await screen.findByRole("button", { name: "仍要保存" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    expect(submittedData.meta?.promptCacheKey).toBeUndefined();
    expect(submittedData.meta?.codexFastMode).toBeUndefined();
    expect(submittedData.meta?.authBinding).toBeUndefined();
  });

  it("clears stale managed auth metadata after switching from an OAuth preset to a non-OAuth preset", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();

    render(
      <ProviderForm {...defaultProps} appId="codex" onSubmit={onSubmit} />,
    );

    await user.click(screen.getByText("Select Codex OAuth"));
    await user.type(screen.getByLabelText("Prompt cache key"), "old-cache");
    await user.click(screen.getByRole("switch", { name: "FAST mode" }));
    await user.click(screen.getByText("Select Codex API Key"));
    await user.click(screen.getByRole("button", { name: "Save" }));
    await user.click(await screen.findByRole("button", { name: "仍要保存" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const submittedData = onSubmit.mock.calls[0][0];
    expect(submittedData.meta?.providerType).toBeUndefined();
    expect(submittedData.meta?.promptCacheKey).toBeUndefined();
    expect(submittedData.meta?.codexFastMode).toBeUndefined();
    expect(submittedData.meta?.authBinding).toBeUndefined();
  });

  it("submits OMO other fields as top-level config fields", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    omoDraftMock = {
      omoAgents: { oracle: { model: "anthropic/claude" } },
      omoCategories: { analyze: { model: "openai/gpt" } },
      omoOtherFieldsStr: JSON.stringify({
        $schema: "schema-url",
        disabled_agents: ["oracle"],
      }),
      mergedOmoJsonPreview: "{}",
    };

    render(<ProviderForm {...defaultProps} appId="omo" onSubmit={onSubmit} />);

    await user.type(screen.getByTestId("name-input"), "OMO Provider");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalled();
    });
    const settingsConfig = JSON.parse(onSubmit.mock.calls[0][0].settingsConfig);
    expect(settingsConfig).toEqual({
      agents: { oracle: { model: "anthropic/claude" } },
      categories: { analyze: { model: "openai/gpt" } },
      $schema: "schema-url",
      disabled_agents: ["oracle"],
    });
    expect(settingsConfig.otherFields).toBeUndefined();
  });

  it("has form id for external submission", () => {
    render(<ProviderForm {...defaultProps} />);

    const form = document.getElementById("provider-form");
    expect(form).toBeInTheDocument();
    expect(form?.tagName).toBe("FORM");
  });
});
