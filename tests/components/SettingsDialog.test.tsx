import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createContext, useContext } from "react";
import { SettingsDialog } from "@/components/settings/SettingsDialog";

const toastSuccessMock = vi.fn();
const toastErrorMock = vi.fn();

vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => toastSuccessMock(...args),
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

const tMock = vi.fn((key: string) => key);
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

vi.mock("@/lib/query", () => ({
  useCapabilitiesQuery: () => ({
    data: {
      features: {
        claudePluginIntegration: true,
        tray: true,
      },
    },
  }),
}));

const adapterMocks = vi.hoisted(() => ({
  isWeb: vi.fn(() => true),
  getStoredWebUsername: vi.fn(() => "admin"),
  getWebApiBase: vi.fn(() => "/api"),
  setWebCredentials: vi.fn(),
}));

vi.mock("@/lib/api/adapter", () => adapterMocks);

interface SettingsMock {
  settings: any;
  isLoading: boolean;
  isSaving: boolean;
  isPortable: boolean;
  appConfigDir?: string;
  resolvedDirs: Record<string, string>;
  requiresRestart: boolean;
  updateSettings: ReturnType<typeof vi.fn>;
  updateDirectory: ReturnType<typeof vi.fn>;
  updateAppConfigDir: ReturnType<typeof vi.fn>;
  browseDirectory: ReturnType<typeof vi.fn>;
  browseAppConfigDir: ReturnType<typeof vi.fn>;
  resetDirectory: ReturnType<typeof vi.fn>;
  resetAppConfigDir: ReturnType<typeof vi.fn>;
  applyWslTemplate: ReturnType<typeof vi.fn>;
  saveSettings: ReturnType<typeof vi.fn>;
  resetSettings: ReturnType<typeof vi.fn>;
  acknowledgeRestart: ReturnType<typeof vi.fn>;
}

const createSettingsMock = (overrides: Partial<SettingsMock> = {}) => {
  const base: SettingsMock = {
    settings: {
      showInTray: true,
      minimizeToTrayOnClose: true,
      enableClaudePluginIntegration: false,
      language: "zh",
      claudeConfigDir: "/claude",
      codexConfigDir: "/codex",
    },
    isLoading: false,
    isSaving: false,
    isPortable: false,
    appConfigDir: "/app-config",
    resolvedDirs: {
      claude: "/claude",
      codex: "/codex",
      gemini: "/gemini",
    },
    requiresRestart: false,
    updateSettings: vi.fn(),
    updateDirectory: vi.fn(),
    updateAppConfigDir: vi.fn(),
    browseDirectory: vi.fn(),
    browseAppConfigDir: vi.fn(),
    resetDirectory: vi.fn(),
    resetAppConfigDir: vi.fn(),
    applyWslTemplate: vi.fn(),
    saveSettings: vi.fn().mockResolvedValue({ requiresRestart: false }),
    resetSettings: vi.fn(),
    acknowledgeRestart: vi.fn(),
  };

  return { ...base, ...overrides };
};

interface ImportExportMock {
  selectedFile: string;
  status: string;
  errorMessage: string | null;
  backupId: string | null;
  isImporting: boolean;
  selectImportFile: ReturnType<typeof vi.fn>;
  importConfig: ReturnType<typeof vi.fn>;
  exportConfig: ReturnType<typeof vi.fn>;
  clearSelection: ReturnType<typeof vi.fn>;
  resetStatus: ReturnType<typeof vi.fn>;
}

const createImportExportMock = (overrides: Partial<ImportExportMock> = {}) => {
  const base: ImportExportMock = {
    selectedFile: "",
    status: "idle",
    errorMessage: null,
    backupId: null,
    isImporting: false,
    selectImportFile: vi.fn(),
    importConfig: vi.fn(),
    exportConfig: vi.fn(),
    clearSelection: vi.fn(),
    resetStatus: vi.fn(),
  };

  return { ...base, ...overrides };
};

let settingsMock = createSettingsMock();
let importExportMock = createImportExportMock();
const useImportExportSpy = vi.fn();
let lastUseImportExportOptions: Record<string, unknown> | undefined;

vi.mock("@/hooks/useSettings", () => ({
  useSettings: () => settingsMock,
}));

vi.mock("@/hooks/useImportExport", () => ({
  useImportExport: (options?: Record<string, unknown>) =>
    useImportExportSpy(options),
}));

vi.mock("@/hooks/useBackupManager", () => ({
  useBackupManager: () => ({
    backups: [],
    isLoading: false,
    create: vi.fn(),
    restore: vi.fn(),
    rename: vi.fn(),
    remove: vi.fn(),
    isBusy: false,
  }),
}));

