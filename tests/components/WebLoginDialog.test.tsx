import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WebLoginDialog } from "@/components/WebLoginDialog";

let apiBaseForBuild = "/custom-api";

type AdapterMocks = {
  buildWebApiUrlWithBase: ReturnType<
    typeof vi.fn<(base: string, path: string) => string>
  >;
  clearWebApiBaseOverride: ReturnType<typeof vi.fn>;
  clearWebCredentials: ReturnType<typeof vi.fn>;
  setWebCredentials: ReturnType<
    typeof vi.fn<
      (username: string, password: string, apiBase?: string | null) => void
    >
  >;
  setWebApiBaseOverride: ReturnType<typeof vi.fn<(value: string) => void>>;
  base64EncodeUtf8: ReturnType<typeof vi.fn<() => string>>;
  getWebApiBase: ReturnType<typeof vi.fn<() => string>>;
  getStoredWebApiBase: ReturnType<typeof vi.fn<() => string | undefined>>;
  getStoredWebUsername: ReturnType<typeof vi.fn<() => string>>;
  getWebApiBaseValidationError: ReturnType<
    typeof vi.fn<(value: string) => string | null>
  >;
  normalizeWebApiBase: ReturnType<typeof vi.fn<(value: string) => string | null>>;
  WEB_CSRF_STORAGE_KEY: string;
};

const adapterMocks = vi.hoisted(() => ({
  buildWebApiUrlWithBase: vi.fn<(base: string, path: string) => string>(
    (base, path) => `${base}${path}`,
  ),
  clearWebApiBaseOverride: vi.fn(),
  clearWebCredentials: vi.fn(),
  setWebCredentials: vi.fn<
    (username: string, password: string, apiBase?: string | null) => void
  >(),
  setWebApiBaseOverride: vi.fn<(value: string) => void>(),
  base64EncodeUtf8: vi.fn(() => "encoded-value"),
  getWebApiBase: vi.fn(() => apiBaseForBuild),
  getStoredWebApiBase: vi.fn<() => string | undefined>(() => undefined),
  getStoredWebUsername: vi.fn<() => string>(() => "admin"),
  getWebApiBaseValidationError: vi.fn<(value: string) => string | null>(
    () => null,
  ),
  normalizeWebApiBase: vi.fn<(value: string) => string | null>((value) => {
    if (typeof value !== "string") return null;
    const trimmed = value.trim();
    if (!trimmed) return null;
    return trimmed.replace(/\/+$/, "");
  }),
  WEB_CSRF_STORAGE_KEY: "cc-switch-csrf-token",
})) as AdapterMocks;

vi.mock("@/lib/api/adapter", () => adapterMocks);

vi.mock("@/components/ui/dialog", () => ({
  Dialog: ({ open, children }: any) =>
    open ? <div data-testid="dialog-root">{children}</div> : null,
  DialogContent: ({ children }: any) => <div>{children}</div>,
  DialogHeader: ({ children }: any) => <div>{children}</div>,
  DialogTitle: ({ children }: any) => <h2>{children}</h2>,
  DialogDescription: ({ children }: any) => <p>{children}</p>,
  DialogFooter: ({ children }: any) => <div>{children}</div>,
}));

vi.mock("@/components/ui/button", () => ({
  Button: ({ children, type = "button", ...rest }: any) => (
    <button type={type} {...rest}>
      {children}
    </button>
  ),
}));

vi.mock("@/components/ui/input", () => ({
  Input: ({ value, onChange, ...rest }: any) => (
    <input
      value={value}
      onChange={(event) => onChange?.({ target: { value: event.target.value } })}
      {...rest}
    />
  ),
}));

vi.mock("@/components/ui/label", () => ({
  Label: ({ children, htmlFor }: any) => (
    <label htmlFor={htmlFor}>{children}</label>
  ),
}));

let fetchMock: ReturnType<typeof vi.fn>;
let sessionStorageMock: {
  getItem: ReturnType<typeof vi.fn>;
  setItem: ReturnType<typeof vi.fn>;
  removeItem: ReturnType<typeof vi.fn>;
  clear: ReturnType<typeof vi.fn>;
};

