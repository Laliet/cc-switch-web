import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProxySettingsSection } from "@/components/settings/ProxySettingsSection";
import type {
  ProxyAppId,
  ProxyRecentLog,
  ProxySettings,
  ProxyStatus,
  ProxyTakeoverResult,
} from "@/types";

const settingsApiMock = vi.hoisted(() => ({
  getProxyStatus: vi.fn(),
  getProxyRecentLogs: vi.fn(),
  getFailoverQueue: vi.fn(),
  listModelPricing: vi.fn(),
  startProxy: vi.fn(),
  stopProxy: vi.fn(),
  testProxy: vi.fn(),
  setProxyTakeover: vi.fn(),
  restoreProxy: vi.fn(),
}));

const toastMock = vi.hoisted(() => ({
  success: vi.fn(),
  info: vi.fn(),
  error: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  settingsApi: settingsApiMock,
  providersApi: {
    getAll: vi.fn().mockResolvedValue({}),
  },
}));

vi.mock("sonner", () => ({
  toast: toastMock,
}));

const appLabels: Record<string, string> = {
  "apps.claude": "Claude",
  "apps.codex": "Codex",
  "apps.gemini": "Gemini",
  "apps.opencode": "OpenCode",
  "apps.omo": "OMO",
};

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) =>
      appLabels[key] ?? options?.defaultValue ?? key,
  }),
}));

const proxyApps: ProxyAppId[] = ["claude", "codex", "gemini", "opencode"];

const createAppSettings = () => ({
  enabled: false,
  autoFailoverEnabled: false,
  maxRetries: 0,
  defaultCostMultiplier: "1",
  pricingModelSource: "response",
});

const createSettings = (
  overrides: Partial<ProxySettings> = {},
): ProxySettings => ({
  enabled: false,
  host: "127.0.0.1",
  port: 3456,
  upstreamProxy: undefined,
  bindApp: "claude",
  autoStart: false,
  enableLogging: false,
  liveTakeoverActive: false,
  streamingFirstByteTimeout: 90,
  streamingIdleTimeout: 120,
  nonStreamingTimeout: 180,
  circuitFailureThreshold: 3,
  circuitRecoveryThreshold: 2,
  circuitRecoveryWaitSeconds: 60,
  circuitErrorRateThreshold: 80,
  rectifyThinkingSignature: true,
  rectifyThinkingBudget: true,
  apps: {
    claude: createAppSettings(),
    codex: createAppSettings(),
    gemini: createAppSettings(),
    opencode: createAppSettings(),
  },
  ...(overrides as Partial<ReturnType<typeof createSettings>>),
});

const createStatus = (overrides: Partial<ProxyStatus> = {}): ProxyStatus => ({
  running: false,
  address: "127.0.0.1",
  port: 3456,
  listenUrl: "http://127.0.0.1:3456",
  activeConnections: 0,
  totalRequests: 0,
  successRequests: 0,
  failedRequests: 0,
  successRate: 0,
  uptimeSeconds: 0,
  activeTargets: [],
  takeover: {
    claude: false,
    codex: false,
    gemini: false,
    opencode: false,
    omo: false,
  },
  bindApp: "claude",
  ...overrides,
});

const createRecentLog = (
  overrides: Partial<ProxyRecentLog> = {},
): ProxyRecentLog => ({
  at: "2026-05-07T08:00:00Z",
  app: "claude",
  method: "POST",
  path: "/v1/messages?key=***",
  status: 200,
  durationMs: 12,
  error: null,
  ...overrides,
});

const renderSection = (initialValue = createSettings()) => {
  const onChangeSpy = vi.fn();

  function Harness() {
    const [value, setValue] = useState(initialValue);
    return (
      <ProxySettingsSection
        value={value}
        onChange={(nextValue) => {
          onChangeSpy(nextValue);
          setValue(nextValue);
        }}
      />
    );
  }

  return {
    ...render(<Harness />),
    onChangeSpy,
  };
};