vi.mock("@/lib/api", () => ({
  settingsApi: {
    restart: vi.fn().mockResolvedValue(true),
    updateWebCredentials: vi.fn().mockResolvedValue(true),
  },
  providersApi: {
    getUniversalAll: vi.fn().mockResolvedValue({}),
    upsertUniversal: vi.fn(),
    deleteUniversal: vi.fn(),
    syncUniversal: vi.fn(),
  },
}));

const TabsContext = createContext<{
  value: string;
  onValueChange?: (value: string) => void;
}>({
  value: "general",
});

vi.mock("@/components/ui/dialog", () => ({
  Dialog: ({ open, children }: any) =>
    open ? <div data-testid="dialog-root">{children}</div> : null,
  DialogContent: ({ children }: any) => <div>{children}</div>,
  DialogHeader: ({ children }: any) => <div>{children}</div>,
  DialogFooter: ({ children }: any) => <div>{children}</div>,
  DialogTitle: ({ children }: any) => <h2>{children}</h2>,
  DialogDescription: ({ children }: any) => <p>{children}</p>,
}));

vi.mock("@/components/ui/input", () => ({
  Input: ({ value, onChange, ...rest }: any) => (
    <input
      value={value}
      onChange={(event) =>
        onChange?.({ target: { value: event.target.value } })
      }
      {...rest}
    />
  ),
}));

vi.mock("@/components/ui/label", () => ({
  Label: ({ children, htmlFor }: any) => (
    <label htmlFor={htmlFor}>{children}</label>
  ),
}));

vi.mock("@/components/ui/tabs", () => {
  return {
    Tabs: ({ value, onValueChange, children }: any) => (
      <TabsContext.Provider value={{ value, onValueChange }}>
        <div data-testid="tabs">{children}</div>
      </TabsContext.Provider>
    ),
    TabsList: ({ children }: any) => <div>{children}</div>,
    TabsTrigger: ({ value, children }: any) => {
      const ctx = useContext(TabsContext);
      return (
        <button type="button" onClick={() => ctx.onValueChange?.(value)}>
          {children}
        </button>
      );
    },
    TabsContent: ({ value, children }: any) => {
      const ctx = useContext(TabsContext);
      if (ctx.value !== value) return null;
      return <div data-testid={`tab-${value}`}>{children}</div>;
    },
  };
});

vi.mock("@/components/settings/LanguageSettings", () => ({
  LanguageSettings: ({ value, onChange }: any) => (
    <div>
      <span>language:{value}</span>
      <button onClick={() => onChange("en")}>change-language</button>
    </div>
  ),
}));

vi.mock("@/components/settings/ThemeSettings", () => ({
  ThemeSettings: () => <div>theme-settings</div>,
}));

vi.mock("@/components/settings/WindowSettings", () => ({
  WindowSettings: ({ onChange }: any) => (
    <button onClick={() => onChange({ minimizeToTrayOnClose: false })}>
      window-settings
    </button>
  ),
}));

vi.mock("@/components/settings/DirectorySettings", () => ({
  DirectorySettings: ({
    onBrowseDirectory,
    onResetDirectory,
    onDirectoryChange,
    onBrowseAppConfig,
    onResetAppConfig,
    onAppConfigChange,
  }: any) => (
    <div>
      <button onClick={() => onBrowseDirectory("claude")}>
        browse-directory
      </button>
      <button onClick={() => onResetDirectory("claude")}>
        reset-directory
      </button>
      <button onClick={() => onDirectoryChange("codex", "/new/path")}>
        change-directory
      </button>
      <button onClick={() => onBrowseAppConfig()}>browse-app-config</button>
      <button onClick={() => onResetAppConfig()}>reset-app-config</button>
      <button onClick={() => onAppConfigChange("/app/new")}>
        change-app-config
      </button>
    </div>
  ),
}));

vi.mock("@/components/settings/AboutSection", () => ({
  AboutSection: ({ isPortable }: any) => <div>about:{String(isPortable)}</div>,
}));

vi.mock("@/components/settings/UniversalProvidersSection", () => ({
  UniversalProvidersSection: () => <div>universal-providers-section</div>,
}));

let settingsApi: any;

