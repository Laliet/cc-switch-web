import type { AppId } from "@/lib/api/types";
import type {
  McpServer,
  Provider,
  ProxyAppId,
  ProxyRecentLog,
  ProxySettings,
  ProxyStatus,
  ProxyTestResult,
  ProxyTakeoverResult,
  Settings,
} from "@/types";
import type { Skill, SkillRepo, SkillsResponse } from "@/lib/api/skills";
import { isMcpApp, type McpAppId } from "@/config/apps";

type ProvidersByApp = Record<AppId, Record<string, Provider>>;
type CurrentProviderState = Record<AppId, string>;
type BackupProviderState = Record<AppId, string | null>;
type McpConfigState = Record<McpAppId, Record<string, McpServer>>;
type McpServersState = Record<string, McpServer>;
type SkillsState = Skill[];
type SkillReposState = SkillRepo[];

const createDefaultProviders = (): ProvidersByApp => ({
  claude: {
    "claude-1": {
      id: "claude-1",
      name: "Claude Default",
      settingsConfig: {},
      category: "official",
      sortIndex: 0,
      createdAt: Date.now(),
    },
    "claude-2": {
      id: "claude-2",
      name: "Claude Custom",
      settingsConfig: {},
      category: "custom",
      sortIndex: 1,
      createdAt: Date.now() + 1,
    },
  },
  "claude-desktop": {
    "claude-desktop-1": {
      id: "claude-desktop-1",
      name: "Claude Desktop Default",
      settingsConfig: {},
      category: "official",
      sortIndex: 0,
      createdAt: Date.now(),
      meta: {
        claudeDesktopMode: "direct",
      },
    },
  },
  codex: {
    "codex-1": {
      id: "codex-1",
      name: "Codex Default",
      settingsConfig: {},
      category: "official",
      sortIndex: 0,
      createdAt: Date.now(),
    },
    "codex-2": {
      id: "codex-2",
      name: "Codex Secondary",
      settingsConfig: {},
      category: "custom",
      sortIndex: 1,
      createdAt: Date.now() + 1,
    },
  },
  gemini: {
    "gemini-1": {
      id: "gemini-1",
      name: "Gemini Default",
      settingsConfig: {
        env: {
          GEMINI_API_KEY: "test-key",
          GOOGLE_GEMINI_BASE_URL: "https://generativelanguage.googleapis.com",
        },
      },
      category: "official",
      sortIndex: 0,
      createdAt: Date.now(),
    },
  },
  opencode: {
    "opencode-1": {
      id: "opencode-1",
      name: "OpenCode Default",
      settingsConfig: {
        npm: "@ai-sdk/openai-compatible",
        options: {
          apiKey: "test-key",
          baseURL: "https://api.example.com/v1",
        },
        models: {},
      },
      category: "custom",
      sortIndex: 0,
      createdAt: Date.now(),
    },
  },
  omo: {
    "omo-1": {
      id: "omo-1",
      name: "OMO Default",
      settingsConfig: {
        agents: {},
        categories: {},
      },
      category: "custom",
      sortIndex: 0,
      createdAt: Date.now(),
    },
  },
  "omo-slim": {
    "omo-slim-1": {
      id: "omo-slim-1",
      name: "OMO Slim Default",
      settingsConfig: {
        agents: {},
      },
      category: "custom",
      sortIndex: 0,
      createdAt: Date.now(),
    },
  },
});

const createDefaultCurrent = (): CurrentProviderState => ({
  claude: "claude-1",
  "claude-desktop": "claude-desktop-1",
  codex: "codex-1",
  gemini: "gemini-1",
  opencode: "opencode-1",
  omo: "omo-1",
  "omo-slim": "omo-slim-1",
});

const createDefaultBackup = (): BackupProviderState => ({
  claude: null,
  "claude-desktop": null,
  codex: null,
  gemini: null,
  opencode: null,
  omo: null,
  "omo-slim": null,
});

const createDefaultSkills = (): SkillsState => [
  {
    key: "terminal",
    name: "Terminal Helper",
    description: "Execute shell commands",
    directory: "/skills/terminal",
    installed: true,
    repoOwner: "mock",
    repoName: "builtin-skills",
    repoBranch: "main",
  },
  {
    key: "notes",
    name: "Notes",
    description: "Take notes quickly",
    directory: "/skills/notes",
    installed: false,
    repoOwner: "community",
    repoName: "ai-skills",
    repoBranch: "main",
    skillsPath: "skills",
  },
];