const getButton = (name: string) => screen.getByRole("button", { name });
const clickButton = (name: string) => fireEvent.click(getButton(name));

const getAppCard = (name: string) => {
  const card = screen
    .getAllByText(name)
    .map((item) => item.closest("div.rounded-md"))
    .find((item): item is HTMLElement => item instanceof HTMLElement);
  if (!card) throw new Error(`Missing app card for ${name}`);
  return card;
};

const getAppSwitch = (name: string) => {
  const switches = within(getAppCard(name)).getAllByRole("switch");
  const takeoverSwitch = switches.at(-1);
  if (!takeoverSwitch) throw new Error(`Missing takeover switch for ${name}`);
  return takeoverSwitch;
};

const waitForInitialStatus = async () => {
  await waitFor(() =>
    expect(settingsApiMock.getProxyStatus).toHaveBeenCalled(),
  );
};

beforeEach(() => {
  settingsApiMock.getProxyStatus.mockResolvedValue(createStatus());
  settingsApiMock.getProxyRecentLogs.mockResolvedValue([]);
  settingsApiMock.getFailoverQueue.mockResolvedValue([]);
  settingsApiMock.listModelPricing.mockResolvedValue([]);
  settingsApiMock.startProxy.mockResolvedValue(createStatus({ running: true }));
  settingsApiMock.stopProxy.mockResolvedValue(createStatus({ running: false }));
  settingsApiMock.testProxy.mockResolvedValue({
    success: true,
    message: "ok",
    baseUrl: "http://127.0.0.1:3456",
  });
  settingsApiMock.setProxyTakeover.mockImplementation(
    async (app: ProxyAppId, enabled: boolean) => ({
      app,
      enabled,
      status: createStatus({
        takeover: {
          claude: app === "claude" ? enabled : false,
          codex: app === "codex" ? enabled : false,
          gemini: app === "gemini" ? enabled : false,
          opencode: app === "opencode" ? enabled : false,
          omo: false,
        },
      }),
    }),
  );
  settingsApiMock.restoreProxy.mockResolvedValue(createStatus());
  settingsApiMock.getProxyRecentLogs.mockClear();
  toastMock.success.mockClear();
  toastMock.info.mockClear();
  toastMock.error.mockClear();
});