describe("SettingsDialog Component", () => {
  beforeEach(async () => {
    tMock.mockImplementation((key: string) => key);
    settingsMock = createSettingsMock();
    importExportMock = createImportExportMock();
    useImportExportSpy.mockReset();
    useImportExportSpy.mockImplementation(
      (options?: Record<string, unknown>) => {
        lastUseImportExportOptions = options;
        return importExportMock;
      },
    );
    lastUseImportExportOptions = undefined;
    toastSuccessMock.mockReset();
    toastErrorMock.mockReset();
    settingsApi = (await import("@/lib/api")).settingsApi;
    settingsApi.restart.mockClear();
    settingsApi.updateWebCredentials.mockClear();
    settingsApi.updateWebCredentials.mockResolvedValue(true);
    adapterMocks.setWebCredentials.mockClear();
    adapterMocks.getWebApiBase.mockClear();
    adapterMocks.getWebApiBase.mockReturnValue("/api");
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("should not render form content when loading", () => {
    settingsMock = createSettingsMock({ settings: null, isLoading: true });

    render(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    expect(screen.queryByText("language:zh")).not.toBeInTheDocument();
    expect(screen.getByText("settings.title")).toBeInTheDocument();
  });

  it("should reset import/export status when dialog transitions to open", () => {
    const { rerender } = render(
      <SettingsDialog open={false} onOpenChange={vi.fn()} />,
    );

    importExportMock.resetStatus.mockClear();

    rerender(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    expect(importExportMock.resetStatus).toHaveBeenCalledTimes(1);
  });

  it("should render general and advanced tabs and trigger child callbacks", () => {
    const onOpenChange = vi.fn();
    importExportMock = createImportExportMock({
      selectedFile: "/tmp/config.json",
    });

    render(<SettingsDialog open={true} onOpenChange={onOpenChange} />);

    expect(screen.getByText("language:zh")).toBeInTheDocument();
    expect(screen.getByText("theme-settings")).toBeInTheDocument();

    fireEvent.click(screen.getByText("change-language"));
    expect(settingsMock.updateSettings).toHaveBeenCalledWith({
      language: "en",
    });

    fireEvent.click(screen.getByText("window-settings"));
    expect(settingsMock.updateSettings).toHaveBeenCalledWith({
      minimizeToTrayOnClose: false,
    });

    fireEvent.click(screen.getByText("settings.tabAdvanced"));
    fireEvent.click(
      screen.getByRole("button", { name: "settings.selectConfigFile" }),
    );

    expect(importExportMock.selectImportFile).toHaveBeenCalled();

    fireEvent.click(
      screen.getByRole("button", { name: "settings.exportConfig" }),
    );
    expect(importExportMock.exportConfig).toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "settings.import" }));
    expect(importExportMock.importConfig).toHaveBeenCalled();

    fireEvent.change(
      screen.getByLabelText("settings.network.githubMirrorBaseUrl"),
      {
        target: { value: "https://ghproxy.net/" },
      },
    );
    expect(settingsMock.updateSettings).toHaveBeenCalledWith({
      network: { githubMirrorBaseUrl: "https://ghproxy.net/" },
    });

    fireEvent.click(screen.getByRole("button", { name: "common.clear" }));
    expect(importExportMock.clearSelection).toHaveBeenCalled();
  });

  it("should pass onImportSuccess callback to useImportExport hook", async () => {
    const onImportSuccess = vi.fn();

    render(
      <SettingsDialog
        open={true}
        onOpenChange={vi.fn()}
        onImportSuccess={onImportSuccess}
      />,
    );

    expect(useImportExportSpy).toHaveBeenCalledWith(
      expect.objectContaining({ onImportSuccess }),
    );
    expect(lastUseImportExportOptions?.onImportSuccess).toBe(onImportSuccess);

    if (typeof lastUseImportExportOptions?.onImportSuccess === "function") {
      await lastUseImportExportOptions.onImportSuccess();
    }
    expect(onImportSuccess).toHaveBeenCalledTimes(1);
  });

  it("should call saveSettings and close dialog when clicking save", async () => {
    const onOpenChange = vi.fn();
    importExportMock = createImportExportMock();

    render(<SettingsDialog open={true} onOpenChange={onOpenChange} />);

    fireEvent.click(screen.getByText("common.save"));

    await waitFor(() => {
      expect(settingsMock.saveSettings).toHaveBeenCalledTimes(1);
      expect(importExportMock.clearSelection).toHaveBeenCalledTimes(1);
      expect(importExportMock.resetStatus).toHaveBeenCalledTimes(2);
      expect(settingsMock.acknowledgeRestart).toHaveBeenCalledTimes(1);
      expect(onOpenChange).toHaveBeenCalledWith(false);
    });
  });

  it("should reset settings and close dialog when clicking cancel", () => {
    const onOpenChange = vi.fn();

    render(<SettingsDialog open={true} onOpenChange={onOpenChange} />);

    fireEvent.click(screen.getByText("common.cancel"));

    expect(settingsMock.resetSettings).toHaveBeenCalledTimes(1);
    expect(settingsMock.acknowledgeRestart).toHaveBeenCalledTimes(1);
    expect(importExportMock.clearSelection).toHaveBeenCalledTimes(1);
    expect(importExportMock.resetStatus).toHaveBeenCalledTimes(2);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("should show restart prompt and allow immediate restart after save", async () => {
    settingsMock = createSettingsMock({
      requiresRestart: true,
      saveSettings: vi.fn().mockResolvedValue({ requiresRestart: true }),
    });

    render(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    expect(
      await screen.findByText("settings.restartRequired"),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByText("settings.restartNow"));

    await waitFor(() => {
      expect(toastSuccessMock).toHaveBeenCalledWith(
        "settings.devModeRestartHint",
      );
    });
  });

  it("should allow postponing restart and close dialog without restarting", async () => {
    const onOpenChange = vi.fn();
    settingsMock = createSettingsMock({ requiresRestart: true });

    render(<SettingsDialog open={true} onOpenChange={onOpenChange} />);

    expect(
      await screen.findByText("settings.restartRequired"),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByText("settings.restartLater"));

    await waitFor(() => {
      expect(onOpenChange).toHaveBeenCalledWith(false);
      expect(settingsMock.acknowledgeRestart).toHaveBeenCalledTimes(1);
    });

    expect(settingsApi.restart).not.toHaveBeenCalled();
    expect(toastSuccessMock).not.toHaveBeenCalled();
    expect(toastErrorMock).not.toHaveBeenCalled();
  });

  it("should trigger directory management callbacks inside advanced tab", () => {
    render(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    fireEvent.click(screen.getByText("settings.tabAdvanced"));

    fireEvent.click(screen.getByText("browse-directory"));
    expect(settingsMock.browseDirectory).toHaveBeenCalledWith("claude");

    fireEvent.click(screen.getByText("reset-directory"));
    expect(settingsMock.resetDirectory).toHaveBeenCalledWith("claude");

    fireEvent.click(screen.getByText("change-directory"));
    expect(settingsMock.updateDirectory).toHaveBeenCalledWith(
      "codex",
      "/new/path",
    );

    fireEvent.click(screen.getByText("browse-app-config"));
    expect(settingsMock.browseAppConfigDir).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("reset-app-config"));
    expect(settingsMock.resetAppConfigDir).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("change-app-config"));
    expect(settingsMock.updateAppConfigDir).toHaveBeenCalledWith("/app/new");
  });

  it("should submit web credentials in advanced tab", async () => {
    render(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    fireEvent.click(screen.getByText("settings.tabAdvanced"));

    fireEvent.change(
      screen.getByLabelText("settings.webCredentials.username"),
      {
        target: { value: "alice" },
      },
    );
    fireEvent.change(
      screen.getByLabelText("settings.webCredentials.password"),
      {
        target: { value: "secret123" },
      },
    );
    fireEvent.change(
      screen.getByLabelText("settings.webCredentials.confirmPassword"),
      {
        target: { value: "secret123" },
      },
    );

    fireEvent.click(
      screen.getByRole("button", { name: "settings.webCredentials.submit" }),
    );

    await waitFor(() => {
      expect(settingsApi.updateWebCredentials).toHaveBeenCalledWith(
        "alice",
        "secret123",
      );
      expect(adapterMocks.setWebCredentials).toHaveBeenCalledWith(
        "alice",
        "secret123",
        "/api",
      );
    });
  });

  it("should reject too short web credentials password before submit", async () => {
    render(<SettingsDialog open={true} onOpenChange={vi.fn()} />);

    fireEvent.click(screen.getByText("settings.tabAdvanced"));

    fireEvent.change(
      screen.getByLabelText("settings.webCredentials.username"),
      {
        target: { value: "alice" },
      },
    );
    fireEvent.change(
      screen.getByLabelText("settings.webCredentials.password"),
      {
        target: { value: "short" },
      },
    );
    fireEvent.change(
      screen.getByLabelText("settings.webCredentials.confirmPassword"),
      {
        target: { value: "short" },
      },
    );

    fireEvent.click(
      screen.getByRole("button", { name: "settings.webCredentials.submit" }),
    );

    await waitFor(() => {
      expect(toastErrorMock).toHaveBeenCalledWith(
        "settings.webCredentials.validation.passwordTooShort",
      );
    });
    expect(settingsApi.updateWebCredentials).not.toHaveBeenCalled();
  });
});