beforeEach(() => {
  fetchMock = vi.fn();
  globalThis.fetch = fetchMock as unknown as typeof fetch;
  apiBaseForBuild = "/custom-api";

  sessionStorageMock = {
    getItem: vi.fn(),
    setItem: vi.fn(),
    removeItem: vi.fn(),
    clear: vi.fn(),
  };
  Object.defineProperty(window, "sessionStorage", {
    value: sessionStorageMock,
    configurable: true,
  });

  vi.clearAllMocks();
});

describe("WebLoginDialog", () => {
  it("renders the login form when open", () => {
    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    expect(screen.getByRole("heading", { name: "登录" })).toBeInTheDocument();
    expect(
      screen.getByLabelText("API 地址 (可选)"),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("用户名")).toBeInTheDocument();
    expect(screen.getByLabelText("密码")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "登录" })).toBeInTheDocument();
  });

  it("clears stored API base when clicking 清除", async () => {
    const user = userEvent.setup();
    adapterMocks.getStoredWebApiBase.mockReturnValueOnce(
      "https://api.example.com/api",
    );

    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    const apiBaseInput = screen.getByLabelText(
      "API 地址 (可选)",
    ) as HTMLInputElement;

    await waitFor(() =>
      expect(apiBaseInput.value).toBe("https://api.example.com/api"),
    );

    await user.click(screen.getByRole("button", { name: "清除" }));

    expect(adapterMocks.clearWebApiBaseOverride).toHaveBeenCalled();
    expect(apiBaseInput.value).toBe("");
  });

  it("shows validation error when password is empty", async () => {
    const user = userEvent.setup();
    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "登录" }));

    expect(await screen.findByText("请输入密码")).toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("submits API request with basic auth", async () => {
    const user = userEvent.setup();
    fetchMock
      .mockResolvedValueOnce({ ok: true, status: 200 })
      .mockResolvedValueOnce({ ok: false, status: 500 });

    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    const usernameInput = screen.getByLabelText(
      "用户名",
    ) as HTMLInputElement;
    await user.clear(usernameInput);
    await user.type(usernameInput, "alice");
    await user.type(screen.getByLabelText("密码"), "secret");
    await user.click(screen.getByRole("button", { name: "登录" }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalled());

    expect(adapterMocks.base64EncodeUtf8).toHaveBeenCalledWith("alice:secret");
    expect(adapterMocks.clearWebApiBaseOverride).toHaveBeenCalled();
    expect(adapterMocks.buildWebApiUrlWithBase).toHaveBeenCalledWith(
      "/custom-api",
      "/settings",
    );
    expect(fetchMock).toHaveBeenCalledWith(
      "/custom-api/settings",
      expect.objectContaining({
        method: "GET",
        credentials: "include",
        headers: expect.objectContaining({
          Accept: "application/json",
          Authorization: "Basic encoded-value",
        }),
      }),
    );
  });

  it("uses API base override when provided", async () => {
    const user = userEvent.setup();
    fetchMock
      .mockResolvedValueOnce({ ok: true, status: 200 })
      .mockResolvedValueOnce({ ok: false, status: 500 });
    adapterMocks.normalizeWebApiBase.mockReturnValueOnce(
      "https://example.com/api",
    );
    apiBaseForBuild = "https://example.com/api";

    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    await user.type(
      screen.getByLabelText("API 地址 (可选)"),
      "https://example.com/api/",
    );
    await user.type(screen.getByLabelText("密码"), "secret");
    await user.click(screen.getByRole("button", { name: "登录" }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalled());

    expect(adapterMocks.setWebApiBaseOverride).toHaveBeenCalledWith(
      "https://example.com/api",
    );
    expect(fetchMock).toHaveBeenCalledWith(
      "https://example.com/api/settings",
      expect.objectContaining({
        method: "GET",
        credentials: "include",
      }),
    );
  });

  it("clears stored credentials when API base changes", async () => {
    const user = userEvent.setup();
    fetchMock
      .mockResolvedValueOnce({ ok: true, status: 200 })
      .mockResolvedValueOnce({ ok: false, status: 500 });
    adapterMocks.getStoredWebApiBase
      .mockReturnValueOnce("https://old.example.com/api")
      .mockReturnValueOnce("https://old.example.com/api");

    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    const apiBaseInput = screen.getByLabelText(
      "API 地址 (可选)",
    ) as HTMLInputElement;

    await waitFor(() =>
      expect(apiBaseInput.value).toBe("https://old.example.com/api"),
    );

    await user.clear(apiBaseInput);
    await user.type(apiBaseInput, "https://new.example.com/api");
    await user.type(screen.getByLabelText("密码"), "secret");
    await user.click(screen.getByRole("button", { name: "登录" }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalled());

    expect(adapterMocks.clearWebCredentials).toHaveBeenCalled();
    expect(adapterMocks.setWebApiBaseOverride).toHaveBeenCalledWith(
      "https://new.example.com/api",
    );
  });

  it("keeps existing credentials when API base changes but login fails", async () => {
    const user = userEvent.setup();
    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 401,
      text: async () => "Unauthorized",
    });
    adapterMocks.getStoredWebApiBase.mockReturnValueOnce(
      "https://old.example.com/api",
    );

    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    const apiBaseInput = screen.getByLabelText(
      "API 地址 (可选)",
    ) as HTMLInputElement;

    await waitFor(() =>
      expect(apiBaseInput.value).toBe("https://old.example.com/api"),
    );

    await user.clear(apiBaseInput);
    await user.type(apiBaseInput, "https://new.example.com/api");
    await user.type(screen.getByLabelText("密码"), "wrong-secret");
    await user.click(screen.getByRole("button", { name: "登录" }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalled());

    expect(adapterMocks.clearWebCredentials).not.toHaveBeenCalled();
    expect(adapterMocks.setWebApiBaseOverride).not.toHaveBeenCalled();
    expect(adapterMocks.setWebCredentials).not.toHaveBeenCalled();
  });

  it("shows validation error when API base is invalid", async () => {
    const user = userEvent.setup();
    adapterMocks.getWebApiBaseValidationError.mockReturnValueOnce(
      "API 地址无效",
    );

    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    await user.type(screen.getByLabelText("API 地址 (可选)"), "invalid");
    await user.type(screen.getByLabelText("密码"), "secret");
    await user.click(screen.getByRole("button", { name: "登录" }));

    expect(await screen.findByText("API 地址无效")).toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("shows password error for 401 responses", async () => {
    const user = userEvent.setup();
    fetchMock.mockResolvedValueOnce({ ok: false, status: 401 });

    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    await user.type(screen.getByLabelText("密码"), "wrong");
    await user.click(screen.getByRole("button", { name: "登录" }));

    expect(await screen.findByText("用户名或密码错误")).toBeInTheDocument();
  });

  it("calls onLoginSuccess on successful login", async () => {
    const user = userEvent.setup();
    const onLoginSuccess = vi.fn();
    fetchMock
      .mockResolvedValueOnce({ ok: true, status: 200 })
      .mockResolvedValueOnce({ ok: false, status: 500 });

    render(<WebLoginDialog open onLoginSuccess={onLoginSuccess} />);

    await user.type(screen.getByLabelText("密码"), "secret");
    await user.click(screen.getByRole("button", { name: "登录" }));

    await waitFor(() => expect(onLoginSuccess).toHaveBeenCalledTimes(1));
  });

  it("stores CSRF token in sessionStorage after login", async () => {
    const user = userEvent.setup();
    fetchMock
      .mockResolvedValueOnce({ ok: true, status: 200 })
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: vi.fn().mockResolvedValue({ csrfToken: "csrf-token" }),
      });

    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    await user.type(screen.getByLabelText("密码"), "secret");
    await user.click(screen.getByRole("button", { name: "登录" }));

    await waitFor(() =>
      expect(sessionStorageMock.setItem).toHaveBeenCalledWith(
        adapterMocks.WEB_CSRF_STORAGE_KEY,
        "csrf-token",
      ),
    );
  });

  it("shows network error message on fetch failure", async () => {
    const user = userEvent.setup();
    fetchMock.mockRejectedValueOnce(new Error());

    render(<WebLoginDialog open onLoginSuccess={vi.fn()} />);

    await user.type(screen.getByLabelText("密码"), "secret");
    await user.click(screen.getByRole("button", { name: "登录" }));

    expect(await screen.findByText("网络错误")).toBeInTheDocument();
  });
});