describe("ProxySettingsSection", () => {
  it("loads proxy status and renders stopped metrics", async () => {
    settingsApiMock.getProxyStatus.mockResolvedValueOnce(
      createStatus({
        running: false,
        listenUrl: "http://127.0.0.1:4567",
        totalRequests: 42,
        successRate: 87.5,
        uptimeSeconds: 123,
        lastError: "last proxy error",
      }),
    );

    renderSection(createSettings({ port: 4567 }));

    await screen.findByText("已停止");
    expect(screen.getAllByText("http://127.0.0.1:4567")).toHaveLength(2);
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.getByText("87.5%")).toBeInTheDocument();
    expect(screen.getByText("123s")).toBeInTheDocument();
    expect(screen.getByText("last proxy error")).toBeInTheDocument();
  });

  it("renders running status and enables stop", async () => {
    settingsApiMock.getProxyStatus.mockResolvedValueOnce(
      createStatus({ running: true }),
    );

    renderSection(createSettings({ enabled: true }));

    await screen.findByText("运行中");
    expect(getButton("停止代理")).toBeEnabled();
  });

  it("loads recent logs when the collapsed section opens", async () => {
    settingsApiMock.getProxyRecentLogs.mockResolvedValueOnce([
      createRecentLog(),
    ]);

    renderSection(createSettings({ enableLogging: true }));
    await waitForInitialStatus();

    fireEvent.click(screen.getByRole("button", { name: "最近请求" }));

    await waitFor(() =>
      expect(settingsApiMock.getProxyRecentLogs).toHaveBeenCalled(),
    );
    expect(
      await screen.findByText(/\/v1\/messages\?key=\*\*\*/),
    ).toBeInTheDocument();
  });

  it("renders per-app takeover rows and disables OMO", async () => {
    settingsApiMock.getProxyStatus.mockResolvedValueOnce(
      createStatus({
        activeTargets: proxyApps.map((app) => ({
          appType: app,
          providerId: `${app}-provider`,
          providerName: `${app} provider`,
        })),
      }),
    );

    renderSection();
    await waitForInitialStatus();

    expect(getAppCard("Claude")).toBeInTheDocument();
    expect(getAppCard("Codex")).toBeInTheDocument();
    expect(getAppCard("Gemini")).toBeInTheDocument();
    expect(getAppCard("OpenCode")).toBeInTheDocument();
    expect(
      screen.getByText(/选择要被 cc-switch-web 修改配置/),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Claude 接管：让 Claude 走本地代理"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "测试 Claude" }),
    ).toBeInTheDocument();
    expect(screen.getByText("实验性")).toBeInTheDocument();
    expect(screen.getByText("OMO")).toBeInTheDocument();
    expect(screen.getByText("暂不支持代理接管")).toBeInTheDocument();
    expect(within(getAppCard("OMO")).getByRole("switch")).toBeDisabled();
    expect(screen.getByText("claude provider")).toBeInTheDocument();
    expect(screen.getByText("codex provider")).toBeInTheDocument();
    expect(screen.getByText("gemini provider")).toBeInTheDocument();
    expect(screen.getByText("opencode provider")).toBeInTheDocument();
  });

  it("starts proxy successfully", async () => {
    const nextStatus = createStatus({
      running: true,
      listenUrl: "http://127.0.0.1:3456",
    });
    settingsApiMock.startProxy.mockResolvedValueOnce(nextStatus);
    const { onChangeSpy } = renderSection();
    await waitForInitialStatus();

    clickButton("启动代理");

    await waitFor(() =>
      expect(settingsApiMock.startProxy).toHaveBeenCalledWith(
        expect.objectContaining({ enabled: true }),
      ),
    );
    expect(onChangeSpy).toHaveBeenLastCalledWith(
      expect.objectContaining({ enabled: true }),
    );
    expect(await screen.findByText("运行中")).toBeInTheDocument();
    expect(toastMock.success).toHaveBeenCalled();
  });

  it("shows start failures without enabling proxy", async () => {
    settingsApiMock.startProxy.mockRejectedValueOnce(new Error("port in use"));
    const { onChangeSpy } = renderSection();
    await waitForInitialStatus();

    clickButton("启动代理");

    await waitFor(() => expect(toastMock.error).toHaveBeenCalled());
    expect(settingsApiMock.startProxy).toHaveBeenCalledTimes(1);
    expect(onChangeSpy).not.toHaveBeenCalledWith(
      expect.objectContaining({ enabled: true }),
    );
  });

  it("refreshes status instead of starting again when proxy is already running", async () => {
    settingsApiMock.getProxyStatus.mockResolvedValue(
      createStatus({ running: true }),
    );
    renderSection();
    await waitForInitialStatus();

    clickButton("启动代理");

    await waitFor(() => expect(toastMock.info).toHaveBeenCalled());
    expect(settingsApiMock.startProxy).not.toHaveBeenCalled();
    expect(settingsApiMock.getProxyStatus).toHaveBeenCalledTimes(2);
  });

  it("stops proxy successfully", async () => {
    settingsApiMock.getProxyStatus.mockResolvedValueOnce(
      createStatus({ running: true }),
    );
    const { onChangeSpy } = renderSection(createSettings({ enabled: true }));
    await screen.findByText("运行中");

    clickButton("停止代理");

    await waitFor(() => expect(settingsApiMock.stopProxy).toHaveBeenCalled());
    expect(onChangeSpy).toHaveBeenLastCalledWith(
      expect.objectContaining({ enabled: false }),
    );
    expect(await screen.findByText("已停止")).toBeInTheDocument();
    expect(toastMock.success).toHaveBeenCalled();
  });

  it("shows stop failures", async () => {
    settingsApiMock.getProxyStatus.mockResolvedValueOnce(
      createStatus({ running: true }),
    );
    settingsApiMock.stopProxy.mockRejectedValueOnce(new Error("stop failed"));
    renderSection(createSettings({ enabled: true }));
    await screen.findByText("运行中");

    clickButton("停止代理");

    await waitFor(() => expect(toastMock.error).toHaveBeenCalled());
    expect(settingsApiMock.stopProxy).toHaveBeenCalledTimes(1);
  });

  it("tests proxy successfully", async () => {
    renderSection();
    await waitForInitialStatus();

    clickButton("测试绑定客户端：Claude");

    await waitFor(() =>
      expect(settingsApiMock.testProxy).toHaveBeenCalledWith(
        expect.objectContaining({
          host: "127.0.0.1",
          port: 3456,
          bindApp: "claude",
        }),
      ),
    );
    expect(toastMock.success).toHaveBeenCalled();
  });

  it("tests a specific takeover app without changing the default bind app", async () => {
    const { onChangeSpy } = renderSection(createSettings({ bindApp: "codex" }));
    await waitForInitialStatus();

    clickButton("测试 Gemini");

    await waitFor(() =>
      expect(settingsApiMock.testProxy).toHaveBeenCalledWith(
        expect.objectContaining({ bindApp: "gemini" }),
      ),
    );
    expect(onChangeSpy).not.toHaveBeenCalledWith(
      expect.objectContaining({ bindApp: "gemini" }),
    );
    expect(toastMock.success).toHaveBeenCalled();
  });

  it("shows test failures", async () => {
    settingsApiMock.testProxy.mockRejectedValueOnce(new Error("bad config"));
    renderSection();
    await waitForInitialStatus();

    clickButton("测试绑定客户端：Claude");

    await waitFor(() => expect(toastMock.error).toHaveBeenCalled());
    expect(settingsApiMock.testProxy).toHaveBeenCalledTimes(1);
  });

  it("enables and disables takeover", async () => {
    renderSection();
    await waitForInitialStatus();

    fireEvent.click(getAppSwitch("Claude"));

    await waitFor(() =>
      expect(settingsApiMock.setProxyTakeover).toHaveBeenCalledWith(
        "claude",
        true,
      ),
    );
    expect(toastMock.success).toHaveBeenCalled();

    fireEvent.click(getAppSwitch("Claude"));

    await waitFor(() =>
      expect(settingsApiMock.setProxyTakeover).toHaveBeenLastCalledWith(
        "claude",
        false,
      ),
    );
  });

  it("dedupes takeover requests and uses a stable short-lived toast", async () => {
    let resolveTakeover: (result: ProxyTakeoverResult) => void = () => {};
    settingsApiMock.setProxyTakeover.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveTakeover = resolve;
      }),
    );
    renderSection();
    await waitForInitialStatus();

    const claudeSwitch = getAppSwitch("Claude");
    fireEvent.click(claudeSwitch);
    fireEvent.click(claudeSwitch);

    expect(settingsApiMock.setProxyTakeover).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(claudeSwitch).toBeDisabled());

    resolveTakeover({
      app: "claude",
      enabled: true,
      status: createStatus({
        takeover: {
          claude: true,
          codex: false,
          gemini: false,
          opencode: false,
          omo: false,
        },
      }),
    });

    await waitFor(() =>
      expect(toastMock.success).toHaveBeenCalledWith("接管已开启", {
        description: "Claude",
        duration: 1800,
        id: "proxy-takeover-claude",
      }),
    );
  });

  it("reverts takeover when the API call fails", async () => {
    settingsApiMock.setProxyTakeover.mockRejectedValueOnce(
      new Error("takeover failed"),
    );
    renderSection();
    await waitForInitialStatus();

    const claudeSwitch = getAppSwitch("Claude");
    fireEvent.click(claudeSwitch);

    await waitFor(() => expect(toastMock.error).toHaveBeenCalled());
    expect(claudeSwitch).not.toBeChecked();
  });

  it("restores takeover config successfully", async () => {
    const initial = createSettings({
      liveTakeoverActive: true,
      apps: {
        claude: { ...createAppSettings(), enabled: true },
        codex: { ...createAppSettings(), enabled: true },
        gemini: { ...createAppSettings(), enabled: true },
        opencode: { ...createAppSettings(), enabled: true },
      },
    });
    const { onChangeSpy } = renderSection(initial);
    await waitForInitialStatus();

    clickButton("恢复接管");

    await waitFor(() =>
      expect(settingsApiMock.restoreProxy).toHaveBeenCalled(),
    );
    expect(onChangeSpy).toHaveBeenLastCalledWith(
      expect.objectContaining({
        liveTakeoverActive: false,
        apps: expect.objectContaining({
          claude: expect.objectContaining({ enabled: false }),
          codex: expect.objectContaining({ enabled: false }),
          gemini: expect.objectContaining({ enabled: false }),
          opencode: expect.objectContaining({ enabled: false }),
        }),
      }),
    );
    expect(toastMock.success).toHaveBeenCalled();
  });

  it("shows restore failures", async () => {
    settingsApiMock.restoreProxy.mockRejectedValueOnce(
      new Error("restore failed"),
    );
    renderSection();
    await waitForInitialStatus();

    clickButton("恢复接管");

    await waitFor(() => expect(toastMock.error).toHaveBeenCalled());
    expect(settingsApiMock.restoreProxy).toHaveBeenCalledTimes(1);
  });

  it("rejects empty host before starting", async () => {
    renderSection();
    await waitForInitialStatus();

    fireEvent.change(screen.getByLabelText("监听地址"), {
      target: { value: "   " },
    });
    clickButton("启动代理");

    expect(settingsApiMock.startProxy).not.toHaveBeenCalled();
    expect(toastMock.error).toHaveBeenCalled();
  });

  it("rejects invalid ports before starting", async () => {
    renderSection();
    await waitForInitialStatus();

    fireEvent.change(screen.getByLabelText("端口"), {
      target: { value: "70000" },
    });
    clickButton("启动代理");

    expect(settingsApiMock.startProxy).not.toHaveBeenCalled();
    expect(toastMock.error).toHaveBeenCalled();
  });

  it("blocks public bind until proxy is explicitly enabled", async () => {
    renderSection();
    await waitForInitialStatus();

    fireEvent.change(screen.getByLabelText("监听地址"), {
      target: { value: "0.0.0.0" },
    });

    expect(
      screen.getByText(
        "当前会暴露到所有网卡。请只在可信内网或 TLS 反代后使用。",
      ),
    ).toBeInTheDocument();

    clickButton("启动代理");

    expect(settingsApiMock.startProxy).not.toHaveBeenCalled();
    expect(toastMock.error).toHaveBeenCalled();
  });

  it("disables actions while an operation is in progress", async () => {
    let resolveStart: (status: ProxyStatus) => void = () => {};
    settingsApiMock.startProxy.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveStart = resolve;
      }),
    );
    renderSection();
    await waitForInitialStatus();

    clickButton("启动代理");

    await waitFor(() => expect(getButton("启动代理")).toBeDisabled());
    expect(getButton("测试绑定客户端：Claude")).toBeDisabled();
    expect(getButton("停止代理")).toBeDisabled();
    expect(getButton("恢复接管")).toBeDisabled();
    expect(getAppSwitch("Codex")).toBeDisabled();

    resolveStart(createStatus({ running: true }));
    await waitFor(() => expect(getButton("启动代理")).toBeEnabled());
  });
});
