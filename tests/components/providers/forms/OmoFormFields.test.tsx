import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { OmoFormFields } from "@/components/providers/forms/OmoFormFields";
import { mergeCustomModelsIntoStore } from "@/components/providers/forms/OmoFormFields";

const toastMock = vi.hoisted(() => ({
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
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

vi.mock("@/lib/query/omo", () => ({
  useReadOmoLocalFile: () => ({
    isPending: false,
    mutateAsync: vi.fn(),
  }),
  useReadOmoSlimLocalFile: () => ({
    isPending: false,
    mutateAsync: vi.fn(),
  }),
}));

const modelOptions = [
  { value: "openai/gpt-5.4", label: "OpenAI / GPT 5.4" },
  { value: "google/gemini-3-pro", label: "Google / Gemini 3 Pro" },
];

class ResizeObserverMock {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}

const defaultProps = {
  modelOptions,
  modelVariantsMap: {},
  presetMetaMap: {},
  agents: {},
  onAgentsChange: vi.fn(),
  categories: {},
  onCategoriesChange: vi.fn(),
  otherFieldsStr: "",
  onOtherFieldsStrChange: vi.fn(),
};

describe("OmoFormFields", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("ResizeObserver", ResizeObserverMock);
    window.HTMLElement.prototype.scrollIntoView = vi.fn();
  });

  it("renders OMO Slim agents without the standard categories section", () => {
    render(<OmoFormFields {...defaultProps} isSlim />);

    expect(screen.getAllByText("Orchestrator").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Council").length).toBeGreaterThan(0);
    expect(screen.queryByText("Categories")).not.toBeInTheDocument();
  });

  it("edits top-level Other Fields JSON through the real form", async () => {
    const user = userEvent.setup();
    const onOtherFieldsStrChange = vi.fn();

    render(
      <OmoFormFields
        {...defaultProps}
        onOtherFieldsStrChange={onOtherFieldsStrChange}
      />,
    );

    await user.click(screen.getByText("Other Fields (JSON)"));
    fireEvent.change(screen.getByPlaceholderText('{ "custom_key": "value" }'), {
      target: { value: '{"disabled_agents":["oracle"]}' },
    });

    expect(onOtherFieldsStrChange).toHaveBeenLastCalledWith(
      '{"disabled_agents":["oracle"]}',
    );
  });

  it("edits common top-level options with structured controls", async () => {
    const user = userEvent.setup();
    const onOtherFieldsStrChange = vi.fn();

    render(
      <OmoFormFields
        {...defaultProps}
        otherFieldsStr='{"disabled_agents":["oracle"]}'
        onOtherFieldsStrChange={onOtherFieldsStrChange}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Default" }));
    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      $schema:
        "https://raw.githubusercontent.com/code-yeongyu/oh-my-openagent/dev/assets/oh-my-opencode.schema.json",
      disabled_agents: ["oracle"],
    });

    await user.click(screen.getByLabelText("Auto Update"));
    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      auto_update: true,
      disabled_agents: ["oracle"],
    });

    await user.click(screen.getByRole("checkbox", { name: "Atlas" }));
    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      disabled_agents: ["oracle", "Atlas"],
    });

    await user.click(screen.getByLabelText("New Task System"));
    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      new_task_system_enabled: true,
      disabled_agents: ["oracle"],
    });

    fireEvent.change(screen.getByLabelText("MCP Env Allowlist"), {
      target: { value: "PATH,HOME" },
    });
    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      mcp_env_allowlist: ["PATH", "HOME"],
      disabled_agents: ["oracle"],
    });
  });

  it("renders upstream standard OMO agents added by the newer schema", () => {
    render(<OmoFormFields {...defaultProps} />);

    expect(screen.getAllByText("Build").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Plan").length).toBeGreaterThan(0);
    expect(screen.getAllByText("OpenCode-Builder").length).toBeGreaterThan(0);
  });

  it("adds OMO Slim object templates to top-level fields", async () => {
    const user = userEvent.setup();
    const onOtherFieldsStrChange = vi.fn();

    render(
      <OmoFormFields
        {...defaultProps}
        isSlim
        onOtherFieldsStrChange={onOtherFieldsStrChange}
      />,
    );

    await user.click(screen.getByRole("button", { name: "council" }));

    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      council: {
        enabled: true,
        minVotes: 2,
      },
    });
  });

  it("edits standard OMO object fields with structured controls", async () => {
    const user = userEvent.setup();
    const onOtherFieldsStrChange = vi.fn();

    render(
      <OmoFormFields
        {...defaultProps}
        otherFieldsStr='{"tmux":{"enabled":true},"runtime_fallback":{"enabled":true},"claude_code":{"mcp":true}}'
        onOtherFieldsStrChange={onOtherFieldsStrChange}
      />,
    );

    fireEvent.change(screen.getByLabelText("main_pane_size"), {
      target: { value: "70" },
    });
    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      tmux: {
        enabled: true,
        layout: "main-vertical",
        main_pane_size: 70,
        main_pane_min_width: 120,
        agent_pane_min_width: 40,
        isolation: "inline",
      },
      runtime_fallback: { enabled: true },
      claude_code: { mcp: true },
    });

    fireEvent.change(screen.getByLabelText("retry_on_errors"), {
      target: { value: "429,503" },
    });
    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      tmux: { enabled: true },
      runtime_fallback: {
        enabled: true,
        retry_on_errors: [429, 503],
        max_fallback_attempts: 3,
        cooldown_seconds: 30,
        timeout_seconds: 0,
        notify_on_fallback: true,
      },
      claude_code: { mcp: true },
    });

    await user.click(screen.getByLabelText("plugins"));
    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      tmux: { enabled: true },
      runtime_fallback: { enabled: true },
      claude_code: {
        mcp: true,
        commands: true,
        skills: true,
        agents: true,
        hooks: true,
        plugins: false,
      },
    });
  });

  it("edits remaining standard OMO object fields with structured controls", async () => {
    const onOtherFieldsStrChange = vi.fn();

    render(
      <OmoFormFields
        {...defaultProps}
        otherFieldsStr='{"sisyphus_agent":{"disabled":false},"git_master":{"commit_footer":true},"model_capabilities":{"enabled":true},"comment_checker":{}}'
        onOtherFieldsStrChange={onOtherFieldsStrChange}
      />,
    );

    fireEvent.change(screen.getByLabelText("babysitting.timeout_ms"), {
      target: { value: "90000" },
    });
    expect(
      JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0]),
    ).toMatchObject({
      babysitting: { timeout_ms: 90000 },
    });

    fireEvent.change(screen.getByLabelText("git_master.git_env_prefix"), {
      target: { value: "GIT_MASTER=preview" },
    });
    expect(
      JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0]),
    ).toMatchObject({
      git_master: {
        commit_footer: true,
        include_co_authored_by: true,
        git_env_prefix: "GIT_MASTER=preview",
      },
    });

    fireEvent.change(screen.getByLabelText("model_capabilities.source_url"), {
      target: { value: "https://example.com/models.json" },
    });
    expect(
      JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0]),
    ).toMatchObject({
      model_capabilities: {
        enabled: true,
        auto_refresh_on_start: true,
        refresh_timeout_ms: 10000,
        source_url: "https://example.com/models.json",
      },
    });

    fireEvent.change(screen.getByLabelText("comment_checker.custom_prompt"), {
      target: { value: "Check comments" },
    });
    expect(
      JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0]),
    ).toMatchObject({
      comment_checker: { custom_prompt: "Check comments" },
    });

    fireEvent.click(screen.getByLabelText("openclaw.enabled"));
    expect(
      JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0]),
    ).toMatchObject({
      openclaw: { enabled: true, gateways: {}, hooks: {} },
    });
  });

  it("edits OMO Slim object fields with structured controls", async () => {
    const onOtherFieldsStrChange = vi.fn();

    render(
      <OmoFormFields
        {...defaultProps}
        isSlim
        otherFieldsStr='{"multiplexer":{"enabled":true},"fallback":{"enabled":true}}'
        onOtherFieldsStrChange={onOtherFieldsStrChange}
      />,
    );

    fireEvent.change(screen.getByLabelText("multiplexer.maxConcurrent"), {
      target: { value: "5" },
    });
    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      multiplexer: { enabled: true, maxConcurrent: 5 },
      fallback: { enabled: true },
    });

    fireEvent.change(screen.getByLabelText("fallback.maxAttempts"), {
      target: { value: "4" },
    });
    expect(JSON.parse(onOtherFieldsStrChange.mock.calls.at(-1)?.[0])).toEqual({
      multiplexer: { enabled: true },
      fallback: { enabled: true, maxAttempts: 4 },
    });
  });

  it("writes preset metadata when selecting a configured model", async () => {
    const user = userEvent.setup();
    const onAgentsChange = vi.fn();

    render(
      <OmoFormFields
        {...defaultProps}
        onAgentsChange={onAgentsChange}
        presetMetaMap={{
          "openai/gpt-5.4": {
            options: { provider: "openai" },
            limit: { context: 200000, output: 16000 },
          },
        }}
      />,
    );

    await user.click(screen.getAllByRole("combobox")[0]);
    await user.click(screen.getByText("OpenAI / GPT 5.4"));

    await waitFor(() => {
      expect(onAgentsChange).toHaveBeenCalledWith({
        build: {
          model: "openai/gpt-5.4",
          options: { provider: "openai" },
          limit: { context: 200000, output: 16000 },
        },
      });
    });
  });
});

describe("mergeCustomModelsIntoStore", () => {
  it("preserves advanced fields and enriches selected custom models with preset metadata", () => {
    expect(
      mergeCustomModelsIntoStore(
        {
          existing: { model: "old/model", temperature: 0.2 },
          sisyphus: { model: "openai/gpt-5.4" },
        },
        new Set(["sisyphus"]),
        [{ key: "existing", model: "google/gemini-3-pro" }],
        {},
        {
          "google/gemini-3-pro": {
            options: { provider: "google" },
            limit: { context: 1000000 },
          },
        },
      ),
    ).toEqual({
      sisyphus: { model: "openai/gpt-5.4" },
      existing: {
        model: "google/gemini-3-pro",
        temperature: 0.2,
        options: { provider: "google" },
        limit: { context: 1000000 },
      },
    });
  });
});