const createDefaultSkillRepos = (): SkillReposState => [
  {
    owner: "mock",
    name: "builtin-skills",
    branch: "main",
    enabled: true,
  },
  {
    owner: "community",
    name: "ai-skills",
    branch: "main",
    enabled: false,
    skillsPath: "skills",
  },
];

const createDefaultProxyAppSettings = () => ({
  enabled: false,
  autoFailoverEnabled: false,
  maxRetries: 0,
  streamingFirstByteTimeout: 90,
  streamingIdleTimeout: 120,
  nonStreamingTimeout: 600,
  circuitFailureThreshold: 3,
  circuitRecoveryThreshold: 2,
  circuitRecoveryWaitSeconds: 60,
  circuitErrorRateThreshold: 80,
  circuitMinRequests: 10,
});

const createDefaultProxySettings = (): ProxySettings => ({
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
  nonStreamingTimeout: 600,
  circuitFailureThreshold: 3,
  circuitRecoveryThreshold: 2,
  circuitRecoveryWaitSeconds: 60,
  circuitErrorRateThreshold: 80,
  rectifyThinkingSignature: true,
  rectifyThinkingBudget: true,
  optimizerEnabled: false,
  optimizerThinking: true,
  optimizerCacheInjection: true,
  optimizerCacheTtl: "1h",
  apps: {
    claude: createDefaultProxyAppSettings(),
    codex: createDefaultProxyAppSettings(),
    gemini: createDefaultProxyAppSettings(),
    opencode: createDefaultProxyAppSettings(),
  },
});

const createDefaultProxyStatus = (): ProxyStatus => ({
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
  failoverCount: 0,
  lastFailoverAt: undefined,
  lastFailoverFrom: undefined,
  lastFailoverTo: undefined,
});

let providers = createDefaultProviders();
let current = createDefaultCurrent();
let backup = createDefaultBackup();
let skills = createDefaultSkills();
let skillRepos = createDefaultSkillRepos();
let proxyStatusState = createDefaultProxyStatus();
let proxyRecentLogsState: ProxyRecentLog[] = [];
let settingsState: Settings = {
  showInTray: true,
  minimizeToTrayOnClose: true,
  enableClaudePluginIntegration: false,
  claudeConfigDir: "/default/claude",
  codexConfigDir: "/default/codex",
  geminiConfigDir: "/default/gemini",
  opencodeConfigDir: "/default/opencode",
  language: "zh",
  network: {
    githubMirrorBaseUrl: "",
  },
  skillStorageLocation: "cc_switch",
  skillSyncMethod: "auto",
  proxy: createDefaultProxySettings(),
};
let appConfigDirOverride: string | null = null;
let mcpConfigs: McpConfigState = {
  claude: {
    sample: {
      id: "sample",
      name: "Sample Claude Server",
      enabled: true,
      apps: { claude: true, codex: false, gemini: false, opencode: false },
      server: {
        type: "stdio",
        command: "claude-server",
      },
    },
  },
  codex: {
    httpServer: {
      id: "httpServer",
      name: "HTTP Codex Server",
      enabled: false,
      apps: { claude: false, codex: true, gemini: false, opencode: false },
      server: {
        type: "http",
        url: "http://localhost:3000",
      },
    },
  },
  gemini: {},
  opencode: {},
  omo: {},
  "omo-slim": {},
};
const buildUnifiedMcpServers = (configs: McpConfigState): McpServersState => {
  const merged: McpServersState = {};
  (Object.keys(configs) as McpAppId[]).forEach((app) => {
    const servers = configs[app];
    Object.values(servers).forEach((server) => {
      const existing = merged[server.id];
      if (!existing) {
        merged[server.id] = JSON.parse(JSON.stringify(server)) as McpServer;
        return;
      }
      merged[server.id] = {
        ...existing,
        apps: {
          claude: existing.apps?.claude || server.apps?.claude || false,
          codex: existing.apps?.codex || server.apps?.codex || false,
          gemini: existing.apps?.gemini || server.apps?.gemini || false,
          opencode: existing.apps?.opencode || server.apps?.opencode || false,
        },
      };
    });
  });
  return merged;
};
let mcpServers: McpServersState = buildUnifiedMcpServers(mcpConfigs);

