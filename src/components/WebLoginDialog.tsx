import { useCallback, useEffect, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  base64EncodeUtf8,
  buildWebApiUrlWithBase,
  clearWebApiBaseOverride,
  clearWebCredentials,
  getWebApiBase,
  getWebApiBaseValidationError,
  getStoredWebApiBase,
  getStoredWebUsername,
  normalizeWebApiBase,
  setWebApiBaseOverride,
  setWebCredentials,
  WEB_CSRF_STORAGE_KEY,
} from "@/lib/api/adapter";

export interface WebLoginDialogProps {
  open: boolean;
  onLoginSuccess: () => void;
}

export function WebLoginDialog({ open, onLoginSuccess }: WebLoginDialogProps) {
  const [apiBase, setApiBase] = useState("");
  const [apiBaseError, setApiBaseError] = useState<string | null>(null);
  const [username, setUsername] = useState(() => getStoredWebUsername());
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const apiBaseHelperId = "cc-switch-web-api-base-helper";
  const apiBaseErrorId = "cc-switch-web-api-base-error";

  useEffect(() => {
    if (!open) return;
    setApiBase(getStoredWebApiBase() ?? "");
    setApiBaseError(null);
    setUsername(getStoredWebUsername());
    setPassword("");
    setError(null);
    setIsSubmitting(false);
  }, [open]);

  const handleClearApiBase = useCallback(() => {
    setApiBase("");
    setApiBaseError(null);
    clearWebApiBaseOverride();
  }, []);

  const handleLogin = useCallback(async () => {
    if (isSubmitting) return;

    const trimmedUsername = username.trim();
    if (!trimmedUsername) {
      setError("请输入用户名");
      return;
    }
    const trimmedPassword = password.trim();
    if (!trimmedPassword) {
      setError("请输入密码");
      return;
    }

    const apiBaseValidationError = getWebApiBaseValidationError(apiBase);
    if (apiBaseValidationError) {
      setApiBaseError(apiBaseValidationError);
      setError(null);
      return;
    }
    setApiBaseError(null);

    setIsSubmitting(true);
    setError(null);

    try {
      const encoded = base64EncodeUtf8(
        `${trimmedUsername}:${trimmedPassword}`,
      );
      const normalizedApiBase = normalizeWebApiBase(apiBase);
      const previousApiBase = getStoredWebApiBase();
      const nextApiBase = normalizedApiBase ?? null;
      const effectiveApiBase = normalizedApiBase ?? getWebApiBase();
      const response = await fetch(
        buildWebApiUrlWithBase(effectiveApiBase, "/settings"),
        {
          method: "GET",
          credentials: "include",
          headers: {
            Accept: "application/json",
            Authorization: `Basic ${encoded}`,
          },
        },
      );

      if (response.ok) {
        if ((previousApiBase ?? null) !== nextApiBase) {
          clearWebCredentials();
        }
        if (normalizedApiBase) {
          setWebApiBaseOverride(normalizedApiBase);
        } else {
          clearWebApiBaseOverride();
        }
        setWebCredentials(
          trimmedUsername,
          trimmedPassword,
          normalizedApiBase ?? getWebApiBase(),
        );
        try {
          const tokenResponse = await fetch(
            buildWebApiUrlWithBase(effectiveApiBase, "/system/csrf-token"),
            {
              method: "GET",
              credentials: "include",
              headers: {
                Accept: "application/json",
                Authorization: `Basic ${encoded}`,
              },
            },
          );
          if (tokenResponse.ok) {
            const data = (await tokenResponse.json()) as {
              csrfToken?: string | null;
            };
            if (data?.csrfToken) {
              window.sessionStorage?.setItem(
                WEB_CSRF_STORAGE_KEY,
                data.csrfToken,
              );
            } else {
              window.sessionStorage?.removeItem(WEB_CSRF_STORAGE_KEY);
            }
          }
        } catch {
          // ignore
        }
        onLoginSuccess();
        return;
      }

      if (response.status === 401) {
        setError("用户名或密码错误");
        return;
      }

      const detail = (await response.text())?.trim();
      setError(detail || `登录失败（${response.status}）`);
    } catch (e) {
      setError((e as Error)?.message || "网络错误");
    } finally {
      setIsSubmitting(false);
    }
  }, [apiBase, isSubmitting, onLoginSuccess, password, username]);

  return (
    <Dialog open={open} onOpenChange={() => {}}>
      <DialogContent className="max-w-sm">
        <DialogHeader className="space-y-2">
          <DialogTitle>登录</DialogTitle>
          <DialogDescription>请输入用户名和密码</DialogDescription>
        </DialogHeader>

        <form
          className="space-y-4 px-6 py-5"
          onSubmit={(e) => {
            e.preventDefault();
            void handleLogin();
          }}
        >
          <div className="space-y-2">
            <Label htmlFor="cc-switch-web-api-base">API 地址 (可选)</Label>
            <div className="flex items-center gap-2">
              <Input
                id="cc-switch-web-api-base"
                name="apiBase"
                type="text"
                autoComplete="url"
                inputMode="url"
                placeholder={getWebApiBase()}
                value={apiBase}
                onChange={(e) => {
                  setApiBase(e.target.value);
                  if (apiBaseError) setApiBaseError(null);
                }}
                aria-describedby={`${apiBaseHelperId}${
                  apiBaseError ? ` ${apiBaseErrorId}` : ""
                }`}
                aria-invalid={apiBaseError ? "true" : undefined}
                disabled={isSubmitting}
                className="flex-1"
              />
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={handleClearApiBase}
                disabled={isSubmitting || !apiBase}
              >
                清除
              </Button>
            </div>
            <p id={apiBaseHelperId} className="text-xs text-muted-foreground">
              支持 https://example.com/api 或 /api，留空使用默认值。局域网地址在服务端启用
              ALLOW_LAN_CORS 后会自动放行。
            </p>
            {apiBaseError ? (
              <p id={apiBaseErrorId} className="text-xs text-destructive">
                {apiBaseError}
              </p>
            ) : null}
          </div>
          <div className="space-y-2">
            <Label htmlFor="cc-switch-web-username">用户名</Label>
            <Input
              id="cc-switch-web-username"
              name="username"
              type="text"
              autoComplete="username"
              autoFocus
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              disabled={isSubmitting}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="cc-switch-web-password">密码</Label>
            <Input
              id="cc-switch-web-password"
              name="password"
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={isSubmitting}
            />
          </div>

          {error ? (
            <div className="text-sm text-destructive">{error}</div>
          ) : null}

          <DialogFooter className="px-0 py-0 border-0 bg-transparent">
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "验证中..." : "登录"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export default WebLoginDialog;