const cloneProviders = (value: ProvidersByApp) =>
  JSON.parse(JSON.stringify(value)) as ProvidersByApp;

const cloneSkills = (value: SkillsState) =>
  JSON.parse(JSON.stringify(value)) as SkillsState;

const cloneSkillRepos = (value: SkillReposState) =>
  JSON.parse(JSON.stringify(value)) as SkillReposState;

export const resetProviderState = () => {
  providers = createDefaultProviders();
  current = createDefaultCurrent();
  backup = createDefaultBackup();
  skills = createDefaultSkills();
  skillRepos = createDefaultSkillRepos();
  proxyStatusState = createDefaultProxyStatus();
  proxyRecentLogsState = [];
  settingsState = {
    showInTray: true,
    minimizeToTrayOnClose: true,
    enableClaudePluginIntegration: false,
    claudeConfigDir: "/default/claude",
    codexConfigDir: "/default/codex",
    geminiConfigDir: "/default/gemini",
    opencodeConfigDir: "/default/opencode",
    language: "zh",
    network: {
      githubMirrorBaseUrl: "",
    },
    proxy: createDefaultProxySettings(),
  };
  appConfigDirOverride = null;
  mcpConfigs = {
    claude: {
      sample: {
        id: "sample",
        name: "Sample Claude Server",
        enabled: true,
        apps: {
          claude: true,
          codex: false,
          gemini: false,
          opencode: false,
        },
        server: {
          type: "stdio",
          command: "claude-server",
        },
      },
    },
    codex: {
      httpServer: {
        id: "httpServer",
        name: "HTTP Codex Server",
        enabled: false,
        apps: {
          claude: false,
          codex: true,
          gemini: false,
          opencode: false,
        },
        server: {
          type: "http",
          url: "http://localhost:3000",
        },
      },
    },
    gemini: {},
    opencode: {},
    omo: {},
    "omo-slim": {},
  };
  mcpServers = buildUnifiedMcpServers(mcpConfigs);
};

export const getProviders = (appType: AppId) =>
  cloneProviders(providers)[appType] ?? {};

export const getCurrentProviderId = (appType: AppId) => current[appType] ?? "";

export const getBackupProviderId = (appType: AppId) => backup[appType] ?? null;

export const setBackupProviderId = (
  appType: AppId,
  providerId: string | null,
) => {
  backup[appType] = providerId;
};

export const setCurrentProviderId = (appType: AppId, providerId: string) => {
  current[appType] = providerId;
};

export const updateProviders = (
  appType: AppId,
  data: Record<string, Provider>,
) => {
  providers[appType] = cloneProviders({ [appType]: data } as ProvidersByApp)[
    appType
  ];
};

export const setProviders = (
  appType: AppId,
  data: Record<string, Provider>,
) => {
  providers[appType] = JSON.parse(JSON.stringify(data)) as Record<
    string,
    Provider
  >;
};

export const addProvider = (appType: AppId, provider: Provider) => {
  providers[appType] = providers[appType] ?? {};
  providers[appType][provider.id] = provider;
};

export const updateProvider = (appType: AppId, provider: Provider) => {
  if (!providers[appType]) return;
  providers[appType][provider.id] = {
    ...providers[appType][provider.id],
    ...provider,
  };
};

export const deleteProvider = (appType: AppId, providerId: string) => {
  if (!providers[appType]) return;
  delete providers[appType][providerId];
  if (current[appType] === providerId) {
    const fallback = Object.keys(providers[appType])[0] ?? "";
    current[appType] = fallback;
  }
};

export const updateSortOrder = (
  appType: AppId,
  updates: { id: string; sortIndex: number }[],
) => {
  if (!providers[appType]) return;
  updates.forEach(({ id, sortIndex }) => {
    const provider = providers[appType][id];
    if (provider) {
      providers[appType][id] = { ...provider, sortIndex };
    }
  });
};

export const listProviders = (appType: AppId) =>
  JSON.parse(JSON.stringify(providers[appType] ?? {})) as Record<
    string,
    Provider
  >;

export const getSkillsState = (): SkillsResponse => ({
  skills: cloneSkills(skills),
  warnings: [],
  cacheHit: false,
  refreshing: false,
});

export const installSkillState = (directory: string) => {
  const existing = skills.find((item) => item.directory === directory);
  if (existing) {
    existing.installed = true;
    return;
  }
  const key = directory.split("/").filter(Boolean).pop() ?? directory;
  skills.push({
    key,
    name: key,
    description: "",
    directory,
    installed: true,
  });
};

export const uninstallSkillState = (directory: string) => {
  const existing = skills.find((item) => item.directory === directory);
  if (existing) {
    existing.installed = false;
  }
};

export const getSkillReposState = () => cloneSkillRepos(skillRepos);

export const addSkillRepoState = (repo: SkillRepo) => {
  const index = skillRepos.findIndex(
    (item) => item.owner === repo.owner && item.name === repo.name,
  );
  const nextRepo = JSON.parse(JSON.stringify(repo)) as SkillRepo;
  if (index >= 0) {
    skillRepos[index] = { ...skillRepos[index], ...nextRepo };
    return;
  }
  skillRepos.push(nextRepo);
};

export const removeSkillRepoState = (owner: string, name: string) => {
  skillRepos = skillRepos.filter(
    (repo) => !(repo.owner === owner && repo.name === name),
  );
};

export const getSettings = () =>
  JSON.parse(JSON.stringify(settingsState)) as Settings;

export const setSettings = (data: Partial<Settings>) => {
  settingsState = { ...settingsState, ...data };
};

const cloneProxySettings = (value: ProxySettings) =>
  JSON.parse(JSON.stringify(value)) as ProxySettings;

const cloneProxyStatus = (value: ProxyStatus) =>
  JSON.parse(JSON.stringify(value)) as ProxyStatus;

const cloneProxyRecentLogs = (value: ProxyRecentLog[]) =>
  JSON.parse(JSON.stringify(value)) as ProxyRecentLog[];

const getCurrentProxySettings = () =>
  cloneProxySettings(settingsState.proxy ?? createDefaultProxySettings());

const proxyTakeoverFromSettings = (settings: ProxySettings) => ({
  claude: settings.apps.claude.enabled,
  codex: settings.apps.codex.enabled,
  gemini: settings.apps.gemini.enabled,
  opencode: settings.apps.opencode.enabled,
  omo: false,
});

const updateProxyStatusFromSettings = (
  settings: ProxySettings,
  running = proxyStatusState.running,
) => {
  proxyStatusState = {
    ...proxyStatusState,
    running,
    address: settings.host,
    port: settings.port,
    listenUrl: `http://${settings.host}:${settings.port}`,
    takeover: proxyTakeoverFromSettings(settings),
    bindApp: settings.bindApp,
    activeTargets: (Object.keys(settings.apps) as ProxyAppId[])
      .filter((app) => settings.apps[app].enabled)
      .map((app) => ({
        appType: app,
        providerId: current[app],
        providerName: providers[app]?.[current[app]]?.name ?? current[app],
      })),
  };
};

export const getProxyConfigState = () => getCurrentProxySettings();

export const setProxyConfigState = (settings: ProxySettings) => {
  settingsState = {
    ...settingsState,
    proxy: cloneProxySettings(settings),
  };
  if (!settings.enableLogging) {
    clearProxyRecentLogsState();
  }
  updateProxyStatusFromSettings(settings);
  return getProxyConfigState();
};

export const getProxyStatusState = () => cloneProxyStatus(proxyStatusState);

export const getProxyRecentLogsState = () =>
  getCurrentProxySettings().enableLogging
    ? cloneProxyRecentLogs(proxyRecentLogsState)
    : [];

export const addProxyRecentLogState = (log: ProxyRecentLog) => {
  const settings = getCurrentProxySettings();
  if (!settings.enableLogging) return;
  proxyRecentLogsState = [...proxyRecentLogsState, log].slice(-100);
};

export const clearProxyRecentLogsState = () => {
  proxyRecentLogsState = [];
};

export const startProxyState = (settings: ProxySettings) => {
  proxyRecentLogsState = [];
  const saved = setProxyConfigState({
    ...settings,
    enabled: true,
    liveTakeoverActive: Object.values(settings.apps).some((app) => app.enabled),
  });
  updateProxyStatusFromSettings(saved, true);
  return getProxyStatusState();
};

export const stopProxyState = () => {
  const settings = getCurrentProxySettings();
  const nextSettings: ProxySettings = {
    ...settings,
    enabled: false,
    liveTakeoverActive: false,
    apps: {
      claude: { ...settings.apps.claude, enabled: false },
      codex: { ...settings.apps.codex, enabled: false },
      gemini: { ...settings.apps.gemini, enabled: false },
      opencode: { ...settings.apps.opencode, enabled: false },
    },
  };
  setProxyConfigState(nextSettings);
  updateProxyStatusFromSettings(nextSettings, false);
  clearProxyRecentLogsState();
  return getProxyStatusState();
};

export const testProxyState = (settings: ProxySettings): ProxyTestResult => ({
  success: settings.host.trim().length > 0 && settings.port > 0,
  message: "ok",
  baseUrl: `http://${settings.host}:${settings.port}`,
});

export const setProxyTakeoverState = (
  app: ProxyAppId,
  enabled: boolean,
): ProxyTakeoverResult => {
  const settings = getCurrentProxySettings();
  const nextSettings: ProxySettings = {
    ...settings,
    liveTakeoverActive:
      enabled ||
      (Object.keys(settings.apps) as ProxyAppId[]).some(
        (item) => item !== app && settings.apps[item].enabled,
      ),
    apps: {
      ...settings.apps,
      [app]: {
        ...settings.apps[app],
        enabled,
      },
    },
  };
  setProxyConfigState(nextSettings);
  return {
    app,
    enabled,
    status: getProxyStatusState(),
  };
};

export const restoreProxyState = () => stopProxyState();

export const recoverStaleProxyTakeoverState = () => restoreProxyState();

export const getAppConfigDirOverride = () => appConfigDirOverride;

export const setAppConfigDirOverrideState = (value: string | null) => {
  appConfigDirOverride = value;
};

export const getMcpConfig = (appType: AppId) => {
  if (!isMcpApp(appType)) {
    return {
      configPath: `/mock/${appType}.mcp.json`,
      servers: {},
    };
  }
  const servers = JSON.parse(
    JSON.stringify(mcpConfigs[appType] ?? {}),
  ) as Record<string, McpServer>;
  return {
    configPath: `/mock/${appType}.mcp.json`,
    servers,
  };
};

export const setMcpConfig = (
  appType: AppId,
  value: Record<string, McpServer>,
) => {
  if (!isMcpApp(appType)) return;
  mcpConfigs[appType] = JSON.parse(JSON.stringify(value)) as Record<
    string,
    McpServer
  >;
};

export const setMcpServerEnabled = (
  appType: AppId,
  id: string,
  enabled: boolean,
) => {
  if (!isMcpApp(appType)) return;
  if (!mcpConfigs[appType]?.[id]) return;
  mcpConfigs[appType][id] = {
    ...mcpConfigs[appType][id],
    enabled,
  };
};

export const upsertMcpServer = (
  appType: AppId,
  id: string,
  server: McpServer,
) => {
  if (!isMcpApp(appType)) return;
  if (!mcpConfigs[appType]) {
    mcpConfigs[appType] = {};
  }
  mcpConfigs[appType][id] = JSON.parse(JSON.stringify(server)) as McpServer;
};

export const deleteMcpServer = (appType: AppId, id: string) => {
  if (!isMcpApp(appType)) return;
  if (!mcpConfigs[appType]) return;
  delete mcpConfigs[appType][id];
};

export const getUnifiedMcpServers = () =>
  JSON.parse(JSON.stringify(mcpServers)) as McpServersState;

export const upsertUnifiedMcpServer = (server: McpServer) => {
  mcpServers[server.id] = JSON.parse(JSON.stringify(server)) as McpServer;
};

export const deleteUnifiedMcpServer = (id: string) => {
  delete mcpServers[id];
};

export const toggleMcpAppState = (id: string, app: AppId, enabled: boolean) => {
  if (!mcpServers[id]) return;
  if (app === "omo") return;
  mcpServers[id] = {
    ...mcpServers[id],
    apps: {
      claude: mcpServers[id].apps?.claude || false,
      codex: mcpServers[id].apps?.codex || false,
      gemini: mcpServers[id].apps?.gemini || false,
      opencode: mcpServers[id].apps?.opencode || false,
      [app]: enabled,
    },
  };
};
